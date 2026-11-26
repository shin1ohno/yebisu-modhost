//! GPUI-independent protocol core of the option-E runner: the handshake
//! answer, the per-copy link that stamps and authenticates envelopes, and the
//! clients that route command gateway and device requests to the renderer.
//! The `yebisu-modhost` binary drives it from the mod's host API.
mod error;
mod gateway;
mod handshake;
mod link;

pub use error::{Error, Result};
pub use gateway::{GatewayClient, GatewaySend};
pub use handshake::{ServeIdentity, answer_hello};
pub use link::{CopyLink, decode_control, encode_control};
