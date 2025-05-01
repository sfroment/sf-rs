use serde::{Deserialize, Serialize, Serializer, ser::SerializeStruct};

use crate::rtc_sdp_wrapper::RtcSdpTypeWrapper;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionDescription {
    pub sdp: String,
    pub r#type: RtcSdpTypeWrapper,
}

impl Serialize for SessionDescription {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("SessionDescription", 2)?;
        state.serialize_field("sdp", &self.sdp)?;
        state.serialize_field("type", &self.r#type.as_str())?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for SessionDescription {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct SessionDescriptionHelper {
            sdp: String,
            #[serde(rename = "type")]
            r#type: String,
        }

        let helper = SessionDescriptionHelper::deserialize(deserializer)?;
        let r#type = match helper.r#type.as_str() {
            "offer" => RtcSdpTypeWrapper(web_sys::RtcSdpType::Offer),
            "pranswer" => RtcSdpTypeWrapper(web_sys::RtcSdpType::Pranswer),
            "answer" => RtcSdpTypeWrapper(web_sys::RtcSdpType::Answer),
            "rollback" => RtcSdpTypeWrapper(web_sys::RtcSdpType::Rollback),
            _ => return Err(serde::de::Error::custom("Unknown RtcSdpType")),
        };
        Ok(SessionDescription {
            sdp: helper.sdp,
            r#type,
        })
    }
}
