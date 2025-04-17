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

// Manually implement PartialEq
impl PartialEq for PeerRequest {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (PeerRequest::KeepAlive, PeerRequest::KeepAlive) => true,
            (
                PeerRequest::Forward {
                    from_peer_id: f1,
                    to_peer_id: t1,
                    data: d1,
                },
                PeerRequest::Forward {
                    from_peer_id: f2,
                    to_peer_id: t2,
                    data: d2,
                },
            ) => f1 == f2 && t1 == t2 && d1.get() == d2.get(),
            _ => false,
        }
    }
}

/// Represents an event from the WebSocket server to peers
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PeerEvent {
    /// A new peer has connected
    NewPeer { peer_id: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_request_keep_alive_serialization() {
        let keep_alive = PeerRequest::KeepAlive;
        let serialized = serde_json::to_string(&keep_alive).unwrap();
        assert_eq!(serialized, r#""keep_alive""#);

        let deserialized: PeerRequest = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, keep_alive);
    }

    #[test]
    fn test_peer_request_forward_serialization() {
        let data = serde_json::value::to_raw_value(r#"{"message":"hello"}"#).unwrap();
        let forward = PeerRequest::Forward {
            from_peer_id: Arc::new("peer1".to_string()),
            to_peer_id: Some("peer2".to_string()),
            data: Arc::from(data),
        };

        let serialized = serde_json::to_string(&forward).unwrap();
        let deserialized: PeerRequest = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, forward);
    }

    #[test]
    fn test_peer_request_equality() {
        assert_eq!(PeerRequest::KeepAlive, PeerRequest::KeepAlive);

        let data1 = serde_json::value::to_raw_value(r#"{"message":"hello"}"#).unwrap();
        let data2 = serde_json::value::to_raw_value(r#"{"message":"hello"}"#).unwrap();

        let forward1 = PeerRequest::Forward {
            from_peer_id: Arc::new("peer1".to_string()),
            to_peer_id: Some("peer2".to_string()),
            data: Arc::from(data1),
        };

        let forward2 = PeerRequest::Forward {
            from_peer_id: Arc::new("peer1".to_string()),
            to_peer_id: Some("peer2".to_string()),
            data: Arc::from(data2),
        };

        assert_eq!(forward1, forward2);

        let data3 = serde_json::value::to_raw_value(r#"{"message":"different"}"#).unwrap();
        let forward3 = PeerRequest::Forward {
            from_peer_id: Arc::new("peer1".to_string()),
            to_peer_id: Some("peer2".to_string()),
            data: Arc::from(data3),
        };

        assert_ne!(forward1, forward3);

        assert_ne!(PeerRequest::KeepAlive, forward1);

        let from_id1 = Arc::new("peer1".to_string());
        let from_id2 = Arc::new("peer2".to_string());
        let data_same = serde_json::value::to_raw_value(r#"{"message":"hello"}"#).unwrap();

        let forward_a = PeerRequest::Forward {
            from_peer_id: from_id1.clone(),
            to_peer_id: Some("target".to_string()),
            data: Arc::from(data_same.clone()),
        };

        let forward_b = PeerRequest::Forward {
            from_peer_id: from_id2,
            to_peer_id: Some("target".to_string()),
            data: Arc::from(data_same),
        };

        assert_ne!(forward_a, forward_b);
    }

    #[test]
    fn test_peer_event_serialization() {
        let new_peer = PeerEvent::NewPeer {
            peer_id: "peer1".to_string(),
        };

        let serialized = serde_json::to_string(&new_peer).unwrap();
        assert_eq!(serialized, r#"{"new_peer":{"peer_id":"peer1"}}"#);

        let deserialized: PeerEvent = serde_json::from_str(&serialized).unwrap();

        match (new_peer, deserialized) {
            (PeerEvent::NewPeer { peer_id: id1 }, PeerEvent::NewPeer { peer_id: id2 }) => {
                assert_eq!(id1, id2)
            }
        }
    }
}
