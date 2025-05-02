use serde::{Deserialize, Serialize};

use super::errors::WebRTCError;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct IceCandidate {
    pub candidate: String,

    pub sdp_m_line_index: Option<u16>,

    pub sdp_mid: Option<String>,
}

impl TryFrom<web_sys::RtcIceCandidate> for IceCandidate {
    type Error = WebRTCError;

    fn try_from(candidate: web_sys::RtcIceCandidate) -> Result<Self, Self::Error> {
        let sdp_mid = candidate.sdp_mid();
        let sdp_m_line_index = candidate.sdp_m_line_index();
        let candidate = candidate.candidate();

        Ok(IceCandidate {
            candidate,
            sdp_m_line_index,
            sdp_mid,
        })
    }
}

impl TryFrom<IceCandidate> for web_sys::RtcIceCandidate {
    type Error = WebRTCError;

    fn try_from(candidate: IceCandidate) -> Result<Self, Self::Error> {
        let rtc_ice_candidate_init = web_sys::RtcIceCandidateInit::new(&candidate.candidate);
        rtc_ice_candidate_init.set_sdp_mid(candidate.sdp_mid.as_deref());
        rtc_ice_candidate_init.set_sdp_m_line_index(candidate.sdp_m_line_index);

        let rtc_ice_candidate = web_sys::RtcIceCandidate::new(&rtc_ice_candidate_init)?;

        Ok(rtc_ice_candidate)
    }
}
