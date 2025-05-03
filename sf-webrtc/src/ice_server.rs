use lazy_static::lazy_static;
use serde::Serialize;
use wasm_bindgen::JsValue;

use super::errors::WebRTCError;

lazy_static! {
    pub static ref STUN_SERVERS: Vec<String> = vec![
        "stun:stun.l.google.com:19302".to_string(),
        "stun:stun.l.google.com:5349".to_string(),
        "stun:stun1.l.google.com:3478".to_string(),
        "stun:stun1.l.google.com:5349".to_string(),
        "stun:stun2.l.google.com:19302".to_string(),
        "stun:stun2.l.google.com:5349".to_string(),
        "stun:stun3.l.google.com:3478".to_string(),
        "stun:stun3.l.google.com:5349".to_string(),
        "stun:stun4.l.google.com:19302".to_string(),
        "stun:stun4.l.google.com:5349".to_string(),
    ];
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct IceServer {
    urls: Vec<String>,
    username: Option<String>,
    credential: Option<String>,
}

impl IceServer {
    pub fn new(urls: Vec<String>, username: Option<String>, credential: Option<String>) -> Self {
        Self {
            urls,
            username,
            credential,
        }
    }
}

impl Default for IceServer {
    fn default() -> Self {
        Self::new(STUN_SERVERS.clone(), Default::default(), Default::default())
    }
}

impl TryFrom<web_sys::RtcIceServer> for IceServer {
    type Error = WebRTCError;

    fn try_from(ice_server: web_sys::RtcIceServer) -> Result<Self, Self::Error> {
        let urls = ice_server.get_urls();
        let urls = serde_wasm_bindgen::from_value(urls)?;

        let username = ice_server.get_username();
        let credential = ice_server.get_credential();

        Ok(Self::new(urls, username, credential))
    }
}

impl TryFrom<IceServer> for web_sys::RtcIceServer {
    type Error = WebRTCError;

    fn try_from(ice_server: IceServer) -> Result<Self, Self::Error> {
        let web_ice_server = web_sys::RtcIceServer::new();

        let urls = serde_wasm_bindgen::to_value(&ice_server.urls)?;
        web_ice_server.set_urls(&urls);
        if let Some(username) = ice_server.username {
            web_ice_server.set_username(&username);
        }
        if let Some(credential) = ice_server.credential {
            web_ice_server.set_credential(&credential);
        }

        Ok(web_ice_server)
    }
}

impl TryFrom<IceServer> for web_sys::RtcConfiguration {
    type Error = WebRTCError;

    fn try_from(ice_server: IceServer) -> Result<Self, Self::Error> {
        let rtc_ice_server: web_sys::RtcIceServer = ice_server.try_into()?;
        let array = js_sys::Array::new();
        array.push(&rtc_ice_server);

        let config = web_sys::RtcConfiguration::new();
        config.set_ice_servers(&JsValue::from(array));

        Ok(config)
    }
}
