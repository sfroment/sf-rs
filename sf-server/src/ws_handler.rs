use axum::{
    extract::{
        ConnectInfo, Path, Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::HeaderMap,
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use sf_metrics::Metrics;
use sf_protocol::PeerRequest;
use std::{collections::HashMap, fmt::Debug, net::SocketAddr, sync::Arc};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::{peer_handler::PeerHandler, peer_id::ExtractPeerID, state::AppState};

#[derive(Debug, Clone)]
pub struct WsUpgradeMeta {
    pub(crate) origin: SocketAddr,
    pub(crate) peer_id: String,
    pub(crate) _path: Option<String>,
    pub(crate) _query_params: HashMap<String, String>,
    pub(crate) _headers: HeaderMap,
}

pub async fn ws_handler<M>(
    ws: WebSocketUpgrade,
    path: Option<Path<String>>,
    headers: HeaderMap,
    State(state): State<Arc<AppState<M>>>,
    Query(query_params): Query<HashMap<String, String>>,
    ExtractPeerID(peer_id): ExtractPeerID,
    ConnectInfo(origin): ConnectInfo<SocketAddr>,
) -> impl IntoResponse
where
    M: Metrics + Clone + Send + Sync + 'static,
{
    let meta = WsUpgradeMeta {
        origin,
        peer_id: peer_id.clone(),
        _headers: headers,
        _query_params: query_params,
        _path: path.map(|p| p.0),
    };

    info!(
        "Upgrading connection: {:?}, peer_id: {:?}",
        meta.origin, peer_id,
    );

    ws.on_upgrade(move |ws| {
        info!(
            "WebSocket connection established for peer_id: {}, origin: {}",
            peer_id, meta.origin
        );
        spawn_peer_task(ws, state, meta)
    })
}

async fn spawn_peer_task<M>(ws: WebSocket, state: Arc<AppState<M>>, meta: WsUpgradeMeta)
where
    M: Metrics + Clone + Send + Sync + 'static,
{
    let (tx, rx): (
        mpsc::Sender<Arc<PeerRequest>>,
        mpsc::Receiver<Arc<PeerRequest>>,
    ) = mpsc::channel(32);

    let peer_handler = PeerHandler::new(meta, tx, state.metrics());
    let peer_handler_clone_for_task = peer_handler.clone();

    match state.add_peer(peer_handler).await {
        Ok(peer_id_arc) => {
            info!(
                "Peer {} added successfully, spawning handler task.",
                peer_id_arc
            );
            tokio::spawn(handle_connection_task(
                ws,
                state,
                rx,
                peer_handler_clone_for_task,
            ));
        }
        Err(e) => {
            error!(
                "Failed to add peer {} to state: {}. Closing connection attempt.",
                peer_handler_clone_for_task.peer_id(),
                e
            );
        }
    }
}

async fn handle_connection_task<M>(
    websocket: WebSocket,
    state: Arc<AppState<M>>,
    mut command_rx: mpsc::Receiver<Arc<PeerRequest>>,
    peer_handler: PeerHandler,
) where
    M: Metrics + Clone + Send + Sync + 'static,
{
    let peer_id_str = peer_handler.peer_id();
    info!("Starting connection handler task for peer {}", peer_id_str);

    let (mut ws_sender, mut ws_receiver) = websocket.split();

    loop {
        tokio::select! {
            Some(event_to_send) = command_rx.recv() => {
                debug!("Task for peer {} received event command: {:?}", peer_id_str, event_to_send);
                match serde_json::to_string(&*event_to_send) {
                    Ok(text_message) => {
                        if let Err(e) = ws_sender.send(Message::Text(text_message.into())).await {
                            error!("WebSocket send error to peer {}: {}", peer_id_str, e);
                            break;
                        }
                    },
                    Err(e) => {
                        error!("Failed to serialize PeerEvent for peer {}: {}", peer_id_str, e);
                    }
                }
            }

            Some(msg_result) = ws_receiver.next() => {
                match msg_result {
                    Ok(msg) => {
                        if !peer_handler.process_incoming(msg, &state).await {
                            break;
                        }
                    }
                    Err(e) => {
                        warn!("WebSocket receive error from peer {}: {}. Closing.", peer_id_str, e);
                        break;
                    }
                }
            }
            else => {
                info!("WebSocket receiver or command channel closed for peer {}. Shutting down task.", peer_id_str);
                break;
            }
        }
    }
    info!("Closing connection handler task for peer {}", peer_id_str);
    state.remove_peer(peer_id_str);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use axum::{
        Router,
        http::{StatusCode, header},
        routing::get,
    };
    use sf_metrics::InMemoryMetrics;
    use sf_protocol::PeerRequest;
    use std::{
        net::{Ipv4Addr, SocketAddr},
        sync::Arc,
        time::Duration,
    };
    use tokio_tungstenite::tungstenite::Message as TMessage;

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn ws_handler_should_upgrade_to_websocket() {
        let state = Arc::new(AppState::new(InMemoryMetrics::new()));

        let app = Router::new()
            .route("/ws", get(ws_handler::<InMemoryMetrics>))
            .with_state(state)
            .into_make_service_with_connect_info::<SocketAddr>();

        let listener = tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(axum::serve(listener, app).into_future());

        let (_, _response) =
            tokio_tungstenite::connect_async(format!("ws://{addr}/ws?peer_id=test"))
                .await
                .unwrap();

        assert_eq!(StatusCode::SWITCHING_PROTOCOLS, _response.status());

        println!("{:?}", _response);
        let headers = _response.headers();

        assert_eq!(
            headers
                .get(header::CONNECTION)
                .unwrap()
                .to_str()
                .unwrap()
                .to_lowercase(),
            "upgrade"
        );
        assert_eq!(
            headers
                .get(header::UPGRADE)
                .unwrap()
                .to_str()
                .unwrap()
                .to_lowercase(),
            "websocket"
        );
        assert!(
            headers.get(header::SEC_WEBSOCKET_ACCEPT).is_some(),
            "missing Sec‑WebSocket‑Accept"
        );
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn test_spawn_peer_task_success_and_duplicate() {
        let state = Arc::new(AppState::new(InMemoryMetrics::new()));

        let app = Router::new()
            .route("/ws", get(ws_handler::<InMemoryMetrics>))
            .with_state(state.clone())
            .into_make_service_with_connect_info::<SocketAddr>();

        let listener = tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();
        let server_task = tokio::spawn(axum::serve(listener, app).into_future());

        let (ws_stream1, _) =
            tokio_tungstenite::connect_async(format!("ws://{addr}/ws?peer_id=test_peer_1"))
                .await
                .unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let (ws_stream2, _) =
            tokio_tungstenite::connect_async(format!("ws://{addr}/ws?peer_id=test_peer_1"))
                .await
                .unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        drop(ws_stream1);
        drop(ws_stream2);
        server_task.abort();
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn test_handle_connection_task_basic_workflow() {
        let state = Arc::new(AppState::new(InMemoryMetrics::new()));

        let app = Router::new()
            .route("/ws", get(ws_handler::<InMemoryMetrics>))
            .with_state(state.clone())
            .into_make_service_with_connect_info::<SocketAddr>();

        let listener = tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();
        let server_task = tokio::spawn(axum::serve(listener, app).into_future());

        let (mut ws_stream, _) =
            tokio_tungstenite::connect_async(format!("ws://{addr}/ws?peer_id=test_workflow"))
                .await
                .unwrap();

        let keep_alive_msg = TMessage::Text(
            serde_json::to_string(&PeerRequest::KeepAlive)
                .unwrap()
                .into(),
        );
        ws_stream.send(keep_alive_msg).await.unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;

        let close_msg = TMessage::Close(None);
        ws_stream.send(close_msg).await.unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;

        server_task.abort();
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn test_handle_connection_task_ping_pong() {
        let state = Arc::new(AppState::new(InMemoryMetrics::new()));

        let app = Router::new()
            .route("/ws", get(ws_handler::<InMemoryMetrics>))
            .with_state(state.clone())
            .into_make_service_with_connect_info::<SocketAddr>();

        let listener = tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();
        let server_task = tokio::spawn(axum::serve(listener, app).into_future());

        let (mut ws_stream, _) =
            tokio_tungstenite::connect_async(format!("ws://{addr}/ws?peer_id=test_ping"))
                .await
                .unwrap();

        let ping_data = vec![1, 2, 3, 4];
        let ping_msg = TMessage::Ping(ping_data.clone().into());
        ws_stream.send(ping_msg).await.unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;

        ws_stream.close(None).await.unwrap();
        server_task.abort();
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn test_handle_connection_task_binary_message() {
        let state = Arc::new(AppState::new(InMemoryMetrics::new()));

        let app = Router::new()
            .route("/ws", get(ws_handler::<InMemoryMetrics>))
            .with_state(state.clone())
            .into_make_service_with_connect_info::<SocketAddr>();

        let listener = tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();
        let server_task = tokio::spawn(axum::serve(listener, app).into_future());

        let (mut ws_stream, _) =
            tokio_tungstenite::connect_async(format!("ws://{addr}/ws?peer_id=test_binary"))
                .await
                .unwrap();

        let binary_data = vec![1, 2, 3, 4];
        let binary_msg = TMessage::Binary(binary_data.into());
        ws_stream.send(binary_msg).await.unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;

        ws_stream.close(None).await.unwrap();
        server_task.abort();
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn test_handle_connection_task_invalid_json() {
        let state = Arc::new(AppState::new(InMemoryMetrics::new()));

        let app = Router::new()
            .route("/ws", get(ws_handler::<InMemoryMetrics>))
            .with_state(state.clone())
            .into_make_service_with_connect_info::<SocketAddr>();

        let listener = tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();
        let server_task = tokio::spawn(axum::serve(listener, app).into_future());

        let (mut ws_stream, _) =
            tokio_tungstenite::connect_async(format!("ws://{addr}/ws?peer_id=test_invalid_json"))
                .await
                .unwrap();

        let invalid_json_msg = TMessage::Text("this is not valid JSON".to_string().into());
        ws_stream.send(invalid_json_msg).await.unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;

        ws_stream.close(None).await.unwrap();
        server_task.abort();
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn test_handle_connection_task_bidirectional_forward() {
        let state = Arc::new(AppState::new(InMemoryMetrics::new()));

        let app = Router::new()
            .route("/ws", get(ws_handler::<InMemoryMetrics>))
            .with_state(state.clone())
            .into_make_service_with_connect_info::<SocketAddr>();

        let listener = tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();
        let server_task = tokio::spawn(axum::serve(listener, app).into_future());

        let (mut sender_ws, _) =
            tokio_tungstenite::connect_async(format!("ws://{addr}/ws?peer_id=test_sender"))
                .await
                .unwrap();

        let (mut receiver_ws, _) =
            tokio_tungstenite::connect_async(format!("ws://{addr}/ws?peer_id=test_receiver"))
                .await
                .unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;

        let raw_data =
            serde_json::value::RawValue::from_string(r#"{"message":"hello"}"#.to_string()).unwrap();
        let forward_msg = PeerRequest::Forward {
            from_peer_id: Arc::new("test_sender".to_string()),
            to_peer_id: Some("test_receiver".to_string()),
            data: Arc::from(raw_data),
        };

        let forward_json = serde_json::to_string(&forward_msg).unwrap();
        sender_ws
            .send(TMessage::Text(forward_json.into()))
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(200)).await;

        sender_ws.close(None).await.unwrap();
        receiver_ws.close(None).await.unwrap();
        server_task.abort();
    }
}
