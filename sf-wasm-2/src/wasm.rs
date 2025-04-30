use std::{cell::RefCell, rc::Rc};

use futures::{SinkExt, StreamExt};
use gloo_net::websocket::{Message, futures::WebSocket};
use sf_peer_id::PeerID;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::console_log;

// what do I need ?
//trait Client {
//    fn list_peers(&self) -> Vec<PeerID>;
//    fn send_to_peer(&self, peer_id: PeerID, message: &[u8]);
//    fn send_to_room(&self, room_id: &str, message: &[u8]);

//    fn register_event_callback(&self, event_name: &str, callback: Function);
//}

#[wasm_bindgen]
pub struct ClientImpl {
    peer_id: PeerID,
    ws: Option<WebSocket>,
}

#[wasm_bindgen]
impl ClientImpl {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<Self, JsValue> {
        let peer_id = PeerID::random().expect("Failed to generate peer ID");
        console_log!("client_impl: peer ID: {}", peer_id);
        Ok(Self { peer_id, ws: None })
    }

    #[wasm_bindgen(getter)]
    pub fn peer_id(&self) -> String {
        self.peer_id.to_string()
    }

    pub fn connect(&self, url: &str) -> Result<(), JsError> {
        let ws = WebSocket::open(url)?;
        let (_, mut read) = ws.split();
        spawn_local(async move {
            while let Some(msg) = read.next().await {
                console_log!("client_impl: onmessage_callback: {:?}", msg);
            }
        });
        Ok(())
    }

    //fn onmessage_callback(&self, event: MessageEvent) {
    //    console_log!("client_impl: onmessage_callback: {:?}", event);
    //}

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
