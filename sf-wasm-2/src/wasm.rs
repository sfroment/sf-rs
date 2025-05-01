use futures::{SinkExt, Stream, StreamExt, channel::mpsc};
use gloo_console::{error, log, warn};
use gloo_net::websocket::{Message, WebSocketError, futures::WebSocket};
use sf_peer_id::PeerID;
use sf_protocol::{PeerEvent, PeerRequest};
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    rc::Rc,
    sync::{Arc, Mutex, MutexGuard},
};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::callback::{JsCallback, JsCallbackManager};

type WsSender = RefCell<Option<mpsc::Sender<Message>>>;

const CHANNEL_BUFFER_SIZE: usize = 32;

pub struct Client {
    peer_id: PeerID,

    message_callbacks: JsCallbackManager,
    new_peer_callbacks: JsCallbackManager,

    sender: WsSender,

    peers: RefCell<Vec<PeerID>>,
}

impl Client {
    pub fn new() -> Result<Rc<Self>, JsValue> {
        let peer_id = PeerID::random().map_err(|e| JsValue::from(e.to_string()))?;
        Ok(Rc::new(Self {
            peer_id,
            message_callbacks: JsCallbackManager::new(),
            new_peer_callbacks: JsCallbackManager::new(),
            sender: RefCell::new(None),
            // Initialize with RefCell
            peers: RefCell::new(Vec::new()),
        }))
    }

    pub fn peer_id(self: &Rc<Self>) -> PeerID {
        self.peer_id
    }

    pub fn add_on_message_callback(&self, callback: JsCallback) -> usize {
        self.message_callbacks.add(callback)
    }

    pub fn remove_on_message_callback(&self, id: usize) {
        self.message_callbacks.remove(id);
    }

    pub fn add_on_new_peer_callback(&self, callback: JsCallback) -> usize {
        self.new_peer_callbacks.add(callback)
    }

    pub fn remove_on_new_peer_callback(&self, id: usize) {
        self.new_peer_callbacks.remove(id);
    }

    pub fn connect(self: &Rc<Self>, url: &str) -> Result<(), JsError> {
        let ws_url = format!("ws://{url}/ws?peer_id={}", self.peer_id);
        log!("Connecting to: {ws_url}");

        let ws = WebSocket::open(&ws_url)?;
        let (write, read) = ws.split();

        let (sender, receiver) = mpsc::channel(CHANNEL_BUFFER_SIZE);
        *self.sender.borrow_mut() = Some(sender);

        spawn_local(Self::websocket_writer_loop(write, receiver));

        let client_clone = self.clone();
        spawn_local(client_clone.websocket_reader_loop(read));

        Ok(())
    }

    async fn websocket_writer_loop(
        mut write: impl SinkExt<Message, Error = WebSocketError> + Unpin,
        mut receiver: mpsc::Receiver<Message>,
    ) {
        log!("WebSocket writer task started.");
        while let Some(msg) = receiver.next().await {
            if let Err(e) = write.send(msg).await {
                error!("WebSocket send error:", e.to_string());
                break;
            }
        }
        log!("WebSocket writer task finished.");
    }

    async fn websocket_reader_loop<R>(self: Rc<Self>, mut read: R)
    where
        R: Stream<Item = Result<Message, WebSocketError>> + Unpin + 'static,
    {
        log!("WebSocket reader task started.");
        while let Some(msg_result) = read.next().await {
            match msg_result {
                Ok(Message::Text(text)) => {
                    self.handle_incoming_text(&text);
                }
                Ok(Message::Bytes(_)) => {
                    warn!("Received unexpected binary message, ignoring.");
                }
                Err(e) => {
                    error!("WebSocket read error:", e.to_string());
                    break;
                }
            }
        }
        log!("WebSocket reader task finished.");
    }

    fn handle_incoming_text(self: &Rc<Self>, text: &str) {
        match serde_json::from_str::<PeerRequest>(text) {
            Ok(peer_request) => {
                log!("Received PeerRequest:", format!("{peer_request:?}"));
                match peer_request {
                    PeerRequest::Forward { data, .. } => match data {
                        PeerEvent::NewPeer { peer_id } => {
                            let peer_id_str = peer_id.to_string();
                            log!(format!("New Peer discovered: {peer_id_str}"));

                            {
                                let mut peers = self.peers.borrow_mut();
                                if peers.contains(&peer_id) {
                                    log!(format!(
                                        "Received NewPeer for already known peer: {peer_id_str}"
                                    ));
                                    return;
                                }

                                peers.push(peer_id);
                            }

                            log!(format!("New Peer added: {peer_id_str}"));
                            let callbacks = self.new_peer_callbacks.borrow_callbacks();
                            for callback in callbacks.values() {
                                Self::invoke_new_peer_callback(callback, &peer_id);
                            }
                        }
                        PeerEvent::Message { peer_id, message } => {
                            let peer_id_str = peer_id.to_string();
                            log!(format!("Message from Peer: {peer_id_str}"));

                            let callbacks = self.message_callbacks.borrow_callbacks();
                            for callback in callbacks.values() {
                                Self::invoke_message_callback(callback, &peer_id, &message);
                            }
                        }
                    },
                    _ => {
                        warn!(format!(
                            "Received unhandled PeerRequest type: {peer_request:?}",
                        ));
                    }
                }
            }
            Err(e) => {
                error!("Failed to deserialize incoming message:", e.to_string());
                error!("Received text:", text);
            }
        };
    }

    fn invoke_new_peer_callback(callback: &JsCallback, peer_id: &PeerID) {
        match serde_wasm_bindgen::to_value(&peer_id.to_string()) {
            Ok(peer_id_js) => {
                let this = JsValue::NULL;
                if let Err(e) = callback.call1(&this, &peer_id_js) {
                    error!("Error calling on_new_peer callback:", format!("{e:?}"));
                }
            }
            Err(e) => {
                error!("Failed to serialize peer_id for callback:", e.to_string());
            }
        }
    }

    fn invoke_message_callback(callback: &JsCallback, peer_id: &PeerID, message: &str) {
        match serde_wasm_bindgen::to_value(&peer_id.to_string()) {
            Ok(peer_id_js) => match serde_wasm_bindgen::to_value(message) {
                Ok(message_js) => {
                    let this = JsValue::NULL;
                    if let Err(e) = callback.call2(&this, &peer_id_js, &message_js) {
                        error!("Error calling on_message callback:", format!("{e:?}"));
                    }
                }
                Err(e) => {
                    error!("Failed to serialize message for callback:", e.to_string());
                }
            },
            Err(e) => {
                error!("Failed to serialize peer_id for callback:", e.to_string());
            }
        }
    }

    fn send_peer_request(&self, peer_request: PeerRequest) -> Result<(), JsError> {
        let sender_opt = self.sender.borrow();
        if let Some(sender) = sender_opt.as_ref() {
            let text = serde_json::to_string(&peer_request)
                .map_err(|e| JsError::new(&format!("Failed to serialize PeerRequest: {e}")))?;

            let message = Message::Text(text);
            let mut sender_clone = sender.clone();

            spawn_local(async move {
                log!("Queueing message for sending: {:?}", peer_request);
                if let Err(e) = sender_clone.send(message).await {
                    error!("Failed to queue message for WebSocket: {:?}", e.to_string());
                }
            });
            Ok(())
        } else {
            Err(JsError::new(
                "WebSocket not connected or sender unavailable",
            ))
        }
    }
}

#[wasm_bindgen]
pub struct ClientWrapper {
    client: Rc<Client>,
}

#[wasm_bindgen]
impl ClientWrapper {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<Self, JsValue> {
        let client = Client::new()?;
        Ok(Self { client })
    }

    #[wasm_bindgen(getter)]
    pub fn peer_id(&self) -> String {
        self.client.peer_id().to_string()
    }

    #[wasm_bindgen(getter)]
    pub fn list_peers(&self) -> Vec<String> {
        let peers: Vec<String> = self
            .client
            .peers
            .borrow()
            .iter()
            .map(|p| p.to_string())
            .collect();
        log!("peers: {:?}", peers.clone());
        peers
    }

    pub fn connect(&self, url: &str) -> Result<(), JsError> {
        self.client.connect(url)
    }

    pub fn on_message(&mut self, callback: js_sys::Function) -> usize {
        self.client.add_on_message_callback(callback)
    }

    pub fn remove_on_message(&mut self, id: usize) {
        self.client.remove_on_message_callback(id);
    }

    pub fn on_new_peer(&mut self, callback: js_sys::Function) -> usize {
        self.client.add_on_new_peer_callback(callback)
    }

    pub fn remove_on_new_peer(&mut self, id: usize) {
        self.client.remove_on_new_peer_callback(id);
    }

    pub fn send(&self, raw_value: JsValue) -> Result<(), JsError> {
        let value: String = serde_wasm_bindgen::from_value(raw_value)?;
        let peer_request: PeerRequest = PeerRequest::new_forward(
            self.client.peer_id(),
            None,
            PeerEvent::Message {
                peer_id: self.client.peer_id(),
                message: value,
            },
        );
        self.client.send_peer_request(peer_request)
    }
}
