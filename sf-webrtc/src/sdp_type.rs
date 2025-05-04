use serde::{Deserialize, Serialize};

#[doc = "Reference: https://developer.mozilla.org/en-US/docs/Web/API/RTCSessionDescription/type"]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SdpType {
    Offer,
    Answer,
    Pranswer,
    Rollback,
}

impl From<web_sys::RtcSdpType> for SdpType {
    fn from(sdp_type: web_sys::RtcSdpType) -> Self {
        match sdp_type {
            web_sys::RtcSdpType::Offer => SdpType::Offer,
            web_sys::RtcSdpType::Answer => SdpType::Answer,
            web_sys::RtcSdpType::Pranswer => SdpType::Pranswer,
            web_sys::RtcSdpType::Rollback => SdpType::Rollback,
            _ => unreachable!(),
        }
    }
}

impl From<SdpType> for web_sys::RtcSdpType {
    fn from(value: SdpType) -> Self {
        match value {
            SdpType::Offer => web_sys::RtcSdpType::Offer,
            SdpType::Answer => web_sys::RtcSdpType::Answer,
            SdpType::Pranswer => web_sys::RtcSdpType::Pranswer,
            SdpType::Rollback => web_sys::RtcSdpType::Rollback,
        }
    }
}

impl From<SdpType> for String {
    fn from(value: SdpType) -> Self {
        match value {
            SdpType::Offer => "offer".to_string(),
            SdpType::Answer => "answer".to_string(),
            SdpType::Pranswer => "pranswer".to_string(),
            SdpType::Rollback => "rollback".to_string(),
        }
    }
}

impl From<String> for SdpType {
    fn from(value: String) -> Self {
        match value.as_str() {
            "offer" => SdpType::Offer,
            "answer" => SdpType::Answer,
            "pranswer" => SdpType::Pranswer,
            "rollback" => SdpType::Rollback,
            _ => unreachable!(),
        }
    }
}
