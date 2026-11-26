use yebisu_mod_wire as wire;

/// Why the runner refused a message or could not send one.
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// The wire contract or a negotiated limit rejected the message.
    Wire(wire::Error),
    /// A reply named a request this runner never sent or already settled.
    UnknownRequest(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Wire(e) => write!(f, "wire: {e}"),
            Self::UnknownRequest(id) => write!(f, "unknown request {id}"),
        }
    }
}
impl std::error::Error for Error {}
impl From<wire::Error> for Error {
    fn from(e: wire::Error) -> Self {
        Self::Wire(e)
    }
}
pub type Result<T> = std::result::Result<T, Error>;
