use futures::{SinkExt, Stream, StreamExt, channel::mpsc};
use gloo_net::websocket::{Message, WebSocketError, futures::WebSocket};
use sf_peer_id::PeerID;
use std::{cell::RefCell, rc::Rc};
use tracing::{error, info, warn};
use wasm_bindgen::JsError;
use wasm_bindgen_futures::spawn_local;

use crate::{Client, WsSenderState};

const CHANNEL_BUFFER_SIZE: usize = 32;

pub struct WebSocketConnection {
    url: String,

    sender_state: WsSenderState,
}

impl WebSocketConnection {
    pub fn connect(url: &str, peer_id: PeerID, client: Rc<Client>) -> Result<Self, JsError> {
        let ws_url = format!("ws://{url}/ws?peer_id={peer_id}");
        info!("Connecting to {}", ws_url);

        let ws = WebSocket::open(&ws_url)?;
        info!("WebSocket opened");

        let (write, read) = ws.split();

        let (sender, receiver) = mpsc::channel(CHANNEL_BUFFER_SIZE);
        let sender_state: WsSenderState = Rc::new(RefCell::new(Some(sender)));

        spawn_local(websocket_writer_loop(write, receiver));
        spawn_local(websocket_reader_loop(read, client));

        info!("Loops spawned");

        Ok(WebSocketConnection {
            url: ws_url,
            sender_state,
        })
    }

    #[inline]
    pub fn sender(&self) -> WsSenderState {
        self.sender_state.clone()
    }

    pub async fn send(&self, message: Message) -> Result<(), JsError> {
        let mut sender = self
            .sender_state
            .try_borrow_mut()
            .map_err(|e| JsError::new(&format!("Failed to borrow WsSender state: {}", e)))?
            .as_mut()
            .ok_or_else(|| JsError::new("WebSocket sender unavailable (already taken or None)"))?
            .clone();

        sender.send(message).await.map_err(|e| {
            error!(
                "WebSocketConnection: Failed to send message via sender channel: {:?}",
                e
            );
            JsError::new(&format!("Failed to queue message for WebSocket: {}", e))
        })
    }
}

async fn websocket_writer_loop(
    mut write: impl SinkExt<Message, Error = WebSocketError> + Unpin,
    mut receiver: mpsc::Receiver<Message>,
) {
    info!("WebSocket writer task started.");
    while let Some(msg) = receiver.next().await {
        info!("WebSocket writer: Sending message...");
        if let Err(e) = write.send(msg).await {
            error!("WebSocket writer: Send error: {}", e);
            break;
        }
    }
    info!("WebSocket writer task finished.");
    // if let Err(e) = write.close().await {
    //     error!("WebSocket writer: Error closing write sink: {}", e);
    // }
}

async fn websocket_reader_loop<R>(mut read: R, client: Rc<Client>)
where
    R: Stream<Item = Result<Message, WebSocketError>> + Unpin + 'static,
{
    info!("WebSocket reader task started.");
    while let Some(msg_result) = read.next().await {
        match msg_result {
            Ok(Message::Text(text)) => {
                client.handle_incoming_text(&text).await;
            }
            Ok(Message::Bytes(_)) => {
                warn!("WebSocket reader: Received unexpected binary message, ignoring.");
            }
            Err(e) => {
                error!("WebSocket reader: Read error: {}", e);
                // TODO: add reconnection logic
                break;
            }
        }
    }
    info!("WebSocket reader task finished.");
    // TODO: Notify client that the reader has stopped
}
