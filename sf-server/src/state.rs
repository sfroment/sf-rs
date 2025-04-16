use crate::peer_handler::PeerHandler;
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
