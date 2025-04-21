use axum::{
    extract::{ConnectInfo, State, WebSocketUpgrade},
    response::IntoResponse,
};
use sf_logging::info;
use sf_metrics::Metrics;
use std::{net::SocketAddr, sync::Arc};

use crate::{extract_peer_id::ExtractPeerID, socket_metadata::SocketMetadata, state::AppState};

pub async fn ws_handler<M>(
    ws: WebSocketUpgrade,
    State(_state): State<Arc<AppState<M>>>,
    ExtractPeerID(peer_id): ExtractPeerID,
    ConnectInfo(origin): ConnectInfo<SocketAddr>,
) -> impl IntoResponse
where
    M: Metrics + Clone + Send + Sync + 'static,
{
    let _meta = SocketMetadata::new(origin, peer_id);

    info!("Upgrading connection: {:?}", _meta);

    ws.on_upgrade(move |_ws| {
        info!("Websocket upgraded origin: {}", origin);
        async {}
    })
}

// async fn handle_ws_connection<M>(ws: WebSocket, state: Arc<AppState<M>>, meta: SocketMetadata)
// where
//     M: Metrics + Clone + Send + Sync + 'static,
// {
//     let (tx, rx) = mpsc::channel::<Arc<PeerRequest>>(32);

//     let handler = PeerHandler::new(meta.clone(), tx, state.metrics());

//     if let Err(e) = state.add_peer(handler.clone()).await {
//         error!("Failed to register peer {}: {e}", handler.peer_id());
//         return;
//     }

//     let peer_id = handler.peer_id();

//     info!(
//         "WebSocket connected — origin = {}, peer_id = {}",
//         meta.origin, peer_id
//     );

//     tokio::spawn(process_ws(ws, rx, handler, state));
// }

#[cfg(test)]
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
