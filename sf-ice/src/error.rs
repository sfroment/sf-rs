#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("bind: {0}")]
    Bind(std::io::Error),

    /// Error on serve
    #[error("Serve error: {0}")]
    Serve(std::io::Error),
}
