use yebisu_mod_wire as wire;

/// Why the renderer refused a message or a local request.
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// The wire contract or a negotiated limit rejected the message.
    Wire(wire::Error),
    /// No publication has been adopted for this copy yet.
    NotAdopted,
    /// A data frame or commit arrived with no transfer in progress.
    NoTransfer,
    /// A reply named a request this receiver never sent or already settled.
    UnknownRequest(String),
    /// A local request reused the ID of one still outstanding.
    DuplicateRequest(String),
    /// The requested list or asset is not part of the adopted copy.
    NotInAdoptedCopy(&'static str),
    /// The copy was not announced in this session's HelloAck.
    UnknownCopy,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Wire(e) => write!(f, "wire: {e}"),
            Self::NotAdopted => f.write_str("no adopted publication"),
            Self::NoTransfer => f.write_str("no transfer in progress"),
            Self::UnknownRequest(id) => write!(f, "unknown request {id}"),
            Self::DuplicateRequest(id) => write!(f, "duplicate request {id}"),
            Self::NotInAdoptedCopy(what) => write!(f, "{what} not in the adopted copy"),
            Self::UnknownCopy => f.write_str("copy not announced by HelloAck"),
        }
    }
}
impl std::error::Error for Error {}
impl From<wire::Error> for Error {
    fn from(e: wire::Error) -> Self {
        Self::Wire(e)
    }
}
impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::Wire(wire::Error::from(e))
    }
}
pub type Result<T> = std::result::Result<T, Error>;
