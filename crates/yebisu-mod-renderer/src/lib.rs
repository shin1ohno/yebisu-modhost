//! GPUI-independent receiving core of the option-E renderer.
//!
//! The iPad renderer (E-1, in yebisu) draws what this crate accepts. This crate
//! owns the receiving rules the wire README assigns to the renderer: the
//! handshake, atomic publication adoption under the negotiated limits, row and
//! asset delivery against the adopted copy with list/copy/host/application
//! resource scopes, and (in later modules) the command gateway and device
//! permissions. It never executes mod code and never draws.
mod adopt;
mod assets;
mod error;
mod handshake;
mod rows;

pub use adopt::{CopyReceiver, Inflate, Uncompressed};
pub use assets::SharedScopes;
pub use error::{Error, Result};
pub use handshake::{ClientHandshake, Negotiated};
