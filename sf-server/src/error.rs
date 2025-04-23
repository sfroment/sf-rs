#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("bind: {0}")]
    Bind(std::io::Error),

    /// Error on serve
    #[error("Serve error: {0}")]
    Serve(std::io::Error),

    /// Error on get peer not found
    #[error("Peer not found: {0}")]
    PeerNotFound(String),

    /// Error when trying to add a peer that already exists
    #[error("Peer already exists: {0}")]
    PeerAlreadyExists(String),

    /// The peer‑specific mpsc channel is closed, so the message could
    /// not be delivered.
    #[error("Send error: peer receiver has been dropped")]
    SendChannelClosed,
}

impl<T> From<tokio::sync::mpsc::error::SendError<T>> for Error {
    fn from(_: tokio::sync::mpsc::error::SendError<T>) -> Self {
        Error::SendChannelClosed
    }
}
