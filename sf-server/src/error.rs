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
}
