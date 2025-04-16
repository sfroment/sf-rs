use core::fmt;
use std::sync::Arc;

use axum::extract::ws::Message;
use sf_metrics::{Counter, Metrics};
use sf_protocol::PeerRequest;
use tokio::sync::mpsc;
use tracing::{debug, error, warn};

use crate::{state::AppState, ws_handler::WsUpgradeMeta};

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
        metrics: &impl Metrics,
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
        state: &Arc<AppState<impl Metrics>>,
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
            _ => {}
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
