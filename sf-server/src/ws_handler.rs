use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, Path, Query, State};
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use futures::{SinkExt, StreamExt};
use sf_metrics::Metrics;
use sf_protocol::PeerRequest;
use std::collections::HashMap;
use std::fmt::Debug;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::peer_handler::PeerHandler;
use crate::peer_id::ExtractPeerID;
use crate::state::AppState;

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
