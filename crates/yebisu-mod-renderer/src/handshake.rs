//! The renderer's side of Hello / HelloAck.
use crate::Result;
use yebisu_mod_wire as wire;

/// What this renderer offers before the runner answers.
#[derive(Debug, Clone)]
pub struct ClientHandshake {
    hello: wire::Hello,
}

/// The session values both sides now apply.
#[derive(Debug, Clone, PartialEq)]
pub struct Negotiated {
    pub boot: wire::Id128,
    pub epoch: wire::Id128,
    pub limits: wire::NegotiatedLimits,
    pub copies: Vec<wire::CopyInfo>,
}

impl ClientHandshake {
    pub fn new(
        client: String,
        nonce: wire::Id128,
        digests: wire::Digests,
        features: Vec<String>,
        limits: wire::NegotiatedLimits,
    ) -> Result<Self> {
        let hello = wire::Hello {
            client,
            nonce,
            protocol: wire::PROTOCOL,
            schema: wire::SCHEMA.into(),
            digests,
            features,
            limits,
        };
        hello.validate()?;
        Ok(Self { hello })
    }
    pub fn hello(&self) -> &wire::Hello {
        &self.hello
    }
    /// Accept the runner's answer: same nonce and protocol, and no value above
    /// what this side offered. The acknowledged limits are the session's.
    pub fn accept(&self, ack: &wire::HelloAck) -> Result<Negotiated> {
        ack.validate_nonce(&self.hello.nonce)?;
        self.hello.limits.check_acknowledged(&ack.limits)?;
        Ok(Negotiated {
            boot: ack.boot.clone(),
            epoch: ack.epoch.clone(),
            limits: ack.limits.clone(),
            copies: ack.copies.clone(),
        })
    }
}

impl Negotiated {
    /// The receiving identity of one copy. `host` is the SSH-verified host
    /// fingerprint, never a value the runner reported about itself.
    pub fn context(
        &self,
        host: String,
        client: String,
        copy: &wire::CopyInfo,
        owner_fence: wire::Counter,
    ) -> Result<wire::ReceiveContext> {
        if !self.copies.contains(copy) {
            return Err(crate::Error::UnknownCopy);
        }
        Ok(wire::ReceiveContext {
            session: wire::Session {
                host,
                boot: self.boot.clone(),
                client,
                epoch: self.epoch.clone(),
            },
            scope: wire::Scope::Copy {
                id: copy.id.clone(),
                incarnation: copy.incarnation.clone(),
            },
            owner_fence,
        })
    }
}
