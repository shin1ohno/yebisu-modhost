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

#[derive(Debug, Clone, PartialEq, Eq)]
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
    /// Keyed by the requesting scope and request ID: IDs are only unique per copy.
    replies: VecDeque<((wire::Scope, String), wire::GatewayReply)>,
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
        let key = (request.scope.clone(), id.clone());
        if let Some((_, reply)) = self.replies.iter().find(|(k, _)| *k == key) {
            return (reply.clone(), None);
        }
        let (reply, effect) = match self.decide(request, context, now_ms) {
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

    fn decide(
        &mut self,
        request: &wire::Envelope<wire::GatewayRequest>,
        context: &wire::ReceiveContext,
        now_ms: u64,
    ) -> Result<GatewayEffect, GatewayFailure> {
        // A message from an older incarnation, session or owner is stale; one
        // that is malformed is invalid.
        match context.validate(request) {
            Ok(()) => {}
            Err(wire::Error::Stale(_)) => return Err(GatewayFailure::StaleTarget),
            Err(_) => return Err(GatewayFailure::InvalidRequest),
        }
        let body = &request.body;
        body.target
            .validate_against(context)
            .map_err(|_| GatewayFailure::StaleTarget)?;
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
                self.last_toast_ms = Some(now_ms);
                self.toasts.push_back(overlay.id.clone());
                self.overlays.insert(overlay.id.clone(), owner);
                Ok(GatewayEffect::ShowToast(overlay.clone()))
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

fn toast_rate() -> u64 {
    limits::TOASTS_PER_SECOND.max(1) as u64
}
