use metrics_exporter_prometheus::PrometheusBuilder;
use metrics_util::MetricKindMask;
use serde::{Deserialize, Serialize, Serializer};
use sf_peer_id::PeerID;
use sf_protocol::{PeerEvent, PeerRequest};
use sf_webrtc::{IceCandidate, SessionDescription};
use std::{
    cell::{Ref, RefCell, RefMut},
    rc::Rc,
    time::Duration,
};
use tracing::{debug, error, info, warn};
use wasm_bindgen::prelude::*;

use crate::{
    WsSenderState,
    callback::{JsCallback, JsCallbackManager},
    peer::Peer,
    peer_manager::PeerManager,
    websocket::WebSocketConnection,
};
fn human_readable_peer_id<S>(peer_id: &PeerID, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&peer_id.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "content")]
pub enum ClientEvent {
    NewPeer {
        #[serde(serialize_with = "human_readable_peer_id")]
        peer_id: PeerID,
    },
    Message {
        #[serde(serialize_with = "human_readable_peer_id")]
        peer_id: PeerID,
        message: String,
    },
    DataChannelOpen {
        #[serde(serialize_with = "human_readable_peer_id")]
        peer_id: PeerID,
    },
    DataChannelClose {
        #[serde(serialize_with = "human_readable_peer_id")]
        peer_id: PeerID,
    },
    DataChannelMessage {
        #[serde(serialize_with = "human_readable_peer_id")]
        peer_id: PeerID,
        message: String,
    },
}

type ClientResult<T> = Result<T, JsError>;

pub struct Client {
    peer_id: PeerID,
    event_callbacks: JsCallbackManager,
    peer_manager: RefCell<PeerManager>,
    ws: RefCell<Option<WebSocketConnection>>,
}

impl Client {
    pub fn new() -> ClientResult<Rc<Self>> {
        let peer_id = PeerID::random()
            .map_err(|e| JsError::new(&format!("Failed to generate peer ID: {e}")))?;

        PrometheusBuilder::new()
            .with_push_gateway(
                format!("http://127.0.0.1:9799/metrics/job/{peer_id}"),
                Duration::from_secs(10),
                None,
                None,
                false,
            )
            .map_err(|e| {
                JsError::new(&format!("Failed to configure Prometheus push gateway: {e}"))
            })?
            .idle_timeout(
                MetricKindMask::COUNTER | MetricKindMask::HISTOGRAM,
                Some(Duration::from_secs(10)),
            )
            .install()
            .map_err(|e| JsError::new(&format!("Failed to install Prometheus recorder: {e}")))?;

        Ok(Rc::new(Self {
            peer_id,
            event_callbacks: JsCallbackManager::new(),
            peer_manager: RefCell::new(PeerManager::new()),
            ws: RefCell::new(None),
        }))
    }

    #[inline]
    pub fn peer_id(self: &Rc<Self>) -> PeerID {
        self.peer_id
    }

    #[inline]
    pub fn list_peers(self: &Rc<Self>) -> Vec<String> {
        self.borrow_pm()
            .map(|pm| {
                pm.get_known_peer_ids()
                    .iter()
                    .map(|p| p.to_string())
                    .collect()
            })
            .unwrap_or_default()
    }

    #[inline]
    fn borrow_ws(&self) -> ClientResult<Ref<WebSocketConnection>> {
        self.ws
            .try_borrow()
            .map_err(|e| JsError::new(&format!("Failed to borrow WebSocket: {e}")))
            .and_then(|guard| {
                Ref::filter_map(guard, |opt| opt.as_ref())
                    .map_err(|_| JsError::new("WebSocket is not initialized"))
            })
    }

    #[inline]
    fn borrow_ws_mut(&self) -> ClientResult<RefMut<Option<WebSocketConnection>>> {
        self.ws
            .try_borrow_mut()
            .map_err(|e| JsError::new(&format!("Failed to borrow WebSocket mutably: {e}")))
    }

    #[inline]
    fn borrow_pm(&self) -> ClientResult<Ref<PeerManager>> {
        self.peer_manager
            .try_borrow()
            .map_err(|e| JsError::new(&format!("Failed to borrow PeerManager: {e}")))
    }

    #[inline]
    fn borrow_pm_mut(&self) -> ClientResult<RefMut<PeerManager>> {
        self.peer_manager
            .try_borrow_mut()
            .map_err(|e| JsError::new(&format!("Failed to borrow PeerManager mutably: {e}")))
    }

    pub fn add_event_callback(&self, callback: JsCallback) -> usize {
        self.event_callbacks.add(callback)
    }

    pub fn remove_event_callback(&self, id: usize) {
        self.event_callbacks.remove(id);
    }

    pub fn notify_event(self: &Rc<Self>, event: ClientEvent) {
        let callbacks = self.event_callbacks.borrow_callbacks();
        for callback in callbacks.values() {
            Self::invoke_event_callback(callback, &event);
        }
    }

    fn invoke_event_callback(callback: &JsCallback, event: &ClientEvent) {
        let event_js = match serde_wasm_bindgen::to_value(event) {
            Ok(value) => value,
            Err(e) => {
                error!(error=?e, "Failed to serialize event for callback");
                return;
            }
        };

        let this = JsValue::NULL;
        if let Err(e) = callback.call1(&this, &event_js) {
            error!(error=?e, "Error calling event callback");
        }
    }

    fn new_peer_discovered(self: &Rc<Self>, peer_id: PeerID) {
        info!(%peer_id, "New Peer discovered");
        match self.borrow_pm_mut() {
            Ok(mut pm) => pm.add_known_peer_id(peer_id),
            Err(e) => {
                error!(error=?e, "Failed to borrow PeerManager to add known peer");
                return;
            }
        }

        self.notify_event(ClientEvent::NewPeer { peer_id });
    }

    pub fn connect(self: &Rc<Self>, url: &str) -> ClientResult<()> {
        if self.borrow_ws().is_ok() {
            return Err(JsError::new("WebSocket connection already established"));
        }

        let ws_connection = WebSocketConnection::connect(url, self.peer_id, self.clone())?;
        self.borrow_ws_mut()?.replace(ws_connection);

        info!("WebSocket connection established");
        Ok(())
    }

    pub async fn connect_to_peer(self: &Rc<Self>, peer_id: JsValue) -> Result<(), JsError> {
        let peer_id: PeerID = peer_id.try_into()?;
        info!(%peer_id, "Attempting webRTC connection to peer");

        self.get_or_create_peer(&peer_id).await?.make_offer().await
    }

    pub async fn handle_incoming_text(self: &Rc<Self>, text: &str) {
        match serde_json::from_str::<PeerRequest>(text) {
            Ok(peer_request) => {
                if let Err(e) = self.process_peer_request(peer_request).await {
                    error!(error = ?e, "Failed to process PeerRequest");
                }
            }
            Err(e) => error!(error = ?e, text = ?text, "Failed to deserialize incoming message"),
        };
    }

    async fn process_peer_request(self: &Rc<Self>, request: PeerRequest) -> ClientResult<()> {
        match request {
            PeerRequest::Forward {
                from_peer_id,
                to_peer_id: _,
                data,
            } => self.handle_peer_event(from_peer_id, data).await?,
            _ => warn!(?request, "Received unhandled PeerRequest type"),
        }
        Ok(())
    }

    async fn handle_peer_event(
        self: &Rc<Self>,
        from_peer_id: PeerID,
        event: PeerEvent,
    ) -> ClientResult<()> {
        debug!(%from_peer_id, ?event, "Handling peer event");
        match event {
            PeerEvent::NewPeer { peer_id } => self.new_peer_discovered(peer_id),
            PeerEvent::Message { message, .. } => {
                self.notify_event(ClientEvent::Message {
                    peer_id: from_peer_id,
                    message,
                });
            }
            PeerEvent::WebRtcOffer {
                session_description,
                ..
            } => {
                self.handle_web_rtc_offer(&from_peer_id, &session_description)
                    .await?
            }
            PeerEvent::WebRtcCandidate { candidate, .. } => {
                self.handle_web_rtc_candidate(&from_peer_id, &candidate)
                    .await?
            }
        }
        Ok(())
    }

    async fn handle_web_rtc_candidate(
        &self,
        peer_id: &PeerID,
        candidate: &IceCandidate,
    ) -> Result<(), JsError> {
        info!(%peer_id, %self.peer_id, "Received WebRtcCandidate");
        let peer = self
            .borrow_pm()?
            .get_peer(peer_id)
            .cloned()
            .ok_or_else(|| {
                JsError::new(&format!("Peer not found to handle candidate: {peer_id}"))
            })?;

        peer.handle_candidate(candidate).await
    }

    async fn handle_web_rtc_offer(
        self: &Rc<Self>,
        peer_id: &PeerID,
        session_description: &SessionDescription,
    ) -> Result<(), JsError> {
        info!(%peer_id, %self.peer_id, "Received WebRtcOffer");
        self.get_or_create_peer(peer_id)
            .await?
            .handle_offer(session_description)
            .await
    }

    async fn get_or_create_peer(self: &Rc<Self>, peer_id: &PeerID) -> ClientResult<Peer> {
        if let Some(peer) = self.borrow_pm()?.get_peer(peer_id).cloned() {
            debug!(%peer_id, "Found existing peer");
            return Ok(peer);
        }
        debug!(%peer_id, "Peer not found, creating new one");

        let sender = self.get_ws_sender()?;
        let new_peer = Peer::new(*peer_id, self.peer_id, sender, self.clone()).await?;

        self.borrow_pm_mut()?.add_peer(new_peer.clone());
        self.notify_event(ClientEvent::NewPeer { peer_id: *peer_id });
        info!(%peer_id, "Added new peer to PeerManager");

        Ok(new_peer)
    }

    fn get_ws_sender(&self) -> ClientResult<WsSenderState> {
        self.borrow_ws().map(|ws_guard| ws_guard.sender())
    }

    pub async fn send_message_to_peer(
        &self,
        peer_id: PeerID,
        message: String,
    ) -> Result<(), JsError> {
        let peer = {
            self.borrow_pm()?
                .get_peer(&peer_id)
                .cloned()
                .ok_or_else(|| {
                    JsError::new(&format!("Peer not found for sending message: {peer_id}"))
                })?
        };

        peer.direct_send_str(&message)?;
        info!(from_peer_id=%self.peer_id, to_peer_id=%peer_id, "Sent message");
        Ok(())
    }
}
