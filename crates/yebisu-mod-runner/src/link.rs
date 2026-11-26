//! One copy's control link: stamps outgoing envelopes with the session,
//! copy scope and owner fence, and authenticates incoming ones against the
//! same identity under the negotiated control size.
use crate::Result;
use yebisu_mod_wire as wire;

pub struct CopyLink {
    context: wire::ReceiveContext,
    limits: wire::NegotiatedLimits,
    seq: u64,
}

impl CopyLink {
    pub fn new(context: wire::ReceiveContext, limits: wire::NegotiatedLimits) -> Result<Self> {
        limits.validate()?;
        Ok(Self {
            context,
            limits,
            seq: 0,
        })
    }
    pub fn context(&self) -> &wire::ReceiveContext {
        &self.context
    }
    pub fn limits(&self) -> &wire::NegotiatedLimits {
        &self.limits
    }
    /// The copy this link serves, as a gateway target.
    pub fn target(&self) -> Result<wire::GatewayTarget> {
        match &self.context.scope {
            wire::Scope::Copy { id, incarnation } => Ok(wire::GatewayTarget {
                boot: self.context.session.boot.clone(),
                copy: id.clone(),
                incarnation: incarnation.clone(),
            }),
            wire::Scope::Session {} => Err(wire::Error::Invalid("link scope").into()),
        }
    }
    pub fn envelope<T: wire::Payload>(&mut self, body: T) -> wire::Envelope<T> {
        self.seq += 1;
        wire::Envelope::new(
            self.context.session.clone(),
            self.context.scope.clone(),
            self.context.owner_fence,
            format!("runner-{}", self.seq),
            wire::Counter(self.seq),
            body,
        )
    }
    /// Decode one control message from the renderer and authenticate it.
    pub fn receive<T: wire::Payload>(&self, frame: &wire::Frame) -> Result<wire::Envelope<T>> {
        let message = decode_control::<T>(frame, &self.limits)?;
        self.context.validate(&message)?;
        Ok(message)
    }
}

/// One control message as a DATA frame on channel 0, within the negotiated size.
pub fn encode_control<T: wire::Payload>(
    message: &wire::Envelope<T>,
    limits: &wire::NegotiatedLimits,
) -> Result<wire::Frame> {
    let payload = wire::encode(message)?;
    limits.check_control(payload.len())?;
    Ok(wire::Frame {
        channel: 0,
        kind: wire::FrameKind::Data,
        payload,
    })
}
/// Check the frame against the negotiated control size before decoding.
pub fn decode_control<T: wire::Payload>(
    frame: &wire::Frame,
    limits: &wire::NegotiatedLimits,
) -> Result<wire::Envelope<T>> {
    if frame.channel != 0 || frame.kind != wire::FrameKind::Data {
        return Err(wire::Error::Invalid("control frame").into());
    }
    limits.check_control(frame.payload.len())?;
    Ok(wire::decode(&frame.payload)?)
}
