use axum::{
    extract::{
        ConnectInfo, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use sf_logging::{debug, error, info, warn};
use sf_metrics::Metrics;
use sf_protocol::PeerRequest;
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::mpsc;

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

    ws.on_upgrade(|ws| handle_ws_connection(ws, state, meta))
}

async fn handle_ws_connection<M>(ws: WebSocket, state: Arc<AppState<M>>, meta: SocketMetadata)
where
    M: Metrics + Clone + Send + Sync + 'static,
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

    tokio::spawn(process_ws(ws, rx, handler, state));
}

async fn process_ws<M>(
    websocket: WebSocket,
    mut outbound_rx: mpsc::Receiver<Arc<PeerRequest>>,
    handler: PeerHandler,
    state: Arc<AppState<M>>,
) where
    M: Metrics + Clone + Send + Sync + 'static,
{
    let peer_id = handler.id();

    let (mut sink, mut stream) = websocket.split();

    loop {
        tokio::select! {
            Some(event) = outbound_rx.recv() => {
                debug!("Sending event to {peer_id}: {event:?}");
                match serde_json::to_string(&*event) {
                    Ok(text) => {
                        if let Err(_e) = sink.send(Message::Text(text.into())).await {
                            warn!("Failed to send message to {peer_id}: {}. Closing connection", _e);
                            break;
                        }
                    }
                    Err(_e) => {
                        warn!("Failed to serialize message for {peer_id}: {}. Closing connection", _e);
                        break;
                    }
                }
            },
            Some(msg) = stream.next() => {
                match msg {
                    Ok(msg) => { if !handler.process_incoming(msg, &state).await {break;} },
                    Err(_e) => {
                        warn!("Error receiving from {}: {_e}", peer_id);
                        break;
                    }
                }
            },
            else => break,
        }
    }

    info!("Connection closed — peer_id = {}", peer_id);
    state.remove_peer(&peer_id);
}
#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use std::{
        net::{Ipv4Addr, SocketAddr},
        sync::Arc,
        time::Duration,
    };

    use axum::{Router, extract::connect_info::IntoMakeServiceWithConnectInfo, routing::get};
    use sf_metrics::InMemoryMetrics;

    use crate::{state::AppState, ws_handler};

    fn get_router() -> IntoMakeServiceWithConnectInfo<Router, SocketAddr> {
        let state = Arc::new(AppState::new(InMemoryMetrics::new()));

        Router::new()
            .route("/ws", get(ws_handler::<InMemoryMetrics>))
            .with_state(state)
            .into_make_service_with_connect_info::<SocketAddr>()
    }

    #[tokio::test]
    async fn test_ws_upgrade() {
        let app = get_router();

        let listener = tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
            .await
            .unwrap();

        let addr = listener.local_addr().unwrap();
        let server_task = tokio::spawn(axum::serve(listener, app).into_future());

        let test_peer_id = "test_peer_id";
        let (ws_stream, _) =
            tokio_tungstenite::connect_async(format!("ws://{addr}/ws?peer_id={test_peer_id}"))
                .await
                .unwrap();

        // Give the server a moment to process the connection and emit logs
        tokio::time::sleep(Duration::from_millis(100)).await;
        drop(ws_stream);
        server_task.abort();
    }
}
