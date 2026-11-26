//! The device side of the A13 command gateway. Every GatewayRequest gets one
//! GatewayReply; only an accepted request yields an effect for the app to run.
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use yebisu_mod_wire as wire;
use yebisu_mod_wire::{GatewayFailure, GatewayOperation, limits};

/// The device's current Policy for gateway operations.
#[derive(Debug, Clone, PartialEq)]
pub struct GatewayPolicy {
    pub rev: wire::Counter,
    /// Commands `run` may start; anything else is `policy_denied`.
    pub commands: BTreeSet<String>,
    /// Whether this mod may reorder agents.
    pub agent_order: bool,
}

/// What the app performs for an accepted request.
#[derive(Debug, Clone, PartialEq)]
pub enum GatewayEffect {
    Run { command: String, args: Vec<String> },
    SetAgentOrder { agents: Vec<String> },
    OpenOverlay(wire::Overlay),
    CloseOverlay { overlay_id: String },
    ShowToast(wire::Overlay),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Owner {
    copy: String,
    incarnation: wire::Id128,
}

/// Replies kept for duplicate request IDs, so a resent request never runs twice.
const REMEMBERED_REPLIES: usize = 256;

/// Gateway state for one host connection. Overlays and toasts belong to the
/// copy incarnation whose envelope opened them.
pub struct Gateway {
    policy: GatewayPolicy,
    operating: bool,
    overlays: BTreeMap<String, Owner>,
    toasts: VecDeque<String>,
    last_toast_ms: Option<u64>,
    stopped: BTreeSet<(String, wire::Id128)>,
    /// Toast display names per copy incarnation, from the device's own records.
    displays: BTreeMap<Owner, ToastNames>,
    /// Replies to authenticated requests only, keyed by the authenticated
    /// identity and request ID: IDs are only unique per copy.
    replies: VecDeque<(ReplyKey, wire::GatewayReply)>,
}

#[derive(Debug, Clone, PartialEq)]
struct ReplyKey {
    session: wire::Session,
    scope: wire::Scope,
    owner_fence: wire::Counter,
    request_id: String,
}

#[derive(Debug, Clone, PartialEq)]
struct ToastNames {
    mod_name: String,
    host_name: String,
}

fn failed(request_id: &str, reason: GatewayFailure) -> wire::GatewayReply {
    wire::GatewayReply {
        request_id: request_id.into(),
        result: wire::GatewayResult::Failed { reason },
    }
}

impl Gateway {
    pub fn new(policy: GatewayPolicy) -> Self {
        Self {
            policy,
            operating: false,
            overlays: BTreeMap::new(),
            toasts: VecDeque::new(),
            last_toast_ms: None,
            stopped: BTreeSet::new(),
            displays: BTreeMap::new(),
            replies: VecDeque::new(),
        }
    }
    /// Whether this connection is the operating one. Background execution
    /// never is.
    pub fn set_operating(&mut self, operating: bool) {
        self.operating = operating;
    }
    pub fn set_policy(&mut self, policy: GatewayPolicy) {
        self.policy = policy;
    }
    /// The names a toast of this copy incarnation shows. Both come from the
    /// device's own records, never from the runner: `mod_name` from the
    /// approved, hash-pinned mod record (or a label the user gave it) and
    /// `host_name` from the host the device verified over SSH. `copy` only
    /// identifies the incarnation. A toast's own `mod_name` / `host_name`
    /// are replaced by these.
    pub fn register_copy(&mut self, copy: &wire::CopyInfo, mod_name: String, host_name: String) {
        self.displays.insert(
            Owner {
                copy: copy.id.clone(),
                incarnation: copy.incarnation.clone(),
            },
            ToastNames {
                mod_name,
                host_name,
            },
        );
    }
    /// After Pause or Unload, the copy's later requests are cancelled and its
    /// overlays and toasts close.
    pub fn stop_copy(&mut self, copy: &str, incarnation: &wire::Id128) -> Vec<String> {
        self.stopped.insert((copy.into(), incarnation.clone()));
        let owner = Owner {
            copy: copy.into(),
            incarnation: incarnation.clone(),
        };
        let closed: Vec<String> = self
            .overlays
            .iter()
            .filter(|(_, o)| **o == owner)
            .map(|(id, _)| id.clone())
            .collect();
        for id in &closed {
            self.overlays.remove(id);
            self.toasts.retain(|t| t != id);
        }
        // A resent request of the stopped copy is decided again: cancelled.
        self.replies.retain(|(key, _)| {
            !matches!(&key.scope, wire::Scope::Copy { id, incarnation: i }
                if *id == owner.copy && *i == owner.incarnation)
        });
        closed
    }
    /// The user or a timer dismissed a toast.
    pub fn dismiss_toast(&mut self, overlay_id: &str) {
        self.toasts.retain(|t| t != overlay_id);
        self.overlays.remove(overlay_id);
    }
    pub fn open_overlays(&self) -> usize {
        self.overlays.len()
    }

    /// Route one request received on `context` (the copy's authenticated
    /// receiving identity) at monotonic time `now_ms`.
    pub fn handle(
        &mut self,
        request: &wire::Envelope<wire::GatewayRequest>,
        context: &wire::ReceiveContext,
        now_ms: u64,
    ) -> (wire::GatewayReply, Option<GatewayEffect>) {
        let id = request.body.request_id.clone();
        // Authenticate before the replay cache, so an unauthenticated message
        // can neither read a remembered reply nor fill the cache.
        if let Err(reason) = authenticate(request, context) {
            return (failed(&id, reason), None);
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
        let (reply, effect) = match self.decide(request, now_ms) {
            Ok(effect) => (
                wire::GatewayReply {
                    request_id: id.clone(),
                    result: wire::GatewayResult::Ok {},
                },
                Some(effect),
            ),
            Err(reason) => (failed(&id, reason), None),
        };
        if self.replies.len() == REMEMBERED_REPLIES {
            self.replies.pop_front();
        }
        self.replies.push_back((key, reply.clone()));
        (reply, effect)
    }

    /// Called only after `authenticate`.
    fn decide(
        &mut self,
        request: &wire::Envelope<wire::GatewayRequest>,
        now_ms: u64,
    ) -> Result<GatewayEffect, GatewayFailure> {
        let body = &request.body;
        let owner = Owner {
            copy: body.target.copy.clone(),
            incarnation: body.target.incarnation.clone(),
        };
        if self
            .stopped
            .contains(&(owner.copy.clone(), owner.incarnation.clone()))
        {
            return Err(GatewayFailure::Cancelled);
        }
        if !self.operating {
            return Err(GatewayFailure::NoOperatingConnection);
        }
        if body.policy_rev != self.policy.rev {
            return Err(GatewayFailure::PolicyDenied);
        }
        match &body.operation {
            GatewayOperation::Run { command, args } => {
                if !self.policy.commands.contains(command) {
                    return Err(GatewayFailure::PolicyDenied);
                }
                Ok(GatewayEffect::Run {
                    command: command.clone(),
                    args: args.clone(),
                })
            }
            GatewayOperation::SetAgentOrder { agents } => {
                if !self.policy.agent_order {
                    return Err(GatewayFailure::PolicyDenied);
                }
                Ok(GatewayEffect::SetAgentOrder {
                    agents: agents.clone(),
                })
            }
            GatewayOperation::OpenOverlay { overlay } => {
                // A toast goes only through notify and its limits: the wire
                // validation in `authenticate` already refused a toast here.
                self.claim(overlay, &owner)?;
                self.overlays.insert(overlay.id.clone(), owner);
                Ok(GatewayEffect::OpenOverlay(overlay.clone()))
            }
            GatewayOperation::CloseOverlay { overlay_id } => {
                // An unknown ID and another copy's overlay look the same.
                if self.overlays.get(overlay_id) != Some(&owner) {
                    return Err(GatewayFailure::NotOwned);
                }
                self.overlays.remove(overlay_id);
                self.toasts.retain(|t| t != overlay_id);
                Ok(GatewayEffect::CloseOverlay {
                    overlay_id: overlay_id.clone(),
                })
            }
            GatewayOperation::Notify { overlay } => {
                self.claim(overlay, &owner)?;
                if self
                    .last_toast_ms
                    .is_some_and(|last| now_ms.saturating_sub(last) < 1_000 / toast_rate())
                {
                    return Err(GatewayFailure::RateLimited);
                }
                if self.toasts.len() >= limits::TOASTS_VISIBLE {
                    return Err(GatewayFailure::LimitExceeded);
                }
                // Show the device's names for this copy, not the payload's.
                let names = self
                    .displays
                    .get(&owner)
                    .ok_or(GatewayFailure::InvalidRequest)?;
                let mut shown = overlay.clone();
                shown.toast = Some(wire::ToastContent {
                    message: overlay
                        .toast
                        .as_ref()
                        .map(|t| t.message.clone())
                        .ok_or(GatewayFailure::InvalidRequest)?,
                    mod_name: names.mod_name.clone(),
                    host_name: names.host_name.clone(),
                });
                self.last_toast_ms = Some(now_ms);
                self.toasts.push_back(overlay.id.clone());
                self.overlays.insert(overlay.id.clone(), owner);
                Ok(GatewayEffect::ShowToast(shown))
            }
        }
    }
    /// The overlay must name the requesting copy incarnation, and an open ID
    /// cannot be taken over.
    fn claim(&self, overlay: &wire::Overlay, owner: &Owner) -> Result<(), GatewayFailure> {
        if overlay.copy != owner.copy || overlay.incarnation != owner.incarnation {
            return Err(GatewayFailure::NotOwned);
        }
        if self.overlays.contains_key(&overlay.id) {
            return Err(GatewayFailure::NotOwned);
        }
        Ok(())
    }
}

/// A message from an older incarnation, session or owner, or a body target
/// that is not the authenticated copy, is stale; a malformed one is invalid.
fn authenticate(
    request: &wire::Envelope<wire::GatewayRequest>,
    context: &wire::ReceiveContext,
) -> Result<(), GatewayFailure> {
    match context.validate(request) {
        Ok(()) => {}
        Err(wire::Error::Stale(_)) => return Err(GatewayFailure::StaleTarget),
        Err(_) => return Err(GatewayFailure::InvalidRequest),
    }
    request
        .body
        .target
        .validate_against(context)
        .map_err(|_| GatewayFailure::StaleTarget)
}

fn toast_rate() -> u64 {
    limits::TOASTS_PER_SECOND.max(1) as u64
}
