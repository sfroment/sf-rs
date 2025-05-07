pub mod futures;

use metrics::{Counter, counter};
use std::rc::Rc;
use wasm_bindgen_futures::JsFuture;
use web_sys::RtcPeerConnection;

use crate::{
    ConnectionStateStream, DataChannelStream, IceCandidate, IceCandidateStream,
    IceConnectionStateStream, IceGatheringStateStream, NegotiationNeededStream,
    SignalingStateStream,
};

use super::{
    data_channel::{DataChannel, DataChannelConfig},
    errors::WebRTCError,
    ice_server::IceServer,
    sdp::SessionDescription,
};

#[derive(Debug, Clone)]
pub struct PeerConnection {
    inner: Rc<RtcPeerConnection>,
    offer_count: Counter,
    answer_count: Counter,
    ice_candidate_count: Counter,
}

impl PartialEq for PeerConnection {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for PeerConnection {}

impl PeerConnection {
    pub fn new(ice_servers: IceServer) -> Result<Self, WebRTCError> {
        let inner = RtcPeerConnection::new_with_configuration(&ice_servers.try_into()?)?;
        let offer_count = counter!("peer_connection.offer_count");
        let answer_count = counter!("peer_connection.answer_count");
        let ice_candidate_count = counter!("peer_connection.ice_candidate_count");
        Ok(Self {
            inner: Rc::new(inner),
            offer_count,
            answer_count,
            ice_candidate_count,
        })
    }

    pub fn new_default() -> Result<Self, WebRTCError> {
        Self::new(IceServer::default())
    }

    pub async fn create_offer(
        &self,
        options: Option<web_sys::RtcOfferOptions>,
    ) -> Result<SessionDescription, WebRTCError> {
        let promise = match options {
            Some(opts) => self.inner.create_offer_with_rtc_offer_options(&opts),
            None => self.inner.create_offer(),
        };
        let offer_js = JsFuture::from(promise).await?;
        self.offer_count.increment(1);
        SessionDescription::try_from(offer_js)
    }

    pub async fn create_answer(
        &self,
        options: Option<web_sys::RtcAnswerOptions>,
    ) -> Result<SessionDescription, WebRTCError> {
        let promise = match options {
            Some(opts) => self.inner.create_answer_with_rtc_answer_options(&opts),
            None => self.inner.create_answer(),
        };
        let answer_js = JsFuture::from(promise).await?;
        self.answer_count.increment(1);
        SessionDescription::try_from(answer_js)
    }

    pub async fn set_remote_description(
        &self,
        description: &SessionDescription,
    ) -> Result<(), WebRTCError> {
        let rtc_session_description_init = description.try_into()?;
        let promise = self
            .inner
            .set_remote_description(&rtc_session_description_init);
        JsFuture::from(promise).await?;
        Ok(())
    }

    pub async fn set_local_description(
        &self,
        description: &SessionDescription,
    ) -> Result<(), WebRTCError> {
        let rtc_session_description_init = description.try_into()?;
        let promise = self
            .inner
            .set_local_description(&rtc_session_description_init);
        JsFuture::from(promise).await?;
        Ok(())
    }

    pub async fn create_data_channel(
        &self,
        label: &str,
        config: Option<DataChannelConfig>,
    ) -> Result<DataChannel, WebRTCError> {
        let config = config.map(DataChannelConfig::try_into).transpose()?;
        let channel = match config {
            Some(conf) => self
                .inner
                .create_data_channel_with_data_channel_dict(label, &conf),
            None => self.inner.create_data_channel(label),
        };

        Ok(channel.into())
    }

    pub fn get_remote_description(&self) -> Result<Option<SessionDescription>, WebRTCError> {
        let description_js = self.inner.remote_description();
        if description_js.is_none() {
            return Ok(None);
        }
        Ok(Some(SessionDescription::try_from(description_js.unwrap())?))
    }

    pub async fn add_ice_candidate(&self, candidate: &IceCandidate) -> Result<(), WebRTCError> {
        let rtc_ice_candidate_init = candidate.try_into()?;
        let promise = self
            .inner
            .add_ice_candidate_with_opt_rtc_ice_candidate(Some(&rtc_ice_candidate_init));
        self.ice_candidate_count.increment(1);
        JsFuture::from(promise).await?;
        Ok(())
    }

    pub fn ice_candidate_stream(&self) -> IceCandidateStream {
        crate::peer_connection::futures::ice_candidate_stream(&self.inner)
    }

    pub fn data_channel_stream(&self) -> DataChannelStream {
        crate::peer_connection::futures::data_channel_stream(&self.inner)
    }

    pub fn negotiation_needed_stream(&self) -> NegotiationNeededStream {
        super::peer_connection::futures::negotiation_needed_stream(&self.inner)
    }

    pub fn connection_state_stream(&self) -> ConnectionStateStream {
        super::peer_connection::futures::connection_state_stream(&self.inner)
    }

    pub fn ice_connection_state_stream(&self) -> IceConnectionStateStream {
        super::peer_connection::futures::ice_connection_state_stream(&self.inner)
    }

    pub fn ice_gathering_state_stream(&self) -> IceGatheringStateStream {
        super::peer_connection::futures::ice_gathering_state_stream(&self.inner)
    }

    pub fn signaling_state_stream(&self) -> SignalingStateStream {
        super::peer_connection::futures::signaling_state_stream(&self.inner)
    }

    pub fn signaling_state(&self) -> web_sys::RtcSignalingState {
        self.inner.signaling_state()
    }

    pub fn ice_gathering_state(&self) -> web_sys::RtcIceGatheringState {
        self.inner.ice_gathering_state()
    }

    pub fn ice_connection_state(&self) -> web_sys::RtcIceConnectionState {
        self.inner.ice_connection_state()
    }

    pub fn connection_state(&self) -> web_sys::RtcPeerConnectionState {
        self.inner.connection_state()
    }

    pub fn close(&self) {
        self.inner.close();
    }

    pub fn is_closed(&self) -> bool {
        matches!(
            self.connection_state(),
            web_sys::RtcPeerConnectionState::Closed | web_sys::RtcPeerConnectionState::Failed
        )
    }

    pub fn raw(&self) -> &RtcPeerConnection {
        &self.inner
    }
}
