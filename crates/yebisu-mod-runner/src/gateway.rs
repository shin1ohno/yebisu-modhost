//! The runner's side of the A13 command gateway: the mod's host calls become
//! GatewayRequests with an explicit target and Policy revision, and each
//! GatewayReply is routed back to the call that is waiting for it.
use crate::{CopyLink, Error, Result};
use std::collections::BTreeMap;
use yebisu_mod_wire as wire;
use yebisu_mod_wire::{GatewayFailure, GatewayOperation};

/// The outcome of asking for a gateway operation.
#[derive(Debug, Clone, PartialEq)]
pub enum GatewaySend {
    /// Send this envelope; the reply settles `request_id`.
    Sent {
        request_id: String,
        envelope: Box<wire::Envelope<wire::GatewayRequest>>,
    },
    /// Settled at once without contacting any device.
    Failed(GatewayFailure),
}

/// Pending requests of one copy. With no operating connection every request
/// fails at once with `no_operating_connection`, including background work.
pub struct GatewayClient {
    policy_rev: wire::Counter,
    next: u64,
    /// Each pending request remembers the copy scope it was sent in.
    pending: BTreeMap<String, (wire::Scope, GatewayOperation)>,
}

impl GatewayClient {
    pub fn new(policy_rev: wire::Counter) -> Self {
        Self {
            policy_rev,
            next: 0,
            pending: BTreeMap::new(),
        }
    }
    pub fn pending(&self) -> usize {
        self.pending.len()
    }
    /// The Policy revision the runner currently executes under.
    pub fn set_policy_rev(&mut self, rev: wire::Counter) {
        self.policy_rev = rev;
    }
    pub fn request(
        &mut self,
        link: Option<&mut CopyLink>,
        operation: GatewayOperation,
    ) -> Result<GatewaySend> {
        let Some(link) = link else {
            return Ok(GatewaySend::Failed(GatewayFailure::NoOperatingConnection));
        };
        self.next += 1;
        let request = wire::GatewayRequest {
            request_id: format!("gateway-{}", self.next),
            target: link.target()?,
            policy_rev: self.policy_rev,
            operation,
        };
        if request.validate().is_err() {
            return Ok(GatewaySend::Failed(GatewayFailure::InvalidRequest));
        }
        let envelope = link.envelope(request);
        // A request too large for the negotiated control size is never sent.
        if crate::encode_control(&envelope, link.limits()).is_err() {
            return Ok(GatewaySend::Failed(GatewayFailure::LimitExceeded));
        }
        let request_id = envelope.body.request_id.clone();
        self.pending.insert(
            request_id.clone(),
            (envelope.scope.clone(), envelope.body.operation.clone()),
        );
        Ok(GatewaySend::Sent {
            request_id,
            envelope: Box::new(envelope),
        })
    }
    /// Route a reply received on `link`: authenticate it, then settle exactly
    /// the request it names. Unknown and repeated replies are refused.
    pub fn receive(
        &mut self,
        link: &CopyLink,
        frame: &wire::Frame,
    ) -> Result<(String, wire::GatewayResult)> {
        let reply: wire::Envelope<wire::GatewayReply> = link.receive(frame)?;
        let id = reply.body.request_id;
        // Only a reply in the scope the request was sent in settles it.
        if self
            .pending
            .get(&id)
            .is_none_or(|(scope, _)| *scope != reply.scope)
        {
            return Err(Error::UnknownRequest(id));
        }
        self.pending.remove(&id);
        Ok((id, reply.body.result))
    }
    /// The connection ended: every pending request fails with `cancelled`.
    pub fn disconnect(&mut self) -> Vec<(String, GatewayFailure)> {
        std::mem::take(&mut self.pending)
            .into_keys()
            .map(|id| (id, GatewayFailure::Cancelled))
            .collect()
    }
}
