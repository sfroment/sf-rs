use std::fmt;

use serde::{Deserialize, Serialize};
use sf_peer_id::PeerID;

use crate::PeerEvent;

/// Represents an event from peers to the WebSocket server
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PeerRequest {
    /// Keep-alive message to maintain the connection
    KeepAlive,

    /// Forward/signal event to be sent to another peer
    Forward {
        /// The ID of the peer that sent the forward
        from_peer_id: PeerID,
        /// The ID of the peer to forward the data to
        to_peer_id: Option<PeerID>,
        /// The data to be forwarded (owned JSON string slice)
        data: PeerEvent,
    },
}

impl PeerRequest {
    pub fn new_forward(from_peer_id: PeerID, to_peer_id: Option<PeerID>, data: PeerEvent) -> Self {
        Self::Forward {
            from_peer_id,
            to_peer_id,
            data,
        }
    }
}

#[cfg(target_arch = "wasm32")]
impl From<PeerRequest> for wasm_bindgen::JsValue {
    fn from(p: PeerRequest) -> Self {
        serde_wasm_bindgen::to_value(&p).unwrap()
    }
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
            ) => f1 == f2 && t1 == t2 && d1 == d2,
            _ => false,
        }
    }
}

impl fmt::Display for PeerRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::KeepAlive => write!(f, "KeepAlive"),
            Self::Forward {
                from_peer_id,
                to_peer_id,
                data,
            } => {
                write!(
                    f,
                    "Forward {{ from_peer_id: {from_peer_id}, to_peer_id: {to_peer_id:?}, data: {data:?} }}"
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn test_peer_request_display() {
        let from_peer_id = PeerID::from_str("01").unwrap();
        let to_peer_id = Some(PeerID::from_str("02").unwrap());
        let data = PeerEvent::NewPeer {
            peer_id: PeerID::from_str("01").unwrap(),
        };
        let forward = PeerRequest::new_forward(from_peer_id, to_peer_id, data.clone());

        let expected = format!(
            "Forward {{ from_peer_id: {from_peer_id}, to_peer_id: {to_peer_id:?}, data: {data:?} }}"
        );
        assert_eq!(format!("{forward}"), expected);
    }

    #[test]
    fn test_new_forward() {
        let from_peer_id = PeerID::from_str("01").unwrap();
        let to_peer_id = Some(PeerID::from_str("02").unwrap());
        let data = PeerEvent::NewPeer {
            peer_id: PeerID::from_str("03").unwrap(),
        };

        let forward = PeerRequest::new_forward(from_peer_id, to_peer_id, data.clone());

        match forward {
            PeerRequest::Forward {
                from_peer_id: f,
                to_peer_id: t,
                data: d,
            } => {
                assert_eq!(f, from_peer_id);
                assert_eq!(t, to_peer_id);
                assert_eq!(d, data);
            }
            _ => panic!("Expected Forward variant"),
        }
    }
}
