use serde::{Deserialize, Serialize, Serializer, ser::SerializeStruct};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    HtmlDivElement, HtmlTextAreaElement, MessageEvent, RtcConfiguration, RtcDataChannel,
    RtcDataChannelEvent, RtcPeerConnection, RtcPeerConnectionIceEvent, RtcSdpType,
    RtcSessionDescriptionInit, window,
};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);

    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn log_u32(a: u32);

    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn log_many(a: &str, b: &str);
}

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

#[derive(Debug)]
struct SessionDescription {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RtcSdpTypeWrapper(web_sys::RtcSdpType);

impl RtcSdpTypeWrapper {
    fn as_str(&self) -> &'static str {
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

#[derive(Serialize)]
struct IceServerConfig {
    urls: Vec<String>,
    username: String,
    credential: String,
}

pub struct WebRtcClient {
    peer_connection: RtcPeerConnection,
    data_channel: RefCell<Option<RtcDataChannel>>,
}

impl WebRtcClient {
    pub fn new() -> Result<Rc<WebRtcClient>, JsValue> {
        let connection = WebRtcClient::create_peer_connection()?;
        let web_rtc_client = Rc::new(WebRtcClient {
            peer_connection: connection,
            data_channel: RefCell::new(None),
        });

        web_rtc_client.on_ice_candidate_cb()?;
        web_rtc_client.on_data_channel_cb()?;

        Ok(web_rtc_client)
    }

    fn create_peer_connection() -> Result<RtcPeerConnection, JsValue> {
        console_log!("create_peer_connection");
        let config = RtcConfiguration::new();
        let ice_server_config = IceServerConfig {
            urls: vec!["stun:stun.l.google.com:19302".to_string()],
            username: "".to_string(),
            credential: "".to_string(),
        };
        let stun_servers = &serde_wasm_bindgen::to_value(&[ice_server_config])?;
        config.set_ice_servers(stun_servers);
        let connection = RtcPeerConnection::new_with_configuration(&config)?;
        Ok(connection)
    }

    fn on_ice_candidate_cb(&self) -> Result<(), JsValue> {
        console_log!("set_on_ice_candidate_cb");
        let document = window()
            .ok_or("Failed to get window")?
            .document()
            .ok_or("Failed to get document")?;
        let text_area = document
            .get_element_by_id("local-sdp")
            .ok_or("failed to get local-sdp")?
            .dyn_into::<HtmlTextAreaElement>()?;

        let peer_connection = self.peer_connection.clone();
        let on_ice_candidate_gather: Box<dyn FnMut(_)> =
            Box::new(move |event: RtcPeerConnectionIceEvent| {
                console_log!("ICE candidate gathered");
                if let Some(candidate) = event.candidate() {
                    console_log!("ICE candidate gathered: {:?}", candidate);
                    let local_desc = peer_connection.local_description().unwrap();
                    let session_description = SessionDescription {
                        sdp: local_desc.sdp().to_string(),
                        r#type: RtcSdpTypeWrapper(RtcSdpType::Offer),
                    };
                    console_log!("session_description: {:?}", session_description);

                    let session_description_value = serde_json::to_string(&session_description)
                        .unwrap_or_else(|_| {
                            console_log!("Failed to serialize session description");
                            "".to_string()
                        });

                    text_area.set_value(&session_description_value);
                } else {
                    console_log!("All ICE candidates gathered, getting local description");
                    let local_desc = peer_connection.local_description().unwrap();
                    let session_description = SessionDescription {
                        sdp: local_desc.sdp().to_string(),
                        r#type: RtcSdpTypeWrapper(RtcSdpType::Offer),
                    };
                    console_log!("session_description: {:?}", session_description);

                    let session_description_value = serde_json::to_string(&session_description)
                        .unwrap_or_else(|_| {
                            console_log!("Failed to serialize session description");
                            "".to_string()
                        });

                    text_area.set_value(&session_description_value);
                }
            });

        let on_ice_candidate_gather = Closure::wrap(on_ice_candidate_gather);
        self.peer_connection
            .set_onicecandidate(Some(on_ice_candidate_gather.as_ref().unchecked_ref()));
        on_ice_candidate_gather.forget();

        Ok(())
    }

    fn on_data_channel_cb(self: &Rc<Self>) -> Result<(), JsValue> {
        let this = Rc::clone(self);
        console_log!("set_on_data_channel_cb");
        let on_data_channel: Box<dyn FnMut(_)> = Box::new(move |event: RtcDataChannelEvent| {
            let dc = event.channel();
            console_log!("new data channel receiveid {:?}", dc.label());

            // set the data channel in the client
            match this.data_channel.try_borrow_mut() {
                Ok(mut data_channel_opt) => match this.setup_data_channel(&dc) {
                    Ok(_) => *data_channel_opt = Some(dc),
                    Err(e) => {
                        console_log!("Failed to setup data channel: {:?}", e);
                    }
                },
                Err(e) => {
                    console_log!("Failed to borrow data channel: {:?}", e);
                }
            }
        });

        let on_data_channel = Closure::wrap(on_data_channel);
        self.peer_connection
            .set_ondatachannel(Some(on_data_channel.as_ref().unchecked_ref()));
        on_data_channel.forget();

        Ok(())
    }

    fn setup_data_channel(self: &Rc<Self>, dc: &RtcDataChannel) -> Result<(), JsValue> {
        let document = window()
            .ok_or("Failed to get window")
            .unwrap()
            .document()
            .ok_or("Failed to get document")
            .unwrap();

        let text_area = document
            .get_element_by_id("messages")
            .ok_or("failed to get messages")
            .unwrap()
            .dyn_into::<HtmlDivElement>()
            .unwrap();

        let onopen_callback = Closure::wrap(Box::new(move || {
            console_log!("DataChannel open");
        }) as Box<dyn FnMut()>);
        dc.set_onopen(Some(onopen_callback.as_ref().unchecked_ref()));
        onopen_callback.forget();

        let onmessage_callback = Closure::wrap(Box::new(move |event: MessageEvent| {
            let message = event.data().as_string().unwrap();
            text_area.set_inner_html(&format!("{}{}<br>", text_area.inner_html(), message));
        }) as Box<dyn FnMut(_)>);
        dc.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
        onmessage_callback.forget();

        Ok(())
    }

    fn create_data_channel(self: &Rc<Self>) -> Result<(), JsValue> {
        let dc = self.peer_connection.create_data_channel("chat");
        self.setup_data_channel(&dc)?;
        self.data_channel.borrow_mut().replace(dc);

        Ok(())
    }

    pub async fn create_offer(self: &Rc<Self>) -> Result<(), JsValue> {
        // 1st create the data channel
        self.create_data_channel()?;

        let offer_promise = self.peer_connection.create_offer();
        let offer_js = JsFuture::from(offer_promise).await?;
        let offer: &RtcSessionDescriptionInit = offer_js.as_ref().unchecked_ref();

        let set_local_promise = self.peer_connection.set_local_description(offer);
        JsFuture::from(set_local_promise).await?;

        Ok(())
    }

    pub async fn set_remote_description(self: &Rc<Self>, sdp_json: JsValue) -> Result<(), JsValue> {
        let sdp: SessionDescription = serde_wasm_bindgen::from_value(sdp_json)?;
        let remote_description = RtcSessionDescriptionInit::new(sdp.r#type.into());
        remote_description.set_sdp(&sdp.sdp);
        let set_remote_promise = self
            .peer_connection
            .set_remote_description(&remote_description);
        JsFuture::from(set_remote_promise).await?;

        if sdp.r#type == RtcSdpTypeWrapper(web_sys::RtcSdpType::Offer) {
            let answer_promise = self.peer_connection.create_answer();
            let answer_js = JsFuture::from(answer_promise).await?;
            let answer: &RtcSessionDescriptionInit = answer_js.as_ref().unchecked_ref();
            let set_local_promise = self.peer_connection.set_local_description(answer);
            JsFuture::from(set_local_promise).await?;
        }

        Ok(())
    }

    pub async fn send_message(&self, msg: String) -> Result<(), JsValue> {
        if let Ok(dc) = self.data_channel.try_borrow() {
            dc.as_ref()
                .ok_or("Data channel not found")?
                .send_with_str(&msg)?;
        }

        Ok(())
    }
}

#[wasm_bindgen]
pub struct WebRtcClientWrapper {
    web_rtc_client: Rc<WebRtcClient>,
}

#[wasm_bindgen]
impl WebRtcClientWrapper {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<WebRtcClientWrapper, JsValue> {
        let web_rtc_client = WebRtcClient::new()?;
        Ok(Self { web_rtc_client })
    }

    pub async fn create_offer(&self) -> Result<(), JsValue> {
        self.web_rtc_client.create_offer().await
    }

    pub async fn set_remote_description(&self, sdp_json: JsValue) -> Result<(), JsValue> {
        self.web_rtc_client.set_remote_description(sdp_json).await
    }

    pub async fn send_message(&self, msg: String) -> Result<(), JsValue> {
        self.web_rtc_client.send_message(msg).await
    }
}
