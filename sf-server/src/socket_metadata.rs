use sf_peer_id::PeerID;
use std::{fmt, net::SocketAddr};

#[derive(Clone)]
pub struct SocketMetadata {
    pub origin: SocketAddr,
    pub peer_id: PeerID,
}

impl SocketMetadata {
    pub fn new(origin: SocketAddr, peer_id: PeerID) -> Self {
        Self { origin, peer_id }
    }
}

impl fmt::Debug for SocketMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SocketMetadata")
            .field("origin", &self.origin)
            .field("peer_id", &self.peer_id)
            .finish()
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn test_new() {
        let origin = SocketAddr::from(([127, 0, 0, 1], 8080));
        let peer_id = PeerID::from_str("test_peer_id").unwrap();
        let metadata = SocketMetadata::new(origin, peer_id);

        assert_eq!(metadata.origin, origin);
        assert_eq!(metadata.peer_id, peer_id);
    }

    #[test]
    fn test_debug() {
        let origin = SocketAddr::from(([127, 0, 0, 1], 8080));
        let peer_id = PeerID::from_str("test_peer_id").unwrap();
        let metadata = SocketMetadata::new(origin, peer_id);

        assert_eq!(
            format!("{metadata:?}"),
            format!("SocketMetadata {{ origin: {origin:?}, peer_id: {peer_id:?} }}",)
        );
    }
}
