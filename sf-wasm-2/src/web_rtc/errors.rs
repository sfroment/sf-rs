use wasm_bindgen::JsValue;

#[derive(Debug, thiserror::Error)]
pub enum WebRTCError {
    #[error("JavaScript error: {0:?}")]
    JsError(JsValue),

    #[error("Serialization/Deserialization error: {0:?}")]
    SerdeError(serde_wasm_bindgen::Error),
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
