use sf_peer_id::PeerID;
use sf_protocol::{PeerEvent, PeerRequest};
use sf_webrtc::{IceCandidate, SessionDescription};
use std::{cell::RefCell, rc::Rc};
use tracing::{error, info};
use wasm_bindgen::prelude::*;

use crate::{
    WsSenderState,
    callback::{JsCallback, JsCallbackManager},
    logging::init_logging,
    peer::Peer,
    peer_manager::PeerManager,
    websocket::WebSocketConnection,
};

pub struct Client {
    peer_id: PeerID,

    message_callbacks: JsCallbackManager,
    new_peer_callbacks: JsCallbackManager,

    peer_manager: RefCell<PeerManager>,

    ws: RefCell<Option<WebSocketConnection>>,
}

impl Client {
    pub fn new() -> Result<Rc<Self>, JsValue> {
        let peer_id = PeerID::random().map_err(|e| JsValue::from(e.to_string()))?;
        Ok(Rc::new(Self {
            peer_id,
            message_callbacks: JsCallbackManager::new(),
            new_peer_callbacks: JsCallbackManager::new(),
            peer_manager: RefCell::new(PeerManager::new()),
            ws: RefCell::new(None),
        }))
    }

    #[inline]
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

    fn get_ws_sender(&self) -> Result<WsSenderState, JsError> {
        self.ws
            .try_borrow()
            .map_err(|e| JsError::new(&format!("Failed to borrow WebSocket: {e}")))
            .and_then(|ws_guard| {
                ws_guard
                    .as_ref()
                    .ok_or_else(|| JsError::new("WebSocket is not initialized"))
                    .map(|ws| ws.sender())
            })
    }

    pub async fn connect_to_peer(&self, peer_id: JsValue) -> Result<(), JsError> {
        let peer_id: PeerID = peer_id.try_into()?;
        info!(%peer_id, "Attempting webRTC connection to peer");

        {
            let pm = self
                .peer_manager
                .try_borrow()
                .map_err(|e| JsError::new(&format!("Failed to borrow PeerManager: {e}")))?;
            if pm.get_peer(&peer_id).is_some() {
                return Ok(());
            }
        }

        let sender = self.get_ws_sender()?;
        let peer = Peer::new(peer_id, self.peer_id, sender).await?;

        {
            let mut pm = self
                .peer_manager
                .try_borrow_mut()
                .map_err(|e| JsError::new(&format!("Failed to borrow PeerManager: {e}")))?;
            pm.add_peer(peer.clone());
        }

        peer.make_offer().await
    }

    pub fn connect(self: &Rc<Self>, url: &str) -> Result<(), JsError> {
        if self.ws.borrow().is_some() {
            return Err(JsError::new("Already connected"));
        }

        let ws_connection = WebSocketConnection::connect(url, self.peer_id, self.clone())?;

        self.ws
            .try_borrow_mut()
            .map_err(|e| JsError::new(&format!("Failed to borrow WebSocket mut: {e}")))?
            .replace(ws_connection);

        info!("WebSocket connection established");
        Ok(())
    }

    fn new_peer_discovered(&self, peer_id: PeerID) {
        info!(%peer_id, "New Peer discovered");

        let mut peer_manager = self.peer_manager.borrow_mut();
        peer_manager.add_known_peer_id(peer_id);
        drop(peer_manager);

        info!(%peer_id, "Notifying JS about new Peer ID");
        let callbacks = self.new_peer_callbacks.borrow_callbacks();
        for callback in callbacks.values() {
            Self::invoke_new_peer_callback(callback, &peer_id);
        }
    }

    pub async fn handle_incoming_text(self: &Rc<Self>, text: &str) {
        match serde_json::from_str::<PeerRequest>(text) {
            Ok(peer_request) => {
                info!(%peer_request, "Received PeerRequest");
                match peer_request {
                    PeerRequest::Forward {
                        from_peer_id, data, ..
                    } => match data {
                        PeerEvent::NewPeer { peer_id } => self.new_peer_discovered(peer_id),
                        PeerEvent::Message { peer_id, message } => {
                            info!(%peer_id, "Message from Peer");

                            let callbacks = self.message_callbacks.borrow_callbacks();
                            for callback in callbacks.values() {
                                Self::invoke_message_callback(callback, &peer_id, &message);
                            }
                        }
                        PeerEvent::WebRtcOffer {
                            session_description,
                            ..
                        } => self
                            .handle_web_rtc_offer(&from_peer_id, &session_description)
                            .await
                            .unwrap_or_else(|e| {
                                error!(error=?e, "Error handling WebRTC offer");
                            }),
                        PeerEvent::WebRtcCandidate { candidate, .. } => self
                            .handle_web_rtc_candidate(&from_peer_id, &candidate)
                            .await
                            .unwrap_or_else(|e| {
                                error!(error=?e, "Error handling WebRTC candidate");
                            }),
                    },
                    _ => {
                        error!(error=?peer_request, "Received unhandled PeerRequest type");
                    }
                }
            }
            Err(e) => {
                error!(error=?e, "Failed to deserialize incoming message");
                error!(text=?text, "Received text");
            }
        };
    }

    async fn handle_web_rtc_candidate(
        &self,
        peer_id: &PeerID,
        candidate: &IceCandidate,
    ) -> Result<(), JsError> {
        info!(from_peer_id=%peer_id, to_peer_id=%self.peer_id, "Received WebRtcCandidate");

        let peer = {
            let pm = self.peer_manager.borrow();
            pm.get_peer(peer_id)
                .cloned()
                .ok_or_else(|| JsError::new(&format!("Peer not found: {peer_id}")))?
        };

        peer.handle_candidate(candidate).await
    }

    async fn handle_web_rtc_offer(
        &self,
        peer_id: &PeerID,
        session_description: &SessionDescription,
    ) -> Result<(), JsError> {
        info!(from_peer_id=%peer_id, to_peer_id=%self.peer_id, "Received WebRtcOffer");

        let exists = {
            let pm = self
                .peer_manager
                .try_borrow()
                .map_err(|e| JsError::new(&format!("Failed to borrow PeerManager: {e}")))?;
            pm.get_peer(peer_id).cloned()
        };

        let peer = if let Some(existing_peer) = exists {
            existing_peer
        } else {
            let sender = self.get_ws_sender()?;
            let new_peer = Peer::new(*peer_id, self.peer_id, sender).await?;

            {
                let mut pm = self
                    .peer_manager
                    .try_borrow_mut()
                    .map_err(|e| JsError::new(&format!("Failed to borrow PeerManager: {e}")))?;
                pm.add_peer(new_peer.clone());
            }

            new_peer
        };

        peer.handle_offer(session_description).await
    }

    async fn send_message_to_peer(&self, peer_id: PeerID, message: String) -> Result<(), JsError> {
        let peer_manager = self.peer_manager.borrow();
        let peer = peer_manager
            .get_peer(&peer_id)
            .cloned()
            .ok_or(JsError::new(&format!("Peer not found: {peer_id}")))?;
        drop(peer_manager);

        peer.direct_send_str(&message)?;
        info!(from_peer_id=%self.peer_id, to_peer_id=%peer_id, "Sent message");
        Ok(())
    }

    fn invoke_new_peer_callback(callback: &JsCallback, peer_id: &PeerID) {
        match serde_wasm_bindgen::to_value(&peer_id.to_string()) {
            Ok(peer_id_js) => {
                let this = JsValue::NULL;
                if let Err(e) = callback.call1(&this, &peer_id_js) {
                    error!(error=?e, "Error calling on_new_peer callback");
                }
            }
            Err(e) => {
                error!(error=?e, "Failed to serialize peer_id for callback");
            }
        }
    }

    fn invoke_message_callback(callback: &JsCallback, peer_id: &PeerID, message: &str) {
        match serde_wasm_bindgen::to_value(&peer_id.to_string()) {
            Ok(peer_id_js) => match serde_wasm_bindgen::to_value(message) {
                Ok(message_js) => {
                    let this = JsValue::NULL;
                    if let Err(e) = callback.call2(&this, &peer_id_js, &message_js) {
                        error!(error=?e, "Error calling on_message callback");
                    }
                }
                Err(e) => {
                    error!(error=?e, "Failed to serialize message for callback");
                }
            },
            Err(e) => {
                error!(error=?e, "Failed to serialize peer_id for callback");
            }
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
        init_logging();
        let client = Client::new()?;
        Ok(Self { client })
    }

    #[wasm_bindgen(getter)]
    pub fn peer_id(&self) -> String {
        self.client.peer_id().to_string()
    }

    #[wasm_bindgen(getter)]
    pub fn list_peers(&self) -> Vec<String> {
        match self.client.peer_manager.try_borrow() {
            Ok(peer_manager) => {
                let peers_list: Vec<String> = peer_manager
                    .get_known_peer_ids()
                    .iter()
                    .map(|p| p.to_string())
                    .collect();
                info!(peers=?peers_list, "Returning peer list from PeerManager");
                peers_list
            }
            Err(e) => {
                error!(                    error=%e.to_string(),
                    "Wrapper: Failed to borrow PeerManager for list_peers",
                );
                Vec::new() // Return empty list on error
            }
        }
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
