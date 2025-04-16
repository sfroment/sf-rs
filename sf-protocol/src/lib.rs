use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use std::sync::Arc;

/// Represents an event from peers to the WebSocket server
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PeerRequest {
    /// Keep-alive message to maintain the connection
    KeepAlive,

    /// Forward/signal event to be sent to another peer
    Forward {
        /// The ID of the peer that sent the forward
        from_peer_id: Arc<String>,
        /// The ID of the peer to forward the data to
        to_peer_id: Option<String>,
        /// The data to be forwarded (owned JSON string slice)
        data: Arc<RawValue>,
    },
}

/// Represents an event from the WebSocket server to peers
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PeerEvent {
    /// A new peer has connected
    NewPeer { peer_id: String },
}
