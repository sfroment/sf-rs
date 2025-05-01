mod peer_event;
mod peer_request;
mod rtc_sdp_wrapper;
mod session_description;

pub use peer_event::*;
pub use peer_request::*;

#[cfg(test)]
mod tests {
    use sf_peer_id::PeerID;
    use std::str::FromStr;

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
        let from_peer_id = PeerID::from_str("01").unwrap();
        println!("{from_peer_id}");
        let forward = PeerRequest::Forward {
            from_peer_id: PeerID::from_str("01").unwrap(),
            to_peer_id: Some(PeerID::from_str("02").unwrap()),
            data: PeerEvent::NewPeer {
                peer_id: PeerID::from_str("01").unwrap(),
            },
        };

        let serialized = serde_json::to_string(&forward).unwrap();
        let deserialized: PeerRequest = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, forward);
    }

    #[test]
    fn test_peer_request_equality() {
        assert_eq!(PeerRequest::KeepAlive, PeerRequest::KeepAlive);
        let forward1 = PeerRequest::Forward {
            from_peer_id: PeerID::from_str("01").unwrap(),
            to_peer_id: Some(PeerID::from_str("02").unwrap()),
            data: PeerEvent::NewPeer {
                peer_id: PeerID::from_str("01").unwrap(),
            },
        };

        let forward2 = PeerRequest::Forward {
            from_peer_id: PeerID::from_str("01").unwrap(),
            to_peer_id: Some(PeerID::from_str("02").unwrap()),
            data: PeerEvent::NewPeer {
                peer_id: PeerID::from_str("01").unwrap(),
            },
        };

        assert_eq!(forward1, forward2, "Forward messages should be equal");

        let forward3 = PeerRequest::Forward {
            from_peer_id: PeerID::from_str("01").unwrap(),
            to_peer_id: Some(PeerID::from_str("02").unwrap()),
            data: PeerEvent::NewPeer {
                peer_id: PeerID::from_str("02").unwrap(),
            },
        };

        assert_ne!(forward1, forward3, "Forward messages should be different");

        assert_ne!(
            PeerRequest::KeepAlive,
            forward1,
            "KeepAlive should be different from Forward"
        );

        let from_id1 = PeerID::from_str("01").unwrap();
        let from_id2 = PeerID::from_str("02").unwrap();

        let forward_a = PeerRequest::Forward {
            from_peer_id: from_id1,
            to_peer_id: Some(PeerID::from_str("02").unwrap()),
            data: PeerEvent::NewPeer {
                peer_id: PeerID::from_str("01").unwrap(),
            },
        };

        let forward_b = PeerRequest::Forward {
            from_peer_id: from_id2,
            to_peer_id: Some(PeerID::from_str("02").unwrap()),
            data: PeerEvent::NewPeer {
                peer_id: PeerID::from_str("02").unwrap(),
            },
        };

        assert_ne!(forward_a, forward_b, "Forward messages should be different");
    }

    #[test]
    fn test_peer_event_serialization() {
        let new_peer = PeerEvent::NewPeer {
            peer_id: PeerID::from_str("01").unwrap(),
        };

        let serialized = serde_json::to_string(&new_peer).unwrap();
        assert_eq!(serialized, r#"{"new_peer":{"peer_id":[1,1]}}"#);

        let deserialized: PeerEvent = serde_json::from_str(&serialized).unwrap();

        match (new_peer, deserialized) {
            (PeerEvent::NewPeer { peer_id: id1 }, PeerEvent::NewPeer { peer_id: id2 }) => {
                assert_eq!(id1, id2)
            }
            (
                PeerEvent::Message {
                    peer_id: id1,
                    message: msg1,
                },
                PeerEvent::Message {
                    peer_id: id2,
                    message: msg2,
                },
            ) => {
                assert_eq!(id1, id2);
                assert_eq!(msg1, msg2);
            }
            _ => panic!("Invalid peer event"),
        }
    }
}
