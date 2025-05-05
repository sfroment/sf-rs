use serde::{Deserialize, Serialize};

use super::errors::WebRTCError;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct IceCandidate {
    pub candidate: Option<String>,

    pub sdp_m_line_index: Option<u16>,

    pub sdp_mid: Option<String>,
}

impl IceCandidate {
    pub fn end_of_candidates() -> Self {
        Self {
            candidate: None,
            sdp_m_line_index: None,
            sdp_mid: None,
        }
    }

    pub fn is_end_of_candidates(&self) -> bool {
        self.candidate.is_none()
    }
}

#[inline]
fn ice_candidate_to_web_sys(
    candidate: &IceCandidate,
) -> Result<web_sys::RtcIceCandidate, WebRTCError> {
    if candidate.is_end_of_candidates() {
        return Err(WebRTCError::EndOfCandidates);
    }

    let rtc_ice_candidate_init =
        web_sys::RtcIceCandidateInit::new(candidate.candidate.as_ref().unwrap());
    rtc_ice_candidate_init.set_sdp_mid(candidate.sdp_mid.as_deref());
    rtc_ice_candidate_init.set_sdp_m_line_index(candidate.sdp_m_line_index);

    let rtc_ice_candidate = web_sys::RtcIceCandidate::new(&rtc_ice_candidate_init)?;

    Ok(rtc_ice_candidate)
}

impl TryFrom<web_sys::RtcIceCandidate> for IceCandidate {
    type Error = WebRTCError;

    fn try_from(candidate: web_sys::RtcIceCandidate) -> Result<Self, Self::Error> {
        let sdp_mid = candidate.sdp_mid();
        let sdp_m_line_index = candidate.sdp_m_line_index();
        let candidate = candidate.candidate();

        Ok(IceCandidate {
            candidate: Some(candidate),
            sdp_m_line_index,
            sdp_mid,
        })
    }
}

impl TryFrom<IceCandidate> for web_sys::RtcIceCandidate {
    type Error = WebRTCError;

    fn try_from(candidate: IceCandidate) -> Result<Self, Self::Error> {
        ice_candidate_to_web_sys(&candidate)
    }
}

impl TryFrom<&IceCandidate> for web_sys::RtcIceCandidate {
    type Error = WebRTCError;

    fn try_from(candidate: &IceCandidate) -> Result<Self, Self::Error> {
        ice_candidate_to_web_sys(candidate)
    }
}
