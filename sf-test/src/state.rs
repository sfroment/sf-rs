use dashmap::DashMap;
use serde_json::value::RawValue;
use sf_logging::{debug, error, info, warn};
use sf_metrics::{Counter, Gauge, Metrics};
use sf_protocol::{PeerEvent, PeerRequest};
use std::sync::Arc;

use crate::{peer_handler::PeerHandler, peer_id::PeerID};

#[derive(Debug)]
pub(crate) struct AppState<M>
where
    M: Metrics + Clone + Send + Sync + 'static,
{
    peers: DashMap<String, PeerHandler>,

    metrics: M,

    peer_count: Arc<M::G>,
    message_broadcast: Arc<M::C>,
    message_forwarded: Arc<M::C>,
}

impl<M: Metrics> AppState<M> {
    pub(crate) fn new(metrics: M) -> Self {
        let peer_count = metrics.gauge("sf.app_state.peer_count", "Connected peers");
        let message_broadcast =
            metrics.counter("sf.app_state.message_broadcast_total", "Messages broadcast");
        let message_forwarded =
            metrics.counter("sf.app_state.message_forwarded_total", "Messages forwarded");

        Self {
            peers: DashMap::new(),
            metrics,
            peer_count,
            message_broadcast,
            message_forwarded,
        }
    }

    pub(crate) async fn add_peer(&self, peer_handler: PeerHandler) -> Result<PeerID, crate::Error> {
        let peer_id = peer_handler.id().clone();
        let key = peer_id.to_string();

        if self.peers.contains_key(&key) {
            warn!("Attempted to add existing peer: {}", key);
            return Err(crate::Error::PeerAlreadyExists(key));
        }

        self.peers.insert(key.clone(), peer_handler);
        self.peer_count.increment();
        debug!("Peer added: {}", key);

        if let Err(_e) = self
            .broadcast_system_event(PeerEvent::NewPeer {
                peer_id: key.clone(),
            })
            .await
        {
            error!("Failed to broadcast NewPeer for {}: {}", key, _e);
        }

        Ok(peer_id)
    }

    pub(crate) fn remove_peer(&self, peer_id: &str) {
        if self.peers.remove(peer_id).is_some() {
            self.peer_count.decrement();
            debug!("Peer removed: {}", peer_id);
        }
    }

    pub(crate) async fn send_to_peer(&self, peer_id: &str, payload: Arc<PeerRequest>) {
        if let Some(entry) = self.peers.get(peer_id) {
            let handler = entry.value().clone();
            let _peer_id_arc = entry.key().clone();

            tokio::spawn(async move {
                if handler.send(payload).await.is_err() {
                    warn!("Send failed to peer {}; disconnecting", _peer_id_arc);
                }
            });
        } else {
            warn!("send_to_peer: peer not found: {}", peer_id);
        }
    }

    pub(crate) async fn handle_keepalive(&self, _peer_id: PeerID) {
        debug!("Keep‑alive from {}", _peer_id);
    }

    pub(crate) async fn handle_forward(
        &self,
        from_peer_id: PeerID,
        connection_peer_id: PeerID,
        to_peer_id: Option<String>,
        data: Arc<RawValue>,
    ) {
        match to_peer_id {
            Some(target) => self.forward_single(from_peer_id, target, data).await,
            None => {
                self.broadcast_forward_except(connection_peer_id, data, None)
                    .await
            }
        }
    }

    async fn forward_single(&self, from_peer: PeerID, to_peer: String, data: Arc<RawValue>) {
        self.send_forward(&from_peer, &to_peer, data).await;
        self.message_forwarded
            .with_labels(&[("peer_id", &to_peer)])
            .increment();
    }

    pub(crate) async fn broadcast_forward_except(
        &self,
        from_peer_id: PeerID,
        data: Arc<RawValue>,
        exclude: Option<&str>,
    ) {
        info!(
            "Broadcasting from {} to {} peers (exclude: {:?})",
            from_peer_id,
            self.peers.len().saturating_sub(exclude.map_or(0, |_| 1)),
            exclude
        );
        let ids: Vec<String> = self.peers.iter().map(|e| e.key().clone()).collect();
        println!("Broadcasting to peers: {:?}", ids);
        for pid in ids {
            if exclude.map_or(false, |ex| ex == pid) {
                continue;
            }
            self.send_forward(&from_peer_id, &pid, data.clone()).await;
        }
        self.message_broadcast.increment();
    }

    async fn send_forward(&self, from: &PeerID, to: &str, data: Arc<RawValue>) {
        println!("send_forward {} {}", from, to);
        let req = Arc::new(PeerRequest::Forward {
            from_peer_id: Arc::clone(from),
            to_peer_id: Some(to.to_owned()),
            data,
        });
        self.send_to_peer(to, req).await;
    }

    async fn broadcast_system_event(&self, event: PeerEvent) -> Result<(), serde_json::Error> {
        let raw = Arc::from(RawValue::from_string(serde_json::to_string(&event)?)?);
        self.broadcast_forward_except(Arc::new("system".into()), raw, None)
            .await;
        Ok(())
    }

    pub(crate) fn metrics(&self) -> &M {
        &self.metrics
    }
}

pub trait AppStateInterface: Send + Sync + 'static {
    async fn handle_keepalive(&self, peer_id: PeerID);
    async fn handle_forward(
        &self,
        from_peer_id: PeerID,
        connection_peer_id: PeerID,
        to_peer_id: Option<String>,
        data: Arc<RawValue>,
    );
}

impl<M> AppStateInterface for AppState<M>
where
    M: Metrics + Clone + Send + Sync + 'static,
{
    async fn handle_keepalive(&self, peer_id: PeerID) {
        self.handle_keepalive(peer_id).await
    }

    async fn handle_forward(
        &self,
        from_peer_id: PeerID,
        connection_peer_id: PeerID,
        to_peer_id: Option<String>,
        data: Arc<RawValue>,
    ) {
        self.handle_forward(from_peer_id, connection_peer_id, to_peer_id, data)
            .await
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use std::{
        net::{IpAddr, Ipv4Addr, SocketAddr},
        sync::{
            Mutex,
            atomic::{AtomicBool, Ordering},
        },
    };

    use crate::socket_metadata::SocketMetadata;

    use super::*;

    use sf_metrics::InMemoryMetrics;
    use tokio::sync::mpsc;
    use tracing_test::traced_test;

    fn get_app_state() -> AppState<InMemoryMetrics> {
        let metrics = InMemoryMetrics::new();
        AppState::new(metrics)
    }

    #[tokio::test]
    async fn test_add_peer_success() {
        let state = get_app_state();

        let (tx, _) = mpsc::channel::<Arc<PeerRequest>>(100);

        let origin = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let peer_id = PeerID::new("peer_id".to_string());
        let peer_handler =
            PeerHandler::new(SocketMetadata::new(origin, peer_id), tx, state.metrics());

        let peer_id_arc = state.add_peer(peer_handler).await.unwrap();
        assert_eq!(peer_id_arc.to_string(), "peer_id");
    }

    #[tokio::test]
    async fn test_add_peer_failure() {
        let state = get_app_state();

        let (tx, _) = mpsc::channel::<Arc<PeerRequest>>(100);

        let origin = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let peer_id = PeerID::new("peer_id".to_string());
        let peer_handler =
            PeerHandler::new(SocketMetadata::new(origin, peer_id), tx, state.metrics());

        let peer_id_arc = state.add_peer(peer_handler.clone()).await.unwrap();
        assert_eq!(peer_id_arc.to_string(), "peer_id");

        let peer_id_arc = state.add_peer(peer_handler).await;
        assert!(matches!(
            peer_id_arc,
            Err(crate::Error::PeerAlreadyExists(_))
        ));
    }

    #[tokio::test]
    async fn test_remove_peer() {
        let state = get_app_state();

        let (tx, _) = mpsc::channel::<Arc<PeerRequest>>(100);
        let origin = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let peer_id = PeerID::new("peer_id".to_string());
        let peer_handler = PeerHandler::new(
            SocketMetadata::new(origin, peer_id.clone()),
            tx,
            state.metrics(),
        );

        let peer_id_arc = state.add_peer(peer_handler.clone()).await.unwrap();
        assert_eq!(peer_id_arc.to_string(), "peer_id");

        state.remove_peer(&peer_id);
        state.remove_peer(&peer_id);
    }

    #[tokio::test]
    #[traced_test]
    async fn broadcast_forward() {
        let state = get_app_state();

        let (tx, _) = mpsc::channel::<Arc<PeerRequest>>(100);

        let origin = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let peer_id = PeerID::new("peer_id".to_string());
        let peer_handler = PeerHandler::new(
            SocketMetadata::new(origin.clone(), peer_id),
            tx,
            state.metrics(),
        );

        let peer_id_after_add_peer = state.add_peer(peer_handler).await.unwrap();
        assert_eq!(peer_id_after_add_peer.to_string(), "peer_id");

        let (tx_2, mut rx_2) = mpsc::channel::<Arc<PeerRequest>>(100);
        let peer_id_2 = PeerID::new("peer_id_2".to_string());
        let meta_2 = SocketMetadata::new(origin.clone(), peer_id_2);
        let peer_handler_2 = PeerHandler::new(meta_2, tx_2, state.metrics());

        let peer_id_after_add_peer = state.add_peer(peer_handler_2).await.unwrap();
        assert_eq!(peer_id_after_add_peer.to_string(), "peer_id_2");

        let data = Arc::from(RawValue::from_string(r#"{"message":"hello"}"#.to_string()).unwrap());
        println!("before broadcast {}", peer_id_after_add_peer);
        state
            .broadcast_forward_except(peer_id_after_add_peer, data, None)
            .await;

        let received_message = rx_2.recv().await.unwrap();

        match &*received_message {
            PeerRequest::Forward {
                from_peer_id,
                to_peer_id,
                data,
            } => {
                assert_eq!(from_peer_id.to_string(), "system");
                assert_eq!(to_peer_id, &Some("peer_id_2".to_string()));
                assert_eq!(
                    data.get().to_string(),
                    r#"{"new_peer":{"peer_id":"peer_id_2"}}"#
                );
            }
            _ => panic!("Expected Forward message, got: {:?}", received_message),
        }

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

    #[derive(Clone, Default)]
    struct MockAppState {
        keepalive_called: Arc<AtomicBool>,
        forward_called: Arc<AtomicBool>,
        last_forward_params: Arc<Mutex<Option<(PeerID, PeerID, Option<String>, Arc<RawValue>)>>>,
    }

    impl AppStateInterface for MockAppState {
        async fn handle_keepalive(&self, _peer_id: PeerID) {
            self.keepalive_called.store(true, Ordering::SeqCst);
            println!("MockAppState: handle_keepalive called");
        }

        async fn handle_forward(
            &self,
            from_peer_id: PeerID,
            connection_peer_id: PeerID,
            to_peer_id: Option<String>,
            data: Arc<RawValue>,
        ) {
            self.forward_called.store(true, Ordering::SeqCst);
            let mut guard = self.last_forward_params.lock().unwrap();
            *guard = Some((
                from_peer_id.clone(),
                connection_peer_id.clone(),
                to_peer_id.clone(),
                data.clone(),
            ));
            println!(
                "MockAppState: handle_forward called from {} (conn: {}), to: {:?}, data: {}",
                from_peer_id,
                connection_peer_id,
                to_peer_id,
                data.get()
            );
        }
    }
}
