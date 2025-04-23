use axum::{
    extract::{ConnectInfo, State, WebSocketUpgrade, ws::Message},
    response::IntoResponse,
};
use futures::{Sink, SinkExt, Stream, StreamExt};
use sf_logging::{debug, error, info, warn};
use sf_metrics::Metrics;
use sf_protocol::PeerRequest;
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::mpsc;
// use tracing::warn;

use crate::{
    extract_peer_id::ExtractPeerID, peer_handler::PeerHandler, socket_metadata::SocketMetadata,
    state::AppState,
};

pub async fn ws_handler<M>(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState<M>>>,
    ExtractPeerID(peer_id): ExtractPeerID,
    ConnectInfo(origin): ConnectInfo<SocketAddr>,
) -> impl IntoResponse
where
    M: Metrics + Clone + Send + Sync + 'static,
{
    let meta = SocketMetadata::new(origin, peer_id);

    info!(
        "Upgrading to WebSocket — origin = {}, peer_id = {}",
        meta.origin, meta.peer_id
    );

    ws.on_upgrade(|ws| {
        let (write, read) = ws.split();
        handle_ws_connection(write, read, state, meta)
    })
}

async fn handle_ws_connection<M, W, R>(
    write: W,
    read: R,
    state: Arc<AppState<M>>,
    meta: SocketMetadata,
) where
    M: Metrics + Clone + Send + Sync + 'static,
    W: Sink<Message> + Unpin,
    W::Error: std::fmt::Debug,
    R: Stream<Item = Result<Message, axum::Error>> + Unpin,
{
    let (tx, rx) = mpsc::channel::<Arc<PeerRequest>>(32);
    let handler = PeerHandler::new(meta.clone(), tx, state.metrics());

    if let Err(_e) = state.add_peer(handler.clone()).await {
        error!("Failed to register peer {}: {_e}", handler.id());
        return;
    }

    info!(
        "WebSocket connected — origin = {}, peer_id = {}",
        meta.origin,
        handler.id()
    );

    process_ws(write, read, rx, handler, state).await
}

async fn process_ws<M, W, R>(
    mut write: W,
    mut read: R,
    mut outbound_rx: mpsc::Receiver<Arc<PeerRequest>>,
    handler: PeerHandler,
    state: Arc<AppState<M>>,
) where
    M: Metrics + Clone + Send + Sync + 'static,
    W: Sink<Message> + Unpin,
    W::Error: std::fmt::Debug,
    R: Stream<Item = Result<Message, axum::Error>> + Unpin,
{
    let peer_id = handler.id();

    loop {
        tokio::select! {
            Some(event) = outbound_rx.recv() => {
                debug!("Sending event to {peer_id}: {event:?}");
                match serde_json::to_string(&*event) {
                    Ok(text) => {
                        if let Err(_e) = write.send(Message::Text(text.into())).await {
                            warn!("Failed to send message to {peer_id}: {:?}. Closing connection", _e);
                            break;
                        }
                    }
                    Err(_e) => { // we shall never fall here
                        warn!("Failed to serialize message for {peer_id}: {}. Closing connection", _e);
                        break;
                    }
                }
            },
            Some(msg) = read.next() => {
                match msg {
                    Ok(msg) => { if !handler.process_incoming(msg, &state).await {break;} },
                    Err(_e) => {
                        warn!("Error receiving from {}: {_e}", peer_id);
                        break;
                    }
                }
            },
        }
    }

    info!("Connection closed — peer_id = {}", peer_id);
    state.remove_peer(peer_id);
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use crate::peer_id::PeerID;

    use super::*;
    use std::{
        net::{Ipv4Addr, SocketAddr},
        str::FromStr,
        sync::Arc,
        time::Duration,
    };

    use axum::{Router, extract::connect_info::IntoMakeServiceWithConnectInfo, routing::get};
    use sf_metrics::InMemoryMetrics;
    use sf_protocol::PeerRequest;
    use tracing_test::traced_test;

    fn get_router_and_state() -> (
        IntoMakeServiceWithConnectInfo<Router, SocketAddr>,
        Arc<AppState<InMemoryMetrics>>,
    ) {
        let state = Arc::new(AppState::new(InMemoryMetrics::new()));
        let app = Router::new()
            .route("/ws", get(ws_handler::<InMemoryMetrics>))
            .with_state(state.clone())
            .into_make_service_with_connect_info::<SocketAddr>();
        (app, state)
    }

    async fn setup_ws_connection(
        test_peer_id: &str,
    ) -> (
        tokio::task::JoinHandle<Result<(), std::io::Error>>,
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        SocketAddr,
        Arc<AppState<InMemoryMetrics>>,
    ) {
        let (app, state) = get_router_and_state();
        let listener = tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();
        let server_task = tokio::spawn(axum::serve(listener, app).into_future());

        let connect_url = format!("ws://{addr}/ws?peer_id={test_peer_id}");

        let mut attempt = 0;
        let (ws_stream, _) = loop {
            match tokio_tungstenite::connect_async(&connect_url).await {
                Ok(result) => break result,
                Err(e) => {
                    attempt += 1;
                    if attempt > 5 {
                        panic!("Failed to connect to WS after multiple attempts: {}", e);
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            }
        };

        (server_task, ws_stream, addr, state)
    }

    #[tokio::test]
    async fn test_ws_upgrade() {
        let (server_task, ws_stream, _addr, _state) =
            setup_ws_connection("test_peer_upgrade").await;

        drop(ws_stream);
        tokio::time::sleep(Duration::from_millis(100)).await;
        server_task.abort();
    }

    #[tokio::test]
    #[traced_test]
    async fn test_process_ws_outbound_send_error() {
        let test_peer_id = "peer_send_error";
        let (server_task, mut ws_stream, _addr, state) = setup_ws_connection(test_peer_id).await;

        assert!(
            state.peers.contains_key(test_peer_id),
            "Peer should be in state after connection"
        );

        ws_stream.close(None).await.ok();
        drop(ws_stream);

        tokio::time::sleep(Duration::from_millis(100)).await;

        let dummy_request = Arc::new(PeerRequest::KeepAlive);
        println!("sending");
        state.send_to_peer(test_peer_id, dummy_request).await;

        tokio::time::sleep(Duration::from_millis(200)).await;

        assert!(
            !state.peers.contains_key(test_peer_id),
            "Peer should be removed after send error"
        );

        server_task.abort();
    }

    #[tokio::test]
    #[traced_test]
    async fn test_websocket_send_failure_warning() {
        let (_, state) = get_router_and_state();
        let (tx, rx) = mpsc::channel::<Arc<PeerRequest>>(32);
        let meta = SocketMetadata::new(
            SocketAddr::from_str("127.0.0.1:12312").unwrap(),
            PeerID::new("toto".to_string()),
        );
        let handler = PeerHandler::new(meta, tx, state.metrics());

        let (mut socket_write, _) = futures::channel::mpsc::channel(1024);
        let (_, socket_read) = futures::channel::mpsc::channel(1024);
        socket_write.close().await.unwrap();
        tokio::spawn(process_ws(
            socket_write,
            socket_read,
            rx,
            handler.clone(),
            state,
        ));
        handler
            .send(Arc::new(PeerRequest::KeepAlive))
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;

        #[cfg(not(coverage))]
        assert!(
            logs_contain("Failed to send message to toto: SendError { "),
            "log not found"
        );
    }

    #[tokio::test]
    #[traced_test]
    async fn test_register_twice_the_same_peer() {
        let (app, _) = get_router_and_state();
        let listener = tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();
        let server_task = tokio::spawn(axum::serve(listener, app).into_future());

        let connect_url = format!("ws://{addr}/ws?peer_id=toto");

        let mut attempt = 0;
        let (ws_stream, _) = loop {
            match tokio_tungstenite::connect_async(&connect_url).await {
                Ok(result) => break result,
                Err(e) => {
                    attempt += 1;
                    if attempt > 5 {
                        panic!("Failed to connect to WS after multiple attempts: {}", e);
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            }
        };

        let (ws_stream_2, _) = loop {
            match tokio_tungstenite::connect_async(&connect_url).await {
                Ok(result) => break result,
                Err(e) => {
                    attempt += 1;
                    if attempt > 5 {
                        panic!("Failed to connect to WS after multiple attempts: {}", e);
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            }
        };

        #[cfg(not(coverage))]
        assert!(
            logs_contain("Failed to register peer toto: Peer already exists: toto"),
            "log not found"
        );

        _ = ws_stream;
        _ = ws_stream_2;

        server_task.abort();
    }
}
