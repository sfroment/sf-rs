use crate::peer_handler::PeerHandler;
use async_trait::async_trait;
use dashmap::DashMap;
use serde_json::value::RawValue;
use sf_metrics::{Counter, Gauge, Metrics};
use sf_protocol::{PeerEvent, PeerRequest};
use std::{fmt::Debug, sync::Arc};
use tracing::{debug, error, info, warn};

/// Represents the shared application state.
///
/// Holds a map of connected peers and metrics capabilities.
#[derive(Debug)]
pub struct AppState<M: Metrics> {
    /// Concurrently accessible map of peer IDs to their handlers.
    peers: DashMap<String, PeerHandler>,
    /// Metrics implementation.
    metrics: M,

    /// The current peer count.
    peer_count: Arc<M::G>,

    /// Message broadcast counter
    message_broadcast: Arc<M::C>,

    /// Counter for forwarded messages
    message_forwarded: Arc<M::C>,
}

impl<M> AppState<M>
where
    M: Metrics + Clone + Send + Sync + 'static,
{
    /// Creates a new instance of `AppState`.
    pub fn new(metrics: M) -> Self {
        let peer_count = metrics.gauge("sf.app_state.peer_count", "Number of connected peers");
        let message_broadcast = metrics.counter(
            "sf.app_state.message_broadcast_total",
            "Total number of messages broadcast",
        );
        let message_forwarded = metrics.counter(
            "sf.app_state.message_forwarded_total",
            "Total number of messages forwarded between peers",
        );
        Self {
            peers: DashMap::new(),
            metrics: metrics.clone(),
            peer_count,
            message_broadcast,
            message_forwarded,
        }
    }

    /// Adds a new peer handler to the state and makes a best-effort attempt
    /// to broadcast the NewPeer event.
    pub async fn add_peer(&self, peer_handler: PeerHandler) -> Result<Arc<String>, crate::Error> {
        let peer_id_arc = Arc::new(peer_handler.peer_id().to_string());
        let peer_id_str = peer_id_arc.to_string();

        if self.peers.contains_key(&peer_id_str) {
            warn!("Attempted to add peer that already exists: {}", peer_id_arc);
            return Err(crate::Error::PeerAlreadyExists(peer_id_str));
        }

        debug!("Adding peer: {}", peer_id_arc);
        self.peers.insert(peer_id_str.clone(), peer_handler.clone());
        self.peer_count.increment();
        debug!("State: Added peer: {}", peer_id_arc);

        let broadcast_prep_result = async {
            let peer_event = PeerEvent::NewPeer {
                peer_id: peer_id_str.clone(),
            };
            let event_data_str = serde_json::to_string(&peer_event)
                .map_err(|e| format!("Failed to serialize NewPeer event: {}", e))?;

            let raw_value = serde_json::value::RawValue::from_string(event_data_str)
                .map_err(|e| format!("Failed to create RawValue from serialized event: {}", e))?;

            Ok::<Arc<RawValue>, String>(Arc::from(raw_value))
        }
        .await;

        match broadcast_prep_result {
            Ok(data_arc) => {
                debug!("Broadcasting NewPeer event for peer {}", peer_id_arc);
                self.broadcast_forward_except(peer_id_arc.clone(), data_arc, &peer_id_str)
                    .await;
            }
            Err(err_msg) => {
                error!(
                    "Failed to prepare NewPeer broadcast for {}: {}. Peer added, but not broadcasted.",
                    peer_id_arc, err_msg
                );
            }
        }

        Ok(peer_id_arc)
    }

    pub fn remove_peer(&self, peer_id: &str) {
        if self.peers.remove(peer_id).is_some() {
            debug!("Removing peer: {}", peer_id);
            self.peer_count.decrement();
        } else {
            warn!("Attempted to remove peer that doesn't exist: {}", peer_id);
        }
    }

    // This method is only used internally by send_to_peer
    fn get_peer(
        &self,
        peer_id: &str,
    ) -> Result<dashmap::mapref::one::Ref<'_, String, PeerHandler>, crate::Error> {
        self.peers
            .get(peer_id)
            .ok_or(crate::Error::PeerNotFound(peer_id.to_string()))
    }

    /// Sends a message payload (type T) to a specific peer.
    pub async fn send_to_peer(&self, peer_id: &str, message_payload: Arc<PeerRequest>) {
        match self.get_peer(peer_id) {
            Ok(peer_handler_ref) => {
                let peer_handler = peer_handler_ref.value().clone();
                drop(peer_handler_ref);

                match peer_handler.send(message_payload).await {
                    Ok(true) => {}
                    Ok(false) | Err(_) => {
                        warn!("Removing peer {} due to send failure.", peer_id);
                        self.remove_peer(peer_id);
                    }
                }
            }
            Err(e) => {
                warn!("send_to_peer: failed to get peer: {}", e);
            }
        }
    }

    /// Handles a KeepAlive request from a peer.
    pub(crate) async fn handle_keepalive(&self, peer_id: Arc<String>) {
        debug!("Received keep-alive from peer {}", peer_id);
    }

    /// Handles a Forward request from a peer.
    pub(crate) async fn handle_forward(
        &self,
        _request_from_peer_id: Arc<String>,
        connection_peer_id: Arc<String>,
        to_peer_id: Option<String>,
        data: Arc<RawValue>,
    ) {
        debug!(
            "Received forward request from connection {}",
            connection_peer_id
        );

        if let Some(target_peer_id) = to_peer_id {
            debug!(
                "Forwarding data from {} to {}",
                connection_peer_id, target_peer_id
            );
            let forward_req = PeerRequest::Forward {
                from_peer_id: connection_peer_id.clone(),
                to_peer_id: Some(target_peer_id.clone()),
                data: data.clone(),
            };
            self.send_to_peer(&target_peer_id, Arc::new(forward_req))
                .await;
            self.message_forwarded
                .with_labels(&[("peer_id", &target_peer_id)])
                .increment();
        } else {
            warn!(
                "Received forward request from peer {} with 'to_peer_id: None', interpreting as broadcast.",
                connection_peer_id
            );

            self.broadcast_forward_except(connection_peer_id.clone(), data, &connection_peer_id)
                .await;
            self.message_broadcast.increment();
        }
    }

    /// Broadcasts data (wrapped in a Forward request) to ALL connected peers.
    #[allow(dead_code)]
    pub async fn broadcast_forward(&self, from_peer_id: Arc<String>, data: Arc<RawValue>) {
        info!(
            "Broadcasting data from {} to {} peers",
            &*from_peer_id,
            self.peers.len()
        );
        let peer_ids: Vec<String> = self.peers.iter().map(|entry| entry.key().clone()).collect();

        for peer_id in peer_ids {
            let targeted_request = PeerRequest::Forward {
                from_peer_id: Arc::clone(&from_peer_id),
                to_peer_id: Some(peer_id.clone()),
                data: Arc::clone(&data),
            };
            self.send_to_peer(&peer_id, Arc::new(targeted_request))
                .await;
        }
        self.message_broadcast.increment();
    }

    /// Broadcasts data (wrapped in a Forward request) to all connected peers EXCEPT the specified one.
    pub async fn broadcast_forward_except(
        &self,
        from_peer_id: Arc<String>,
        data: Arc<RawValue>,
        exclude_peer_id: &str,
    ) {
        info!(
            "Broadcasting data from {} to {} peers (excluding {})",
            &*from_peer_id,
            self.peers.len().saturating_sub(1),
            exclude_peer_id
        );
        let peer_ids: Vec<String> = self.peers.iter().map(|entry| entry.key().clone()).collect();

        for peer_id in peer_ids {
            if peer_id == exclude_peer_id {
                continue;
            }
            let targeted_request = PeerRequest::Forward {
                from_peer_id: Arc::clone(&from_peer_id),
                to_peer_id: Some(peer_id.clone()),
                data: Arc::clone(&data),
            };
            self.send_to_peer(&peer_id, Arc::new(targeted_request))
                .await;
        }
        self.message_broadcast.increment();
    }

    pub fn metrics(&self) -> &M {
        &self.metrics
    }
}

/// Trait defining the interface that PeerHandler needs from AppState.
/// This makes testing easier by allowing mock implementations.
#[async_trait]
pub trait AppStateInterface: Send + Sync + 'static {
    /// Handles a KeepAlive request from a peer.
    async fn handle_keepalive(&self, peer_id: Arc<String>);

    /// Handles a Forward request from a peer.
    async fn handle_forward(
        &self,
        from_peer_id: Arc<String>,
        connection_peer_id: Arc<String>,
        to_peer_id: Option<String>,
        data: Arc<RawValue>,
    );
}

// Implement the trait for AppState
#[async_trait]
impl<M: Metrics + Clone + Send + Sync + 'static> AppStateInterface for AppState<M> {
    async fn handle_keepalive(&self, peer_id: Arc<String>) {
        self.handle_keepalive(peer_id).await
    }

    async fn handle_forward(
        &self,
        from_peer_id: Arc<String>,
        connection_peer_id: Arc<String>,
        to_peer_id: Option<String>,
        data: Arc<RawValue>,
    ) {
        self.handle_forward(from_peer_id, connection_peer_id, to_peer_id, data)
            .await
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        net::{IpAddr, Ipv4Addr, SocketAddr},
    };

    use super::*;
    use crate::{state::AppState, ws_handler::WsUpgradeMeta};
    use axum::http::header;
    use sf_metrics::InMemoryMetrics;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_add_peer_success() {
        let metrics = InMemoryMetrics::new();
        let state = AppState::new(metrics);

        let (tx, _) = mpsc::channel::<Arc<PeerRequest>>(100);

        let peer_handler = PeerHandler::new(
            WsUpgradeMeta {
                peer_id: "peer_id".to_string(),
                origin: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
                _headers: header::HeaderMap::new(),
                _path: None,
                _query_params: HashMap::new(),
            },
            tx,
            state.metrics(),
        );

        let peer_id_arc = state.add_peer(peer_handler).await.unwrap();
        assert_eq!(peer_id_arc.to_string(), "peer_id");
    }

    #[tokio::test]
    async fn test_add_peer_failure() {
        let metrics = InMemoryMetrics::new();
        let state = AppState::new(metrics);

        let (tx, _) = mpsc::channel::<Arc<PeerRequest>>(100);

        let peer_handler = PeerHandler::new(
            WsUpgradeMeta {
                peer_id: "peer_id".to_string(),
                origin: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
                _headers: header::HeaderMap::new(),
                _path: None,
                _query_params: HashMap::new(),
            },
            tx,
            state.metrics(),
        );

        let peer_id_arc = state.add_peer(peer_handler.clone()).await.unwrap();
        assert_eq!(peer_id_arc.to_string(), "peer_id");

        let peer_id_arc = state.add_peer(peer_handler).await;
        assert!(matches!(
            peer_id_arc,
            Err(crate::Error::PeerAlreadyExists(_))
        ));
    }

    #[tokio::test]
    async fn broadcast_forward() {
        let metrics = InMemoryMetrics::new();
        let state = AppState::new(metrics);

        let (tx, _) = mpsc::channel::<Arc<PeerRequest>>(100);

        let peer_handler = PeerHandler::new(
            WsUpgradeMeta {
                peer_id: "peer_id".to_string(),
                origin: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
                _headers: header::HeaderMap::new(),
                _path: None,
                _query_params: HashMap::new(),
            },
            tx,
            state.metrics(),
        );

        let peer_id_arc = state.add_peer(peer_handler).await.unwrap();
        assert_eq!(peer_id_arc.to_string(), "peer_id");

        let (tx_2, mut rx_2) = mpsc::channel::<Arc<PeerRequest>>(100);
        let peer_handler_2 = PeerHandler::new(
            WsUpgradeMeta {
                peer_id: "peer_id_2".to_string(),
                origin: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
                _headers: header::HeaderMap::new(),
                _path: None,
                _query_params: HashMap::new(),
            },
            tx_2,
            state.metrics(),
        );

        let peer_id_arc = state.add_peer(peer_handler_2).await.unwrap();
        assert_eq!(peer_id_arc.to_string(), "peer_id_2");

        let data = Arc::from(RawValue::from_string(r#"{"message":"hello"}"#.to_string()).unwrap());
        state.broadcast_forward(peer_id_arc, data).await;

        let received_message = rx_2.recv().await.unwrap();

        match &*received_message {
            PeerRequest::Forward {
                from_peer_id,
                to_peer_id,
                data,
            } => {
                assert_eq!(from_peer_id.to_string(), "peer_id_2");
                assert_eq!(to_peer_id, &Some("peer_id_2".to_string()));
                assert_eq!(data.get().to_string(), r#"{"message":"hello"}"#);
            }
            _ => panic!("Expected Forward message, got: {:?}", received_message),
        }
    }
}
