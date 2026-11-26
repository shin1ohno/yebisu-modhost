//! The runner's answer to a renderer's Hello.
use crate::Result;
use yebisu_mod_wire as wire;

/// What the runner states about itself and the copies it serves.
#[derive(Debug, Clone)]
pub struct ServeIdentity {
    pub serve: wire::ServeVersion,
    pub boot: wire::Id128,
    pub epoch: wire::Id128,
    /// The digests of this runner's reflection tables.
    pub digests: wire::Digests,
    /// Features the renderer must offer.
    pub required: Vec<String>,
    /// The values this runner can accept; never above the A8 ceilings.
    pub limits: wire::NegotiatedLimits,
    pub copies: Vec<wire::CopyInfo>,
}

/// Check the renderer's protocol, schema, exact table digests and required
/// features, then acknowledge the smaller of both sides' limits.
pub fn answer_hello(hello: &wire::Hello, serve: &ServeIdentity) -> Result<wire::HelloAck> {
    hello.validate_compatibility(&serve.digests, &serve.required)?;
    Ok(wire::HelloAck {
        nonce: hello.nonce.clone(),
        protocol: wire::PROTOCOL,
        serve: serve.serve.clone(),
        boot: serve.boot.clone(),
        epoch: serve.epoch.clone(),
        limits: serve.limits.intersect(&hello.limits)?,
        copies: serve.copies.clone(),
    })
}
