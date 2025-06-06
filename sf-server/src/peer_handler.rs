use axum::extract::ws::Message;
use sf_logging::{debug, warn};
use sf_metrics::{Counter, Metrics as MetricsTrait};
use sf_peer_id::PeerID;
use sf_protocol::PeerRequest;
use std::{fmt, sync::Arc};
use tokio::sync::mpsc;

use crate::{socket_metadata::SocketMetadata, state::AppStateInterface};

type PeerSender = mpsc::Sender<Arc<PeerRequest>>;

/// Represents a connected peer and its metadata.
/// A `PeerHandler` is cheap to clone and can be stored elsewhere
/// to enqueue [`PeerRequest`]s for this peer.
#[derive(Clone)]
pub(crate) struct PeerHandler {
    meta: SocketMetadata,
    sender: PeerSender,

    msg_recv_total: Arc<dyn Counter>,
    msg_sent_total: Arc<dyn Counter>,
}

impl PeerHandler {
    pub fn new(meta: SocketMetadata, sender: PeerSender, metrics: &impl MetricsTrait) -> Self {
        let peer_id = meta.peer_id.to_string();
        let labels = &[("peer_id", peer_id.as_str())];

        let msg_recv_total = metrics
            .counter(
                "sf.peer.messages_received_total",
                "Total messages received from this peer",
            )
            .with_labels(labels);

        let msg_sent_total = metrics
            .counter(
                "sf.peer.messages_sent_total",
                "Total messages sent to this peer",
            )
            .with_labels(labels);

        Self {
            meta,
            sender,
            msg_recv_total,
            msg_sent_total,
        }
    }

    #[inline]
    pub fn id(&self) -> &PeerID {
        &self.meta.peer_id
    }

    /// The original Web‑Socket upgrade metadata.
    #[inline]
    #[allow(dead_code)]
    pub fn meta(&self) -> &SocketMetadata {
        &self.meta
    }

    /// Queue a message for delivery to the peer task.
    pub async fn send(&self, req: Arc<PeerRequest>) -> Result<(), crate::error::Error> {
        debug!(peer=%self.meta.peer_id, ?req, "queueing message");
        self.sender
            .send(req)
            .await
            .map(|()| self.msg_sent_total.increment())?;
        Ok(())
    }

    /// Parse an incoming Web‑Socket frame and dispatch it.
    /// Returns **`false`** if the connection should be shut down.
    pub(crate) async fn process_incoming(
        &self,
        msg: Message,
        state: &Arc<impl AppStateInterface>,
    ) -> bool {
        self.msg_recv_total.increment();

        match msg {
            Message::Text(raw) => match serde_json::from_str::<PeerRequest>(&raw) {
                Ok(req) => self.handle_request(req, state).await,
                Err(_e) => {
                    warn!(peer=%self.meta.peer_id, %_e, raw=%raw,
                          "failed to parse text as PeerRequest");
                }
            },
            Message::Binary(_b) => {
                warn!(peer=%self.meta.peer_id, ?_b, "unexpected binary message");
            }
            Message::Ping(_d) => {
                debug!(peer=%self.meta.peer_id, ?_d, "ping");
            }
            Message::Pong(_d) => {
                debug!(peer=%self.meta.peer_id, ?_d, "pong");
            }
            Message::Close(_fr) => {
                debug!(peer=%self.meta.peer_id, ?_fr, "close");
                return false;
            }
        }
        true
    }

    async fn handle_request(&self, req: PeerRequest, state: &Arc<impl AppStateInterface>) {
        let connection_id = self.id();

        match req {
            PeerRequest::KeepAlive => {
                debug!(peer_id = %connection_id, "Processing KeepAlive request");
                state.handle_keepalive(*connection_id).await;
            }
            PeerRequest::Forward {
                from_peer_id,
                to_peer_id,
                data,
            } => {
                debug!(
                    connection_id = %connection_id,
                    from = %from_peer_id,
                    to = ?to_peer_id,
                    "Processing Forward request"
                );
                state
                    .handle_forward(from_peer_id, *connection_id, to_peer_id, data)
                    .await;
            }
        }
    }
}

impl fmt::Debug for PeerHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PeerHandler")
            .field("peer_id", &self.meta.peer_id)
            .field("origin", &self.meta.origin)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    use axum::body::Bytes;
    use sf_metrics::InMemoryMetrics;
    use sf_protocol::PeerEvent;
    use std::{
        net::{IpAddr, Ipv4Addr, SocketAddr},
        str::FromStr,
        sync::atomic::{AtomicBool, Ordering},
    };
    use tokio::sync::mpsc::Receiver;

    fn setup() -> (PeerHandler, InMemoryMetrics, Receiver<Arc<PeerRequest>>) {
        let localhost = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let meta = SocketMetadata::new(localhost, PeerID::from_str("01").unwrap());
        let (sender, receiver) = mpsc::channel::<Arc<PeerRequest>>(1);
        let metrics = InMemoryMetrics::new();
        let peer_handler = PeerHandler::new(meta.clone(), sender, &metrics);
        (peer_handler, metrics, receiver)
    }

    #[test]
    fn test_peer_handler_new() {
        let (_peer_handler, metrics, _receiver) = setup();
        let labels = &[("peer_id", "01")];

        // Check if counters exist and have the correct initial value
        assert_eq!(
            metrics.get_counter_value("sf.peer.messages_received_total", labels),
            Some(0.0),
        );
        assert_eq!(
            metrics.get_counter_value("sf.peer.messages_sent_total", labels),
            Some(0.0),
        );

        // Check for counters with incorrect labels (should not exist)
        let wrong_labels = &[("peer_id", "wrong_peer")];
        assert_eq!(
            metrics.get_counter_value("sf.peer.messages_received_total", wrong_labels),
            None,
        );
    }

    #[test]
    fn test_peer_id() {
        let (peer_handler, _metrics, _receiver) = setup();
        assert_eq!(peer_handler.id(), &PeerID::from_str("01").unwrap());
    }

    #[test]
    fn test_meta() {
        let (peer_handler, _metrics, _receiver) = setup();
        let meta = peer_handler.meta();
        assert_eq!(meta.peer_id, PeerID::from_str("01").unwrap());
        assert_eq!(
            meta.origin,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080)
        );
    }

    #[tokio::test]
    async fn test_send_increments_metric_and_sends() {
        let (peer_handler, metrics, mut receiver) = setup();
        let labels = &[("peer_id", "01")];

        let message = Arc::new(PeerRequest::KeepAlive);
        let result = peer_handler.send(Arc::clone(&message)).await;

        // Check result and metric
        assert!(result.is_ok());
        assert_eq!(
            metrics.get_counter_value("sf.peer.messages_sent_total", labels),
            Some(1.0),
        );

        // Check if message was sent through the channel
        let received_message = receiver.recv().await;
        assert!(received_message.is_some());
        assert_eq!(received_message.unwrap(), message);
    }

    #[tokio::test]
    async fn test_send_fails_when_receiver_dropped() {
        let (peer_handler, metrics, receiver) = setup();
        let labels = &[("peer_id", "01")];

        // Drop the receiver to simulate the channel being closed
        drop(receiver);

        let message = Arc::new(PeerRequest::KeepAlive);
        let result = peer_handler.send(message).await;

        // Check that send fails gracefully and returns Ok(false)
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(crate::error::Error::SendChannelClosed)
        ));

        // Check that the metric was NOT incremented
        assert_eq!(
            metrics.get_counter_value("sf.peer.messages_sent_total", labels),
            Some(0.0),
        );
    }

    #[test]
    fn test_debug_impl() {
        let (peer_handler, _metrics, _receiver) = setup();
        let debug_output = format!("{peer_handler:?}");

        // Check that the output contains the key fields
        assert!(debug_output.contains("PeerHandler"));
        assert!(debug_output.contains("PeerID<32>(01)"));
        assert!(debug_output.contains("origin: 127.0.0.1:8080"));
    }

    #[tokio::test]
    async fn test_process_incoming() {
        // Setup
        let (peer_handler, metrics, _receiver) = setup();
        let labels = &[("peer_id", "01")];

        // Create a direct mock of AppState with the exact methods that PeerHandler.process_incoming
        // calls
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
        impl AppStateInterface for MockAppState {
            async fn handle_keepalive(&self, _peer_id: PeerID) {
                self.keepalive_called.store(true, Ordering::SeqCst);
            }

            async fn handle_forward(
                &self,
                _from_peer_id: PeerID,
                _connection_peer_id: PeerID,
                _to_peer_id: Option<PeerID>,
                _data: PeerEvent,
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
        assert!(result);
        assert_eq!(
            metrics.get_counter_value("sf.peer.messages_received_total", labels),
            Some(1.0),
        );
        assert!(mock_state.keepalive_called.load(Ordering::SeqCst));

        let forward_msg = axum::extract::ws::Message::Text(
            serde_json::to_string(&PeerRequest::Forward {
                from_peer_id: PeerID::from_str("01").unwrap(),
                to_peer_id: Some(PeerID::from_str("02").unwrap()),
                data: PeerEvent::NewPeer {
                    peer_id: PeerID::from_str("01").unwrap(),
                },
            })
            .unwrap()
            .into(),
        );

        let result = peer_handler
            .process_incoming(forward_msg, &mock_state)
            .await;
        assert!(result);
        assert_eq!(
            metrics.get_counter_value("sf.peer.messages_received_total", labels),
            Some(2.0),
        );
        assert!(mock_state.forward_called.load(Ordering::SeqCst));

        let invalid_msg = axum::extract::ws::Message::Text("not valid json".to_string().into());
        let result = peer_handler
            .process_incoming(invalid_msg, &mock_state)
            .await;
        assert!(result);
        assert_eq!(
            metrics.get_counter_value("sf.peer.messages_received_total", labels),
            Some(3.0),
        );

        let binary_msg = axum::extract::ws::Message::Binary(Bytes::from(vec![1, 2, 3]));
        let result = peer_handler.process_incoming(binary_msg, &mock_state).await;
        assert!(result);
        assert_eq!(
            metrics.get_counter_value("sf.peer.messages_received_total", labels),
            Some(4.0),
        );

        let ping_msg = axum::extract::ws::Message::Ping(vec![].into());
        let result = peer_handler.process_incoming(ping_msg, &mock_state).await;
        assert!(result);
        assert_eq!(
            metrics.get_counter_value("sf.peer.messages_received_total", labels),
            Some(5.0),
        );

        let pong_msg = axum::extract::ws::Message::Pong(vec![].into());
        let result = peer_handler.process_incoming(pong_msg, &mock_state).await;
        assert!(result);
        assert_eq!(
            metrics.get_counter_value("sf.peer.messages_received_total", labels),
            Some(6.0),
        );

        let close_msg = axum::extract::ws::Message::Close(None);
        let result = peer_handler.process_incoming(close_msg, &mock_state).await;
        assert!(!result);
        assert_eq!(
            metrics.get_counter_value("sf.peer.messages_received_total", labels),
            Some(7.0),
        );
    }
}
