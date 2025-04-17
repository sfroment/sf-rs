use crate::{state::AppStateInterface, ws_handler::WsUpgradeMeta};
use axum::extract::ws::Message;
use core::fmt;
use sf_metrics::{Counter, Metrics as MetricsTrait};
use sf_protocol::PeerRequest;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, warn};

/// Represents a connected peer and holds its metadata.
///
/// This structure can be expanded later to hold communication channels
/// (like an `mpsc::Sender`) to send messages *to* this specific peer
/// from other parts of the application if needed.
#[derive(Clone)]
pub struct PeerHandler {
    meta: WsUpgradeMeta,
    sender: mpsc::Sender<Arc<PeerRequest>>,

    message_received: Arc<dyn Counter>,
    message_sent: Arc<dyn Counter>,
}

impl PeerHandler {
    /// Creates a new `PeerHandler` with the given metadata.
    pub fn new(
        meta: WsUpgradeMeta,
        sender: mpsc::Sender<Arc<PeerRequest>>,
        metrics: &impl MetricsTrait,
    ) -> Self {
        let labels: &[(&str, &str)] = &[("peer_id", &meta.peer_id)];
        let message_received = metrics
            .counter(
                "sf.peer.messages_received_total",
                "Total messages received from this peer",
            )
            .with_labels(labels);
        let message_sent = metrics
            .counter(
                "sf.peer.messages_sent_total",
                "Total messages sent to this peer",
            )
            .with_labels(labels);

        Self {
            meta,
            sender,
            message_received,
            message_sent,
        }
    }

    /// Returns the peer ID associated with this handler.
    pub fn peer_id(&self) -> &str {
        &self.meta.peer_id
    }

    /// Returns the connection metadata.
    #[allow(dead_code)]
    pub fn meta(&self) -> &WsUpgradeMeta {
        &self.meta
    }

    /// Sends a message to this specific peer's handling task via the channel.
    pub async fn send(
        &self,
        message: Arc<PeerRequest>,
    ) -> Result<bool, mpsc::error::SendError<PeerRequest>> {
        debug!(
            "Queueing message for peer {}: {:?}",
            self.meta.peer_id, message
        );

        match self.sender.send(message).await {
            Ok(()) => {
                self.message_sent.increment();
                Ok(true)
            }
            Err(e @ mpsc::error::SendError(_)) => {
                error!(
                    "Failed to send message to peer {}: Receiver dropped. Peer task likely terminated. Error: {}",
                    self.meta.peer_id, e
                );
                Ok(false)
            }
        }
    }

    /// Parses incoming message and delegates handling to AppState.
    pub async fn process_incoming(
        &self,
        msg: Message,
        state: &Arc<impl AppStateInterface>,
    ) -> bool {
        self.message_received.increment();
        match msg {
            Message::Text(text) => match serde_json::from_str::<PeerRequest>(&text) {
                Ok(request) => {
                    let connection_peer_id = Arc::new(self.peer_id().to_string());

                    match request {
                        PeerRequest::KeepAlive => {
                            state.handle_keepalive(connection_peer_id).await;
                        }
                        PeerRequest::Forward {
                            from_peer_id,
                            to_peer_id,
                            data,
                        } => {
                            state
                                .handle_forward(from_peer_id, connection_peer_id, to_peer_id, data)
                                .await;
                        }
                    }
                }
                Err(e) => {
                    warn!(
                        "Failed to parse text from peer {} as PeerRequest: {}. Text: '{}'",
                        self.peer_id(),
                        e,
                        text
                    );
                }
            },
            Message::Binary(data) => {
                warn!(
                    "Received binary message from peer {}: {:?}",
                    self.meta.peer_id, data
                );
            }
            Message::Ping(data) => {
                debug!("Received ping from peer {}: {:?}", self.meta.peer_id, data);
            }
            Message::Close(_) => {
                debug!("Received close from peer {}", self.meta.peer_id);
                return false;
            }
            Message::Pong(_) => {
                debug!("Received pong from peer {}", self.meta.peer_id);
            }
        }
        true
    }
}

impl fmt::Debug for PeerHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PeerHandler")
            .field("peer_id", &self.meta.peer_id)
            .field("origin", &self.meta.origin)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use serde_json::value::RawValue;
    use std::{
        net::{IpAddr, Ipv4Addr, SocketAddr},
        sync::atomic::{AtomicBool, Ordering},
    };
    use tokio::sync::mpsc::Receiver;

    use async_trait::async_trait;
    use sf_metrics::InMemoryMetrics;

    fn setup() -> (PeerHandler, InMemoryMetrics, Receiver<Arc<PeerRequest>>) {
        let meta = WsUpgradeMeta {
            peer_id: "test_peer".to_string(),
            origin: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
            _path: Some("/ws".to_string()),
            _query_params: Default::default(),
            _headers: Default::default(),
        };
        let (sender, receiver) = mpsc::channel::<Arc<PeerRequest>>(1);
        let metrics = InMemoryMetrics::new();
        let peer_handler = PeerHandler::new(meta.clone(), sender, &metrics);
        (peer_handler, metrics, receiver)
    }

    #[test]
    fn test_peer_handler_new() {
        let (_peer_handler, metrics, _receiver) = setup();
        let labels = &[("peer_id", "test_peer")];

        // Check if counters exist and have the correct initial value
        assert_eq!(
            metrics.get_counter_value("sf.peer.messages_received_total", labels),
            Some(0.0),
            "messages_received_total counter should exist and be 0"
        );
        assert_eq!(
            metrics.get_counter_value("sf.peer.messages_sent_total", labels),
            Some(0.0),
            "messages_sent_total counter should exist and be 0"
        );

        // Check for counters with incorrect labels (should not exist)
        let wrong_labels = &[("peer_id", "wrong_peer")];
        assert_eq!(
            metrics.get_counter_value("sf.peer.messages_received_total", wrong_labels),
            None,
            "messages_received_total counter should not exist for wrong labels"
        );
    }

    #[test]
    fn test_peer_id() {
        let (peer_handler, _metrics, _receiver) = setup();
        assert_eq!(peer_handler.peer_id(), "test_peer");
    }

    #[test]
    fn test_meta() {
        let (peer_handler, _metrics, _receiver) = setup();
        let meta = peer_handler.meta();
        assert_eq!(meta.peer_id, "test_peer");
        // We can add more assertions here if needed, e.g., checking origin or path
        assert_eq!(
            meta.origin,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080)
        );
        assert_eq!(meta._path, Some("/ws".to_string()));
    }

    #[tokio::test]
    async fn test_send_increments_metric_and_sends() {
        let (peer_handler, metrics, mut receiver) = setup();
        let labels = &[("peer_id", "test_peer")];

        let message = Arc::new(PeerRequest::KeepAlive);
        let result = peer_handler.send(Arc::clone(&message)).await;

        // Check result and metric
        assert!(result.is_ok());
        assert!(result.unwrap(), "send should return true on success");
        assert_eq!(
            metrics.get_counter_value("sf.peer.messages_sent_total", labels),
            Some(1.0),
            "messages_sent_total counter should be 1.0 after successful send"
        );

        // Check if message was sent through the channel
        let received_message = receiver.recv().await;
        assert!(
            received_message.is_some(),
            "Should receive the message from the channel"
        );
        assert_eq!(
            received_message.unwrap(),
            message,
            "Received message should match sent message"
        );
    }

    #[tokio::test]
    async fn test_send_fails_when_receiver_dropped() {
        let (peer_handler, metrics, receiver) = setup();
        let labels = &[("peer_id", "test_peer")];

        // Drop the receiver to simulate the channel being closed
        drop(receiver);

        let message = Arc::new(PeerRequest::KeepAlive);
        let result = peer_handler.send(message).await;

        // Check that send fails gracefully and returns Ok(false)
        assert!(result.is_ok());
        assert!(
            !result.unwrap(),
            "send should return false when receiver is dropped"
        );

        // Check that the metric was NOT incremented
        assert_eq!(
            metrics.get_counter_value("sf.peer.messages_sent_total", labels),
            Some(0.0),
            "messages_sent_total counter should remain 0.0 after failed send"
        );
    }

    #[test]
    fn test_debug_impl() {
        let (peer_handler, _metrics, _receiver) = setup();
        let debug_output = format!("{:?}", peer_handler);

        // Check that the output contains the key fields
        assert!(debug_output.contains("PeerHandler"));
        assert!(debug_output.contains("peer_id: \"test_peer\""));
        assert!(debug_output.contains("origin: 127.0.0.1:8080"));
    }

    #[tokio::test]
    async fn test_process_incoming() {
        // Setup
        let (peer_handler, metrics, _receiver) = setup();
        let labels = &[("peer_id", "test_peer")];

        // Create a direct mock of AppState with the exact methods that PeerHandler.process_incoming calls
        #[derive(Debug)]
        struct MockAppState {
            keepalive_called: AtomicBool,
            forward_called: AtomicBool,
        }

        impl MockAppState {
            fn new() -> Self {
                Self {
                    keepalive_called: AtomicBool::new(false),
                    forward_called: AtomicBool::new(false),
                }
            }
        }

        // Implement AppStateInterface for our mock
        #[async_trait]
        impl AppStateInterface for MockAppState {
            async fn handle_keepalive(&self, _peer_id: Arc<String>) {
                self.keepalive_called.store(true, Ordering::SeqCst);
            }

            async fn handle_forward(
                &self,
                _from_peer_id: Arc<String>,
                _connection_peer_id: Arc<String>,
                _to_peer_id: Option<String>,
                _data: Arc<RawValue>,
            ) {
                self.forward_called.store(true, Ordering::SeqCst);
            }
        }

        // Use Arc directly without AppState::new
        let mock_state = Arc::new(MockAppState::new());

        let keep_alive_msg = axum::extract::ws::Message::Text(
            serde_json::to_string(&PeerRequest::KeepAlive)
                .unwrap()
                .into(),
        );

        let result = peer_handler
            .process_incoming(keep_alive_msg, &mock_state)
            .await;
        assert!(result, "process_incoming should return true for KeepAlive");
        assert_eq!(
            metrics.get_counter_value("sf.peer.messages_received_total", labels),
            Some(1.0),
            "messages_received_total counter should be incremented"
        );
        assert!(
            mock_state.keepalive_called.load(Ordering::SeqCst),
            "handle_keepalive should be called"
        );

        let forward_msg = axum::extract::ws::Message::Text(
            serde_json::to_string(&PeerRequest::Forward {
                from_peer_id: Arc::new("sender".to_string()),
                to_peer_id: Some("recipient".to_string()),
                data: Arc::from(serde_json::value::to_raw_value("hello").unwrap()),
            })
            .unwrap()
            .into(),
        );

        let result = peer_handler
            .process_incoming(forward_msg, &mock_state)
            .await;
        assert!(result, "process_incoming should return true for Forward");
        assert_eq!(
            metrics.get_counter_value("sf.peer.messages_received_total", labels),
            Some(2.0),
            "messages_received_total counter should be incremented twice"
        );
        assert!(
            mock_state.forward_called.load(Ordering::SeqCst),
            "handle_forward should be called"
        );

        let invalid_msg = axum::extract::ws::Message::Text("not valid json".to_string().into());
        let result = peer_handler
            .process_incoming(invalid_msg, &mock_state)
            .await;
        assert!(
            result,
            "process_incoming should return true for invalid message"
        );
        assert_eq!(
            metrics.get_counter_value("sf.peer.messages_received_total", labels),
            Some(3.0),
            "messages_received_total counter should be incremented for invalid messages"
        );

        let binary_msg = axum::extract::ws::Message::Binary(Bytes::from(vec![1, 2, 3]));
        let result = peer_handler.process_incoming(binary_msg, &mock_state).await;
        assert!(
            result,
            "process_incoming should return true for Binary message"
        );
        assert_eq!(
            metrics.get_counter_value("sf.peer.messages_received_total", labels),
            Some(4.0),
            "messages_received_total counter should be incremented for binary messages"
        );

        let ping_msg = axum::extract::ws::Message::Ping(vec![].into());
        let result = peer_handler.process_incoming(ping_msg, &mock_state).await;
        assert!(
            result,
            "process_incoming should return true for Ping message"
        );
        assert_eq!(
            metrics.get_counter_value("sf.peer.messages_received_total", labels),
            Some(5.0),
            "messages_received_total counter should be incremented for ping messages"
        );

        let pong_msg = axum::extract::ws::Message::Pong(vec![].into());
        let result = peer_handler.process_incoming(pong_msg, &mock_state).await;
        assert!(
            result,
            "process_incoming should return true for Pong message"
        );
        assert_eq!(
            metrics.get_counter_value("sf.peer.messages_received_total", labels),
            Some(6.0),
            "messages_received_total counter should be incremented for pong messages"
        );

        let close_msg = axum::extract::ws::Message::Close(None);
        let result = peer_handler.process_incoming(close_msg, &mock_state).await;
        assert!(
            !result,
            "process_incoming should return false for Close message"
        );
        assert_eq!(
            metrics.get_counter_value("sf.peer.messages_received_total", labels),
            Some(7.0),
            "messages_received_total counter should be incremented for close messages"
        );
    }
}
