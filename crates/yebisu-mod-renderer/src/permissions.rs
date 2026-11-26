//! The device's A12 permission ledger. A permission is issued for one
//! individual tap or key event and one operation, travels in that Action's
//! `ActionData.permissions`, and may be spent once, within one second on the
//! device's monotonic clock, by a DeviceRequest from the same connection,
//! copy incarnation and owner fence under the same Policy revision.
use std::collections::{BTreeMap, VecDeque};
use yebisu_mod_wire as wire;
use yebisu_mod_wire::{DeviceOperation, DeviceResult, limits};

/// What the app performs for an accepted request.
#[derive(Debug, Clone, PartialEq)]
pub enum DeviceEffect {
    /// Open `url` after showing `host` in the confirmation sheet.
    OpenUrl {
        url: String,
        host: String,
    },
    ClipboardWrite {
        text: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Live,
    Consumed,
    Cancelled,
}

#[derive(Debug, Clone)]
struct Grant {
    policy_rev: wire::Counter,
    event_id: String,
    session: wire::Session,
    scope: wire::Scope,
    owner_fence: wire::Counter,
    issued_ms: u64,
    state: State,
}

/// Replies kept for duplicate request IDs, so a resent request is not spent twice.
const REMEMBERED_REPLIES: usize = 256;

pub struct DevicePermissions {
    policy_rev: wire::Counter,
    remote_clipboard: bool,
    grants: BTreeMap<wire::Id128, Grant>,
    /// The Action each event's permissions were sent in, for the hand-off check.
    actions: BTreeMap<(String, wire::Id128, String), wire::Action>,
    /// Replies to authenticated requests only, keyed by the authenticated
    /// session, copy scope, owner fence and request ID.
    replies: VecDeque<(ReplyKey, wire::DeviceReply)>,
}

#[derive(Debug, Clone, PartialEq)]
struct ReplyKey {
    session: wire::Session,
    scope: wire::Scope,
    owner_fence: wire::Counter,
    request_id: String,
}

fn reply(request_id: &str, result: DeviceResult) -> wire::DeviceReply {
    wire::DeviceReply {
        request_id: request_id.into(),
        result,
    }
}

impl DevicePermissions {
    /// `remote_clipboard` says whether this device lets a host write its clipboard.
    pub fn new(policy_rev: wire::Counter, remote_clipboard: bool) -> Self {
        Self {
            policy_rev,
            remote_clipboard,
            grants: BTreeMap::new(),
            actions: BTreeMap::new(),
            replies: VecDeque::new(),
        }
    }
    /// Issue one permission per operation for an individual tap or key event
    /// of the copy behind `context`. Tokens come from the caller's CSPRNG.
    pub fn issue(
        &mut self,
        context: &wire::ReceiveContext,
        kind: wire::ActionKind,
        event_id: &str,
        operations: &[DeviceOperation],
        now_ms: u64,
        mut token: impl FnMut() -> wire::Id128,
    ) -> Option<Vec<wire::DevicePermission>> {
        if !matches!(kind, wire::ActionKind::Click | wire::ActionKind::Key) || event_id.is_empty() {
            return None;
        }
        let mut issued = Vec::new();
        for operation in operations {
            if issued
                .iter()
                .any(|p: &wire::DevicePermission| p.operation == *operation)
            {
                return None;
            }
            let permission = token();
            if self.grants.contains_key(&permission) {
                return None;
            }
            issued.push(wire::DevicePermission {
                permission,
                operation: *operation,
                policy_rev: self.policy_rev,
            });
        }
        for p in &issued {
            self.grants.insert(
                p.permission.clone(),
                Grant {
                    policy_rev: p.policy_rev,
                    event_id: event_id.into(),
                    session: context.session.clone(),
                    scope: context.scope.clone(),
                    owner_fence: context.owner_fence,
                    issued_ms: now_ms,
                    state: State::Live,
                },
            );
        }
        Some(issued)
    }
    /// Record the Action that carried the permissions, as it was sent.
    pub fn sent(&mut self, action: &wire::Envelope<wire::Action>) -> wire::Result<()> {
        action.validate()?;
        let (copy, incarnation) =
            copy_of(&action.scope).ok_or(wire::Error::Invalid("action scope"))?;
        self.actions.insert(
            (copy, incarnation, action.body.event_id.clone()),
            action.body.clone(),
        );
        Ok(())
    }
    /// Pause or Unload: the copy's unspent permissions can no longer be used.
    pub fn cancel_copy(&mut self, scope: &wire::Scope) {
        for grant in self.grants.values_mut().filter(|g| g.scope == *scope) {
            if grant.state == State::Live {
                grant.state = State::Cancelled;
            }
        }
    }
    /// The connection ended or changed epoch: nothing it was issued survives.
    pub fn cancel_connection(&mut self, session: &wire::Session) {
        for grant in self.grants.values_mut().filter(|g| g.session == *session) {
            if grant.state == State::Live {
                grant.state = State::Cancelled;
            }
        }
    }
    /// A new Policy revision invalidates permissions issued under the old one.
    pub fn set_policy(&mut self, rev: wire::Counter) {
        self.policy_rev = rev;
    }
    /// Forget grants older than the expiry window (their requests are Expired).
    pub fn prune(&mut self, now_ms: u64) {
        self.grants
            .retain(|_, g| now_ms.saturating_sub(g.issued_ms) <= limits::GRANT_MS as u64);
    }

    /// Answer a DeviceRequest received on `context` at monotonic `now_ms`.
    pub fn handle(
        &mut self,
        request: &wire::Envelope<wire::DeviceRequest>,
        context: &wire::ReceiveContext,
        now_ms: u64,
    ) -> (wire::DeviceReply, Option<DeviceEffect>) {
        let id = request.body.request_id.clone();
        // Current connection, copy incarnation, owner fence and canonical
        // value, before the replay cache: a forged request can neither read
        // a remembered reply nor fill the cache.
        if context.validate(request).is_err() {
            return (reply(&id, DeviceResult::Denied), None);
        }
        let key = ReplyKey {
            session: context.session.clone(),
            scope: context.scope.clone(),
            owner_fence: context.owner_fence,
            request_id: id.clone(),
        };
        if let Some((_, reply)) = self.replies.iter().find(|(k, _)| *k == key) {
            return (reply.clone(), None);
        }
        let (result, effect) = match self.decide(request, now_ms) {
            Ok(effect) => (DeviceResult::Ok, Some(effect)),
            Err(result) => (result, None),
        };
        let answer = reply(&id, result);
        if self.replies.len() == REMEMBERED_REPLIES {
            self.replies.pop_front();
        }
        self.replies.push_back((key, answer.clone()));
        (answer, effect)
    }

    /// Called only after the envelope was authenticated.
    fn decide(
        &mut self,
        request: &wire::Envelope<wire::DeviceRequest>,
        now_ms: u64,
    ) -> Result<DeviceEffect, DeviceResult> {
        let body = &request.body;
        let grant = self
            .grants
            .get(&body.permission)
            .ok_or(DeviceResult::Denied)?;
        if grant.session != request.session
            || grant.scope != request.scope
            || grant.owner_fence != request.owner_fence
        {
            return Err(DeviceResult::Denied);
        }
        // The explicit hand-off: this event's Action carried this permission
        // for this operation under this Policy revision.
        let (copy, incarnation) = copy_of(&request.scope).ok_or(DeviceResult::Denied)?;
        let action = self
            .actions
            .get(&(copy, incarnation, grant.event_id.clone()))
            .ok_or(DeviceResult::Denied)?;
        body.validate_permission(action)
            .map_err(|_| DeviceResult::Denied)?;
        // The Action was looked up by the grant's own event, and it carries
        // this permission for this operation: event and operation match.
        match grant.state {
            State::Live => {}
            State::Consumed | State::Cancelled => return Err(DeviceResult::Denied),
        }
        if now_ms.saturating_sub(grant.issued_ms) > limits::GRANT_MS as u64 {
            // Expiry is final, even if a later clock reading looked earlier.
            if let Some(grant) = self.grants.get_mut(&body.permission) {
                grant.state = State::Cancelled;
            }
            return Err(DeviceResult::Expired);
        }
        if grant.policy_rev != self.policy_rev {
            return Err(DeviceResult::Denied);
        }
        let effect = match body.operation {
            DeviceOperation::OpenUrl => DeviceEffect::OpenUrl {
                host: wire::validate_url(&body.value).map_err(|_| DeviceResult::Denied)?,
                url: body.value.clone(),
            },
            DeviceOperation::ClipboardWrite => {
                if !self.remote_clipboard {
                    return Err(DeviceResult::UnsupportedRemoteClipboard);
                }
                DeviceEffect::ClipboardWrite {
                    text: body.value.clone(),
                }
            }
        };
        // Spent exactly once, whatever the app does with the effect.
        if let Some(grant) = self.grants.get_mut(&body.permission) {
            grant.state = State::Consumed;
        }
        Ok(effect)
    }
}

fn copy_of(scope: &wire::Scope) -> Option<(String, wire::Id128)> {
    match scope {
        wire::Scope::Copy { id, incarnation } => Some((id.clone(), incarnation.clone())),
        wire::Scope::Session {} => None,
    }
}
