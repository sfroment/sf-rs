pub mod futures;

#[derive(Debug)]
pub enum WebRTCError {}

impl std::fmt::Display for WebRTCError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WebRTCError")
    }
}

impl std::error::Error for WebRTCError {}
