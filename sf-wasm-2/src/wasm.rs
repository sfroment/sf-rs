use futures::{SinkExt, Stream, StreamExt, channel::mpsc, stream::SplitStream};
use gloo_console::log;
use gloo_net::websocket::{Message, WebSocketError, futures::WebSocket};
use serde_json::value::RawValue;
use sf_peer_id::PeerID;
use sf_protocol::{PeerEvent, PeerRequest};
use std::{cell::RefCell, rc::Rc, sync::Arc};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

pub struct Client {
    peer_id: PeerID,
    on_message_callbacks: RefCell<Vec<js_sys::Function>>,
    sender: RefCell<Option<mpsc::Sender<Message>>>,
}

impl Client {
    pub fn new() -> Result<Rc<Self>, JsValue> {
        let peer_id = PeerID::random().expect("Failed to generate peer ID");
        Ok(Rc::new(Self {
            peer_id,
            on_message_callbacks: RefCell::new(Vec::new()),
            sender: RefCell::new(None),
        }))
    }

    pub fn peer_id(self: &Rc<Self>) -> PeerID {
        self.peer_id
    }

    pub fn on_message(self: &mut Rc<Self>, callback: js_sys::Function) -> usize {
        let mut callbacks = self.on_message_callbacks.borrow_mut();
        let id = callbacks.len();
        callbacks.push(callback);
        id
    }

    pub fn remove_on_message(self: &mut Rc<Self>, id: usize) {
        let mut callbacks = self.on_message_callbacks.borrow_mut();
        callbacks.remove(id);
    }

    pub fn connect(self: &Rc<Self>, url: &str) -> Result<(), JsError> {
        let ws = WebSocket::open(format!("ws://{url}/ws?peer_id={}", self.peer_id).as_str())?;
        let (write, read) = ws.split();
        let (sender, mut receiver) = mpsc::channel(32);
        *self.sender.borrow_mut() = Some(sender);

        // Spawn a task to forward messages from the channel to the WebSocket
        let mut write = write;
        spawn_local(async move {
            while let Some(msg) = receiver.next().await {
                if let Err(e) = write.send(msg).await {
                    log!("Failed to send message: {:?}", e.to_string());
                    break;
                }
            }
        });

        self.read_ws(read);
        Ok(())
    }

    fn read_ws<R>(self: &Rc<Self>, mut read: R)
    where
        R: Stream<Item = Result<Message, WebSocketError>> + Unpin + 'static,
    {
        let client = self.clone();
        spawn_local(async move {
            while let Some(msg) = read.next().await {
                match msg {
                    Ok(msg) => {
                        if let Message::Text(text) = msg {
                            let peer_request: PeerRequest = serde_json::from_str(&text).unwrap();
                            log!(
                                "client_impl: onmessage_callback: {:?}",
                                peer_request.clone()
                            );

                            let peer_request_js =
                                serde_wasm_bindgen::to_value(&peer_request).unwrap();
                            for callback in client.on_message_callbacks.borrow().iter() {
                                let this = JsValue::NULL;
                                let _ = callback.call1(&this, &peer_request_js);
                            }
                        }
                    }
                    Err(e) => {
                        log!("client_impl: onmessage_callback: {:?}", e.to_string());
                    }
                }
            }
        });
    }

    fn send_peer_request(&self, peer_request: PeerRequest) -> Result<(), JsError> {
        let mut sender = self.sender.borrow_mut();
        if let Some(sender) = sender.as_mut() {
            let text = serde_json::to_string(&peer_request)
                .map_err(|e| JsError::new(&format!("Failed to serialize peer request: {}", e)))?;
            let message = Message::Text(text);
            let mut sender = sender.clone();
            spawn_local(async move {
                if let Err(e) = sender.send(message).await {
                    log!("Failed to send message: {:?}", e.to_string());
                }
            });
            Ok(())
        } else {
            Err(JsError::new("WebSocket not connected"))
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

    pub fn connect(&self, url: &str) -> Result<(), JsError> {
        self.client.connect(url)
    }

    pub fn on_message(&mut self, callback: js_sys::Function) -> usize {
        self.client.on_message(callback)
    }

    pub fn remove_on_message(&mut self, id: usize) {
        self.client.remove_on_message(id);
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

    //fn list_peers(&self) -> Vec<PeerID> {
    //    unimplemented!();
    //}

    //fn send_to_peer(&self, peer_id: PeerID, message: &[u8]) {
    //    unimplemented!();
    //}

    //fn send_to_room(&self, room_id: &str, message: &[u8]) {
    //    unimplemented!();
    //}

    //fn register_event_callback(&self, event_name: &str, callback: Function) {
    //    unimplemented!();
    //}
}
