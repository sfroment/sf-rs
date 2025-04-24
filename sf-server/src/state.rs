use dashmap::DashMap;
use serde_json::value::RawValue;
use sf_logging::{debug, error, info, warn};
use sf_metrics::{Counter, Gauge, Metrics};
use sf_peer_id::PeerID;
use sf_protocol::{PeerEvent, PeerRequest};
use std::{str::FromStr, sync::Arc};

use crate::peer_handler::PeerHandler;

const SYSTEM_EVENT_PREFIX: &str = "system";

#[derive(Debug)]
pub(crate) struct AppState<M>
where
    M: Metrics + Clone + Send + Sync + 'static,
{
    pub(crate) peers: DashMap<PeerID, PeerHandler>,

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
        let peer_id = peer_handler.id();

        if self.peers.contains_key(peer_id) {
            warn!("Attempted to add existing peer: {peer_id}");
            return Err(crate::Error::PeerAlreadyExists(*peer_id));
        }

        self.peers.insert(*peer_id, peer_handler.clone());
        self.peer_count.increment();
        debug!("Peer added: {peer_id}");

        self.broadcast_system_event_and_log(PeerEvent::NewPeer { peer_id: *peer_id })
            .await;

        Ok(*peer_id)
    }

    #[inline]
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub(crate) async fn broadcast_system_event_and_log(&self, event: PeerEvent) {
        if let Err(_e) = self.broadcast_system_event(event).await {
            error!("Failed to broadcast system event: {_e}");
        }
    }

    pub(crate) fn remove_peer(&self, peer_id: &PeerID) {
        if self.peers.remove(peer_id).is_some() {
            self.peer_count.decrement();
            debug!("Peer removed: {peer_id}");
        }
    }

    pub(crate) async fn send_to_peer(&self, peer_id: &PeerID, payload: Arc<PeerRequest>) {
        if let Some(entry) = self.peers.get(peer_id) {
            let handler = entry.value().clone();
            let _peer_id_arc = *entry.key();

            tokio::spawn(async move {
                if handler.send(payload).await.is_err() {
                    warn!("Send failed to peer {}; disconnecting", _peer_id_arc);
                }
            });
            return;
        }
        warn!("send_to_peer: peer not found: {peer_id}");
    }

    pub(crate) async fn handle_keepalive(&self, _peer_id: PeerID) {
        debug!("Keep‑alive from {_peer_id}");
    }

    pub(crate) async fn handle_forward(
        &self,
        from_peer_id: PeerID,
        connection_peer_id: PeerID,
        to_peer_id: Option<PeerID>,
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

    async fn forward_single(&self, from_peer: PeerID, to_peer: PeerID, data: Arc<RawValue>) {
        self.send_forward(&from_peer, &to_peer, data).await;
        self.message_forwarded
            .with_labels(&[("peer_id", &to_peer.to_string())])
            .increment();
    }

    pub(crate) async fn broadcast_forward_except(
        &self,
        from_peer_id: PeerID,
        data: Arc<RawValue>,
        exclude: Option<&PeerID>,
    ) {
        info!(
            "Broadcasting from {from_peer_id} to {} peers (exclude: {exclude:?})",
            self.peers.len().saturating_sub(exclude.map_or(0, |_| 1))
        );
        let ids: Vec<PeerID> = self.peers.iter().map(|e| *e.key()).collect();
        for pid in ids {
            if exclude.is_some_and(|ex| ex == &pid) {
                continue;
            }
            self.send_forward(&from_peer_id, &pid, data.clone()).await;
        }
        self.message_broadcast.increment();
    }

    async fn send_forward(&self, from: &PeerID, to: &PeerID, data: Arc<RawValue>) {
        let req = Arc::new(PeerRequest::Forward {
            from_peer_id: *from,
            to_peer_id: Some(*to),
            data,
        });
        self.send_to_peer(to, req).await;
    }

    async fn broadcast_system_event(&self, event: PeerEvent) -> Result<(), serde_json::Error> {
        let system_event_peer_id = PeerID::from_str(SYSTEM_EVENT_PREFIX).unwrap();
        let raw = Arc::from(RawValue::from_string(serde_json::to_string(&event)?)?);
        self.broadcast_forward_except(system_event_peer_id, raw, None)
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
        to_peer_id: Option<PeerID>,
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
        to_peer_id: Option<PeerID>,
        data: Arc<RawValue>,
    ) {
        self.handle_forward(from_peer_id, connection_peer_id, to_peer_id, data)
            .await
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

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
        let peer_id = PeerID::from_str("peer_id").unwrap();
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
        let peer_id = PeerID::from_str("peer_id").unwrap();
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
        let peer_id = PeerID::from_str("peer_id").unwrap();
        let peer_handler =
            PeerHandler::new(SocketMetadata::new(origin, peer_id), tx, state.metrics());

        let peer_id_arc = state.add_peer(peer_handler.clone()).await.unwrap();
        assert_eq!(peer_id_arc.to_string(), "peer_id");

        state.remove_peer(&peer_id);
        state.remove_peer(&peer_id);
    }

    #[tokio::test]
    async fn send_to_unknow_peer() {
        let state = get_app_state();
        let peer_id = PeerID::from_str("peer_id").unwrap();
        let data: Arc<RawValue> = Arc::from(RawValue::from_string("{}".to_string()).unwrap());
        state
            .send_to_peer(
                &peer_id,
                Arc::new(PeerRequest::Forward {
                    from_peer_id: peer_id,
                    to_peer_id: None,
                    data,
                }),
            )
            .await;
    }

    #[tokio::test]
    #[traced_test]
    async fn broadcast_forward() {
        let state = get_app_state();

        let (tx, _) = mpsc::channel::<Arc<PeerRequest>>(100);

        let origin = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let peer_id = PeerID::from_str("peer_id").unwrap();
        let peer_handler =
            PeerHandler::new(SocketMetadata::new(origin, peer_id), tx, state.metrics());

        let peer_id_after_add_peer = state.add_peer(peer_handler).await.unwrap();
        assert_eq!(peer_id_after_add_peer.to_string(), "peer_id");

        let (tx_2, mut rx_2) = mpsc::channel::<Arc<PeerRequest>>(100);
        let peer_id_2 = PeerID::from_str("peer_id_2").unwrap();
        let meta_2 = SocketMetadata::new(origin, peer_id_2);
        let peer_handler_2 = PeerHandler::new(meta_2, tx_2, state.metrics());

        let peer_id_after_add_peer = state.add_peer(peer_handler_2).await.unwrap();
        assert_eq!(peer_id_after_add_peer.to_string(), "peer_id_2");

        let data: Arc<RawValue> =
            Arc::from(RawValue::from_string(r#"{"message":"hello"}"#.to_string()).unwrap());
        state
            .broadcast_forward_except(peer_id_after_add_peer, data, Some(&peer_id))
            .await;

        let received_message = rx_2.recv().await.unwrap();

        match &*received_message {
            PeerRequest::Forward {
                from_peer_id,
                to_peer_id,
                data,
            } => {
                assert_eq!(from_peer_id.to_string(), "system");
                assert_eq!(to_peer_id, &Some(peer_id_2));
                assert_eq!(
                    data.get().to_string(),
                    r#"{"new_peer":{"peer_id":"peer_id_2"}}"#
                );
            }
            _ => panic!("Expected Forward message, got: {received_message:?}"),
        }

        let received_message = rx_2.recv().await.unwrap();

        match &*received_message {
            PeerRequest::Forward {
                from_peer_id,
                to_peer_id,
                data,
            } => {
                assert_eq!(from_peer_id.to_string(), "peer_id_2");
                assert_eq!(to_peer_id, &Some(peer_id_2));
                assert_eq!(data.get().to_string(), r#"{"message":"hello"}"#);
            }
            _ => panic!("Expected Forward message, got: {received_message:?}"),
        }
    }

    #[tokio::test]
    #[traced_test]
    async fn test_app_state_interface_on_real_appstate() {
        let state = Arc::new(get_app_state());
        let origin = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);

        let (tx1, _rx1) = mpsc::channel::<Arc<PeerRequest>>(100);
        let peer_id1 = PeerID::from_str("peer1_interface_test").unwrap();
        let meta1 = SocketMetadata::new(origin, peer_id1);
        let handler1 = PeerHandler::new(meta1, tx1, state.metrics());
        state
            .add_peer(handler1)
            .await
            .expect("Failed to add peer 1");

        let (tx2, mut rx2) = mpsc::channel::<Arc<PeerRequest>>(100);
        let peer_id2 = PeerID::from_str("peer2_interface_test").unwrap();
        let meta2 = SocketMetadata::new(origin, peer_id2);
        let handler2 = PeerHandler::new(meta2, tx2, state.metrics());
        state
            .add_peer(handler2)
            .await
            .expect("Failed to add peer 2");

        let (tx3, mut rx3) = mpsc::channel::<Arc<PeerRequest>>(100);
        let peer_id3 = PeerID::from_str("peer3_interface_test").unwrap();
        let meta3 = SocketMetadata::new(origin, peer_id3);
        let handler3 = PeerHandler::new(meta3, tx3, state.metrics());
        state
            .add_peer(handler3)
            .await
            .expect("Failed to add peer 3");

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        while rx2.try_recv().is_ok() {}
        while rx3.try_recv().is_ok() {}

        debug!("Calling handle_keepalive via trait");
        AppStateInterface::handle_keepalive(state.as_ref(), peer_id1).await;
        debug!("Finished handle_keepalive via trait");

        let data: Arc<RawValue> =
            Arc::from(RawValue::from_string(r#"{"msg": "direct"}"#.to_string()).unwrap());

        debug!("Calling handle_forward (direct) via trait");
        AppStateInterface::handle_forward(
            state.as_ref(),
            peer_id1,
            peer_id1,
            Some(peer_id2),
            data.clone(),
        )
        .await;
        debug!("Finished handle_forward (direct) via trait");

        let received = rx2.recv().await.expect("Peer 2 should receive message");
        match &*received {
            PeerRequest::Forward {
                from_peer_id,
                to_peer_id,
                data: received_data,
            } => {
                assert_eq!(from_peer_id, &peer_id1, "Originating peer ID mismatch");
                assert_eq!(to_peer_id, &Some(peer_id2), "Target peer ID mismatch");
                assert_eq!(received_data.get(), r#"{"msg": "direct"}"#, "Data mismatch");
            }
            _ => panic!("Expected Forward request, got {received:?}"),
        }
        assert!(
            rx3.try_recv().is_err(),
            "Peer 3 should not receive direct message"
        );

        let broadcast_data: Arc<RawValue> =
            Arc::from(RawValue::from_string(r#"{"msg": "broadcast"}"#.to_string()).unwrap());

        debug!("Calling handle_forward (broadcast) via trait");
        AppStateInterface::handle_forward(
            state.as_ref(),
            peer_id1,
            peer_id1,
            None,
            broadcast_data.clone(),
        )
        .await;
        debug!("Finished handle_forward (broadcast) via trait");

        let received2 = rx2.recv().await.expect("Peer 2 should receive broadcast");
        match &*received2 {
            PeerRequest::Forward {
                from_peer_id,
                to_peer_id,
                data: received_data,
            } => {
                assert_eq!(from_peer_id, &peer_id1);
                assert_eq!(
                    to_peer_id.as_ref().map(|s| s.to_string()),
                    Some(peer_id2.to_string())
                );
                assert_eq!(received_data.get(), r#"{"msg": "broadcast"}"#);
            }
            _ => panic!("Expected Forward request on peer 2, got {received2:?}"),
        }

        let received3 = rx3.recv().await.expect("Peer 3 should receive broadcast");
        match &*received3 {
            PeerRequest::Forward {
                from_peer_id,
                to_peer_id,
                data: received_data,
            } => {
                assert_eq!(from_peer_id, &peer_id1);
                assert_eq!(
                    to_peer_id.as_ref().map(|s| s.to_string()),
                    Some(peer_id3.to_string())
                ); // Check target
                assert_eq!(received_data.get(), r#"{"msg": "broadcast"}"#);
            }
            _ => panic!("Expected Forward request on peer 3, got {received3:?}"),
        }
    }
}
