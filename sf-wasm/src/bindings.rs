use sf_peer_id::PeerID;
use std::rc::Rc;
use wasm_bindgen::prelude::*;

use crate::{Client, logging::init_logging};

#[wasm_bindgen(js_name = "Client")]
pub struct ClientWrapper {
    client: Rc<Client>,
}

#[wasm_bindgen(js_class = "Client")]
impl ClientWrapper {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<Self, JsValue> {
        init_logging();
        let client = Client::new()?;
        Ok(Self { client })
    }

    #[wasm_bindgen(getter, js_name = "peerId")]
    pub fn peer_id(&self) -> String {
        self.client.peer_id().to_string()
    }

    #[wasm_bindgen(getter, js_name = "listPeers")]
    pub fn list_peers(&self) -> Vec<String> {
        self.client.list_peers()
    }

    #[wasm_bindgen(js_name = "connect")]
    pub fn connect(&self, url: &str) -> Result<(), JsError> {
        self.client.connect(url)
    }

    #[wasm_bindgen(js_name = "onEvent")]
    pub fn on_event(&mut self, callback: js_sys::Function) -> usize {
        self.client.add_event_callback(callback)
    }

    #[wasm_bindgen(js_name = "removeOnEvent")]
    pub fn remove_on_event(&mut self, id: usize) {
        self.client.remove_event_callback(id);
    }

    #[wasm_bindgen(js_name = "connectToPeer")]
    pub async fn connect_to_peer(&self, peer_id: JsValue) -> Result<(), JsError> {
        self.client.connect_to_peer(peer_id).await
    }

    // pub fn send(&self, raw_value: JsValue) -> Result<(), JsError> {
    //     let value: String = serde_wasm_bindgen::from_value(raw_value)?;
    //     let peer_request: PeerRequest = PeerRequest::new_forward(
    //         self.client.peer_id(),
    //         None,
    //         PeerEvent::Message {
    //             peer_id: self.client.peer_id(),
    //             message: value,
    //         },
    //     );
    //     self.client.send_peer_request(peer_request)
    // }

    #[wasm_bindgen(js_name = "sendMessageToPeer")]
    pub async fn send_message_to_peer(
        &self,
        peer_id: JsValue,
        message: JsValue,
    ) -> Result<(), JsError> {
        let peer_id: PeerID = peer_id.try_into()?;
        let message: String = serde_wasm_bindgen::from_value(message)?;
        self.client.send_message_to_peer(peer_id, message).await
    }
}
