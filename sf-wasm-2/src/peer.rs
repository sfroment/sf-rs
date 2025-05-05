use futures::{SinkExt, Stream, StreamExt, channel::mpsc};
use gloo_net::websocket::Message;
use sf_peer_id::PeerID;
use sf_protocol::{PeerEvent, PeerRequest};
use sf_webrtc::{
    DataChannel, DataChannelConfig, IceCandidate, PeerConnection, SdpType, SessionDescription,
};
use std::{cell::RefCell, fmt, hash::Hash, sync::Arc};
use tracing::{error, info};
use wasm_bindgen::JsError;
use wasm_bindgen_futures::spawn_local;

use crate::WsSenderState;

const DEFAULT_CHANNEL_NAME: &str = "sf-channel";

pub struct PeerInner {
    id: PeerID,
    host_peer_id: PeerID,
    dc: DataChannel,
    pc: PeerConnection,
    sender: WsSenderState,
}

#[derive(Clone)]
pub struct Peer(Arc<PeerInner>);

impl Peer {
    pub async fn new(
        id: PeerID,
        host_peer_id: PeerID,
        sender: WsSenderState,
        options: Option<DataChannelConfig>,
    ) -> Result<Self, JsError> {
        let pc = PeerConnection::new_default()?;
        let dc = pc
            .create_data_channel(DEFAULT_CHANNEL_NAME, options)
            .await?;
        let peer = Self(Arc::new(PeerInner {
            id,
            host_peer_id,
            pc,
            dc,
            sender,
        }));
        peer.init_callbacks();
        Ok(peer)
    }

    #[inline]
    pub fn id(&self) -> &PeerID {
        &self.0.id
    }

    /// Make an offer and send it to the host peer
    pub async fn make_offer(&self) -> Result<(), JsError> {
        let session_description = self.0.pc.create_offer(None).await?;
        self.0
            .pc
            .set_local_description(&session_description)
            .await?;

        self.send_peer_request(PeerRequest::Forward {
            from_peer_id: self.0.host_peer_id,
            to_peer_id: Some(self.0.id),
            data: PeerEvent::WebRtcOffer {
                peer_id: self.0.id,
                session_description,
            },
        })
        .await
    }

    // Handle an offer from a peer
    pub async fn handle_offer(&self, offer: &SessionDescription) -> Result<(), JsError> {
        if self.0.pc.get_remote_description()?.is_none() {
            info!(from_peer_id=%self.0.id, to_peer_id=%self.0.host_peer_id, "Setting remote description");
            self.0.pc.set_remote_description(offer).await?;
        }

        if offer.sdp_type == SdpType::Offer {
            info!(from_peer_id=%self.0.id, to_peer_id=%self.0.host_peer_id, "Creating answer");
            let local_description = self.0.pc.create_answer(None).await?;
            self.0.pc.set_local_description(&local_description).await?;

            self.send_peer_request(PeerRequest::Forward {
                from_peer_id: self.0.host_peer_id,
                to_peer_id: Some(self.0.id),
                data: PeerEvent::WebRtcOffer {
                    peer_id: self.0.id,
                    session_description: local_description,
                },
            })
            .await?;
        }

        Ok(())
    }

    pub async fn handle_candidate(&self, candidate: &IceCandidate) -> Result<(), JsError> {
        if self.0.pc.get_remote_description()?.is_none() {
            return Err(JsError::new("Remote description not set"));
        }

        self.0
            .pc
            .add_ice_candidate(candidate)
            .await
            .map_err(|e| JsError::new(&format!("Failed to add ICE candidate: {:?}", e)))
    }

    pub fn direct_send_str(&self, message: &str) -> Result<(), JsError> {
        self.0.dc.send_str(message)?;
        Ok(())
    }

    async fn send_peer_request(&self, peer_request: PeerRequest) -> Result<(), JsError> {
        let mut sender = self
            .0
            .sender
            .borrow()
            .as_ref()
            .cloned()
            .ok_or_else(|| JsError::new("Ws not connected or sender unavailable"))?;

        let text = serde_json::to_string(&peer_request)
            .map_err(|e| JsError::new(&format!("Failed to serialize PeerRequest: {e}")))?;

        let message = Message::Text(text);

        if let Err(e) = sender.send(message).await {
            error!("Failed to queue message for WebSocket: {:?}", e.to_string());
        }

        Ok(())
    }

    fn init_callbacks(&self) {
        self.init_peer_connection_callbacks();
        self.init_data_channel_callbacks();
    }

    fn init_peer_connection_callbacks(&self) {
        let this = self.clone();
        let mut ice_stream = self.0.pc.ice_candidate_stream();
        spawn_local(async move {
            while let Some(Ok(ice_candidate)) = ice_stream.next().await {
                if ice_candidate.is_end_of_candidates() {
                    info!("ICE candidate stream ended");
                    break;
                }
                info!("Ice Candidate gathered: {:?}", ice_candidate);
                if let Err(e) = this
                    .send_peer_request(PeerRequest::Forward {
                        from_peer_id: this.0.host_peer_id,
                        to_peer_id: Some(this.0.id),
                        data: PeerEvent::WebRtcCandidate {
                            peer_id: this.0.id,
                            candidate: ice_candidate,
                        },
                    })
                    .await
                {
                    error!("Failed to send WebRTC candidate: {:?}", e);
                }
            }
        });
    }

    fn init_data_channel_callbacks(&self) {
        let mut data_channel_stream = self.0.dc.state_stream();
        spawn_local(async move {
            while let Some(state) = data_channel_stream.next().await {
                info!("Data channel state: {:?}", state);
            }
        });

        let mut message_stream = self.0.dc.message_stream();
        spawn_local(async move {
            while let Some(message) = message_stream.next().await {
                info!("Data channel message: {:?}", message);
            }
        });

        let mut error_stream = self.0.dc.error_stream();
        spawn_local(async move {
            while let Some(error) = error_stream.next().await {
                error!("Data channel error: {:?}", error);
            }
        });
    }
}

impl PartialEq for Peer {
    fn eq(&self, other: &Self) -> bool {
        self.0.id == other.0.id
    }
}

impl Eq for Peer {}

impl Hash for Peer {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.id.hash(state);
    }
}

impl fmt::Debug for Peer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Peer({})", self.0.id)
    }
}

impl fmt::Display for Peer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Peer({})", self.0.id)
    }
}
