use serde::{Deserialize, Serialize};

use super::{errors::WebRTCError, sdp_type::SdpType};

#[doc = "Reference: https://developer.mozilla.org/en-US/docs/Web/API/RTCSessionDescription"]
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct SessionDescription {
    pub sdp: String,
    pub sdp_type: SdpType,
}

impl TryFrom<SessionDescription> for web_sys::RtcSessionDescription {
    type Error = WebRTCError;

    fn try_from(session_description: SessionDescription) -> Result<Self, Self::Error> {
        let sdp_type = session_description.sdp_type.into();

        let rtc_session_description = web_sys::RtcSessionDescription::new()?;
        rtc_session_description.set_sdp(&session_description.sdp);
        rtc_session_description.set_type(sdp_type);

        Ok(rtc_session_description)
    }
}

impl TryFrom<web_sys::RtcSessionDescription> for SessionDescription {
    type Error = WebRTCError;

    fn try_from(
        rtc_session_description: web_sys::RtcSessionDescription,
    ) -> Result<Self, Self::Error> {
        Ok(SessionDescription {
            sdp: rtc_session_description.sdp(),
            sdp_type: rtc_session_description.type_().into(),
        })
    }
}
