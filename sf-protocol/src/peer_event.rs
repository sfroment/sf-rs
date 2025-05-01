use serde::{Deserialize, Serialize};
use sf_peer_id::PeerID;

use crate::session_description::SessionDescription;

/// Represents an event from the WebSocket server to peers
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PeerEvent {
    /// A new peer has connected
    NewPeer { peer_id: PeerID },
    /// Message from a peer
    Message { peer_id: PeerID, message: String },

    /// WebRTC offer
    WebRtcOffer {
        peer_id: PeerID,
        session_description: SessionDescription,
    },
}
