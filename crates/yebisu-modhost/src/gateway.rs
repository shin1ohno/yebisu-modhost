//! The mod's A13 gateway calls (`run`, `setAgentOrder`, `openOverlay`,
//! `closeOverlay`, `notify`) routed through the runner core. This offline
//! runner has no operating connection, so every call fails with the explicit
//! `no_operating_connection`, as A13 requires, instead of an ad-hoc error.
use gpui_shell::{HostArguments, HostError, HostModule, HostValue};
use std::{cell::RefCell, rc::Rc};
use yebisu_mod_runner::{CopyLink, GatewayClient, GatewaySend};
use yebisu_mod_wire::{Counter, GatewayFailure, GatewayOperation};

pub struct HostGateway {
    client: GatewayClient,
    /// The operating connection's link to this copy; none offline.
    link: Option<CopyLink>,
}

impl HostGateway {
    pub fn offline() -> Self {
        Self {
            client: GatewayClient::new(Counter(1)),
            link: None,
        }
    }
    /// Send `operation`, or settle it at once. A call whose arguments have no
    /// wire mapping yet (`None`) still reports the missing connection first.
    fn call(&mut self, operation: Option<GatewayOperation>) -> Result<(), GatewayFailure> {
        let Some(operation) = operation else {
            return Err(if self.link.is_none() {
                GatewayFailure::NoOperatingConnection
            } else {
                GatewayFailure::Unsupported
            });
        };
        match self.client.request(self.link.as_mut(), operation) {
            // The reply settles through GatewayClient::receive on the live
            // transport; the herdr-app calls themselves do not wait for it.
            Ok(GatewaySend::Sent { .. }) => Ok(()),
            Ok(GatewaySend::Failed(failure)) => Err(failure),
            Err(_) => Err(GatewayFailure::InvalidRequest),
        }
    }
}

/// The A13 failure name, e.g. `no_operating_connection`.
pub fn failure_name(failure: GatewayFailure) -> String {
    match serde_json::to_value(failure) {
        Ok(serde_json::Value::String(name)) => name,
        _ => format!("{failure:?}"),
    }
}

fn strings(args: &HostArguments, index: usize) -> Result<Vec<String>, HostError> {
    match args.value(index)? {
        HostValue::Array(items) => items
            .iter()
            .map(|v| match v {
                HostValue::Str(s) => Ok(s.clone()),
                _ => Err(HostError::new("expected a list of strings")),
            })
            .collect(),
        _ => Err(HostError::new("expected a list of strings")),
    }
}

/// Map each call's arguments to a wire operation where the shapes agree.
/// `run`'s herdr-app arguments are a JSON object while the wire carries a
/// string list, and `openOverlay` / `notify` describe a mod or layout rather
/// than a wire Overlay; those mappings belong to the interactive adapter.
fn operation(name: &str, args: &HostArguments) -> Result<Option<GatewayOperation>, HostError> {
    Ok(match name {
        "setAgentOrder" => Some(GatewayOperation::SetAgentOrder {
            agents: strings(args, 0)?,
        }),
        "closeOverlay" => Some(GatewayOperation::CloseOverlay {
            overlay_id: args.string(0)?.to_owned(),
        }),
        "run" => match args.get(1) {
            None | Some(HostValue::Null) => Some(GatewayOperation::Run {
                command: args.string(0)?.to_owned(),
                args: vec![],
            }),
            Some(_) => None,
        },
        _ => None,
    })
}

pub fn install(mut module: HostModule, gateway: Rc<RefCell<HostGateway>>) -> HostModule {
    for name in [
        "run",
        "setAgentOrder",
        "openOverlay",
        "closeOverlay",
        "notify",
    ] {
        let gateway = gateway.clone();
        module = module.function(name, move |args| {
            let operation = operation(name, args)?;
            gateway
                .borrow_mut()
                .call(operation)
                .map(|()| HostValue::Null)
                .map_err(|failure| HostError::new(format!("{name}: {}", failure_name(failure))))
        });
    }
    module
}
