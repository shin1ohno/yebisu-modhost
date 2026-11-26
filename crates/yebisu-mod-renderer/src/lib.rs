//! GPUI-independent receiving core of the option-E renderer.
//!
//! The iPad renderer (E-1, in yebisu) draws what this crate accepts. This crate
//! owns the receiving rules the wire README assigns to the renderer: the
//! handshake, atomic publication adoption under the negotiated limits, row and
//! asset delivery against the adopted copy with list/copy/host/application
//! resource scopes, the A13 command gateway and the A12 device permissions. It never executes mod code
//! and never draws.
mod adopt;
mod assets;
mod error;
mod gateway;
mod handshake;
mod permissions;
mod rows;

pub use adopt::{CopyReceiver, Inflate, Uncompressed};
pub use assets::SharedScopes;
pub use error::{Error, Result};
pub use gateway::{Gateway, GatewayEffect, GatewayPolicy};
pub use handshake::{ClientHandshake, Negotiated};
pub use permissions::{DeviceEffect, DevicePermissions};
