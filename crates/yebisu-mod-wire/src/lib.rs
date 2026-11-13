//! GPUI-independent option-E protocol. Decode, validate against negotiated
//! tables and current context, then atomically accept a complete publication.
mod codec;
mod framing;
pub mod limits;
mod message;
mod primitive;
mod snapshot;
mod validate;
pub use codec::*;
pub use framing::*;
pub use message::*;
pub use primitive::*;
pub use snapshot::*;
pub use validate::*;
pub const PROTOCOL: u32 = 1;
pub const SCHEMA: &str = "e1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Io(String),
    Json(String),
    Limit(&'static str),
    Invalid(&'static str),
    Unknown(&'static str),
    Stale(&'static str),
    DigestMismatch,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e.to_string())
    }
}
pub type Result<T> = std::result::Result<T, Error>;
pub trait Payload: serde::Serialize + serde::de::DeserializeOwned {
    const KIND: MessageKind;
    fn validate_wire(&self) -> Result<()> {
        Ok(())
    }
}
