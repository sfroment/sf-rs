use wasm_bindgen::JsValue;

#[derive(Debug, thiserror::Error)]
pub enum WebRTCError {
    #[error("JavaScript error: {0:?}")]
    JsError(JsValue),

    #[error("Serialization/Deserialization error: {0:?}")]
    SerdeError(serde_wasm_bindgen::Error),

    #[error("End of candidates")]
    EndOfCandidates,

    #[error("DataChannel send error: {0:?}")]
    DataChannelSend(JsValue),

    #[error("DataChannel is not open (state: {0:?})")]
    DataChannelNotOpen(Option<web_sys::RtcDataChannelState>),

    #[error("DataChannel received unexpected data type")]
    DataChannelInvalidDataType,

    #[error("Event error: {0}")]
    EventError(String),
}

impl From<JsValue> for WebRTCError {
    fn from(error: JsValue) -> Self {
        WebRTCError::JsError(error)
    }
}

impl From<serde_wasm_bindgen::Error> for WebRTCError {
    fn from(error: serde_wasm_bindgen::Error) -> Self {
        WebRTCError::SerdeError(error)
    }
}
