#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RtcSdpTypeWrapper(pub web_sys::RtcSdpType);

impl RtcSdpTypeWrapper {
    pub fn as_str(&self) -> &'static str {
        match self.0 {
            web_sys::RtcSdpType::Offer => "offer",
            web_sys::RtcSdpType::Pranswer => "pranswer",
            web_sys::RtcSdpType::Answer => "answer",
            web_sys::RtcSdpType::Rollback => "rollback",
            _ => "unknown",
        }
    }
}

impl From<RtcSdpTypeWrapper> for web_sys::RtcSdpType {
    fn from(wrapper: RtcSdpTypeWrapper) -> Self {
        wrapper.0
    }
}
