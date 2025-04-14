use axum::Extension;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, Path, Query};
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use futures::{SinkExt, StreamExt, stream::SplitStream};
use std::collections::HashMap;
use std::net::SocketAddr;
use tracing::info;

#[derive(Debug, Clone)]
pub struct WsUpgradeMeta {
    pub origin: SocketAddr,
    pub path: Option<String>,
    pub query_params: HashMap<String, String>,
    pub headers: HeaderMap,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    path: Option<Path<String>>,
    headers: HeaderMap,
    Query(query_params): Query<HashMap<String, String>>,
    Extension(peer_id): Extension<String>,
    ConnectInfo(origin): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
    let meta = WsUpgradeMeta {
        origin,
        headers,
        query_params,
        path: path.map(|p| p.0),
    };

    info!(
        "Upgrading connection: {:?}, peer_id: {:?}",
        meta.origin, "123"
    );

    ws.on_upgrade(move |ws| handle_socket(meta, ws))
}

async fn handle_socket(meta: WsUpgradeMeta, mut ws: WebSocket) {
    if let Err(e) = ws.send(Message::Text("Welcome to sf-ice!".into())).await {
        tracing::error!("Error sending welcome message: {}", e);
        return;
    }

    let (mut ws_sink, mut receiver) = ws.split();
    while let Some(msg) = receiver.next().await {
        let msg = if let Ok(msg) = msg {
            msg
        } else {
            // Client disconnected or error occurred
            tracing::warn!("Client disconnected");
            return;
        };

        match msg {
            Message::Text(text) => {
                tracing::debug!("Received text message: {}", text);
                if let Err(e) = ws_sink.send(Message::Text(text)).await {
                    tracing::error!("Error sending message: {}", e);
                    return;
                }
            }
            Message::Binary(data) => {
                tracing::debug!("Received binary message, length: {}", data.len());
                // Echo binary messages back
                if let Err(e) = ws_sink.send(Message::Binary(data)).await {
                    tracing::error!("Error sending binary message: {}", e);
                    return;
                }
            }
            Message::Ping(data) => {
                // Automatically respond to pings
                if let Err(e) = ws_sink.send(Message::Pong(data)).await {
                    tracing::error!("Error sending pong: {}", e);
                    return;
                }
            }
            Message::Close(_) => {
                tracing::info!("Client sent close message");
                return;
            }
            _ => {}
        }
    }
}
