//! The runner's side of the A12 permission hand-off. A permission arrives in
//! an Action's `ActionData.permissions`; the mod's callback must retain it
//! explicitly while the Action is current, and async work later spends the
//! retained value (once: it is moved into the request) in a DeviceRequest.
use crate::{CopyLink, Error, Result};
use std::collections::BTreeMap;
use yebisu_mod_wire as wire;
use yebisu_mod_wire::DeviceOperation;

/// A permission the mod retained from one event. Not `Clone`: sending a
/// request consumes it.
#[derive(Debug, PartialEq)]
pub struct RetainedPermission {
    permission: wire::DevicePermission,
    event_id: String,
    scope: wire::Scope,
}
impl RetainedPermission {
    pub fn operation(&self) -> DeviceOperation {
        self.permission.operation
    }
}

/// Why a device request was not sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceRefusal {
    NoOperatingConnection,
    /// The permission was retained from another copy's event.
    OtherCopy,
    /// `open_url` needs an HTTPS URL without credentials.
    InvalidUrl,
    /// The request does not fit the wire contract or the control size.
    Invalid,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DeviceSend {
    Sent {
        request_id: String,
        envelope: Box<wire::Envelope<wire::DeviceRequest>>,
    },
    Refused(DeviceRefusal),
}

#[derive(Default)]
pub struct DeviceClient {
    current: Option<(wire::Scope, wire::Action)>,
    next: u64,
    pending: BTreeMap<String, wire::Scope>,
}

impl DeviceClient {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn pending(&self) -> usize {
        self.pending.len()
    }
    /// Receive an Action: authenticate it, apply the negotiated limits, and
    /// make it the current event whose permissions may be retained.
    pub fn on_action(&mut self, link: &CopyLink, frame: &wire::Frame) -> Result<wire::Action> {
        let action: wire::Envelope<wire::Action> = link.receive(frame)?;
        link.limits().check_action(&action.body)?;
        self.current = Some((action.scope, action.body.clone()));
        Ok(action.body)
    }
    /// Retain this event's permission for `operation`. Each permission can be
    /// retained once, and only while its Action is current.
    pub fn retain(&mut self, operation: DeviceOperation) -> Option<RetainedPermission> {
        let (scope, action) = self.current.as_mut()?;
        let index = action
            .data
            .permissions
            .iter()
            .position(|p| p.operation == operation)?;
        Some(RetainedPermission {
            permission: action.data.permissions.remove(index),
            event_id: action.event_id.clone(),
            scope: scope.clone(),
        })
    }
    /// The callback returned: permissions it did not retain are gone.
    pub fn end_action(&mut self) {
        self.current = None;
    }
    /// Spend a retained permission. The `open_url` value goes on the wire as
    /// `normalize_url` output, so the device confirms exactly that host.
    pub fn request(
        &mut self,
        link: Option<&mut CopyLink>,
        retained: RetainedPermission,
        value: &str,
    ) -> Result<DeviceSend> {
        let Some(link) = link else {
            return Ok(DeviceSend::Refused(DeviceRefusal::NoOperatingConnection));
        };
        if retained.scope != link.context().scope {
            return Ok(DeviceSend::Refused(DeviceRefusal::OtherCopy));
        }
        let value = match retained.permission.operation {
            DeviceOperation::OpenUrl => match wire::normalize_url(value) {
                Ok(url) => url,
                Err(_) => return Ok(DeviceSend::Refused(DeviceRefusal::InvalidUrl)),
            },
            DeviceOperation::ClipboardWrite => value.to_owned(),
        };
        self.next += 1;
        let request = wire::DeviceRequest {
            request_id: format!("device-{}", self.next),
            permission: retained.permission.permission,
            event_id: retained.event_id,
            operation: retained.permission.operation,
            policy_rev: retained.permission.policy_rev,
            value,
        };
        if request.validate().is_err() {
            return Ok(DeviceSend::Refused(DeviceRefusal::Invalid));
        }
        let envelope = link.envelope(request);
        if crate::encode_control(&envelope, link.limits()).is_err() {
            return Ok(DeviceSend::Refused(DeviceRefusal::Invalid));
        }
        let request_id = envelope.body.request_id.clone();
        self.pending
            .insert(request_id.clone(), envelope.scope.clone());
        Ok(DeviceSend::Sent {
            request_id,
            envelope: Box::new(envelope),
        })
    }
    /// Route a DeviceReply received on `link` to the one request it names,
    /// only in the scope that request was sent in.
    pub fn receive(
        &mut self,
        link: &CopyLink,
        frame: &wire::Frame,
    ) -> Result<(String, wire::DeviceResult)> {
        let reply: wire::Envelope<wire::DeviceReply> = link.receive(frame)?;
        let id = reply.body.request_id;
        if self.pending.get(&id) != Some(&reply.scope) {
            return Err(Error::UnknownRequest(id));
        }
        self.pending.remove(&id);
        Ok((id, reply.body.result))
    }
    /// The connection ended: pending requests will get no reply.
    pub fn disconnect(&mut self) -> Vec<String> {
        self.current = None;
        std::mem::take(&mut self.pending).into_keys().collect()
    }
}
