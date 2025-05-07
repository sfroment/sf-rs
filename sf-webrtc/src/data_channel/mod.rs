pub mod futures;

use js_sys::ArrayBuffer;
use metrics::{Counter, counter};
use std::rc::Rc;
use tracing::info;
use web_sys::{RtcDataChannel, RtcDataChannelState};

use crate::{DataChannelStateStream, ErrorStream, MessageStream, WebRTCError};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataChannelConfig {
    pub ordered: Option<bool>,
    pub max_packet_life_time: Option<u16>,
    pub max_retransmits: Option<u16>,
    pub protocol: Option<String>,
    pub negotiated: Option<bool>,
    pub id: Option<u16>,
}

impl TryFrom<DataChannelConfig> for web_sys::RtcDataChannelInit {
    type Error = WebRTCError;

    fn try_from(config: DataChannelConfig) -> Result<Self, Self::Error> {
        let init = web_sys::RtcDataChannelInit::new();
        if let Some(ordered) = config.ordered {
            init.set_ordered(ordered);
        }

        if let Some(max_packet_life_time) = config.max_packet_life_time {
            init.set_max_packet_life_time(max_packet_life_time);
        }

        if let Some(max_retransmits) = config.max_retransmits {
            init.set_max_retransmits(max_retransmits);
        }

        if let Some(protocol) = config.protocol {
            init.set_protocol(&protocol);
        }

        if let Some(negotiated) = config.negotiated {
            init.set_negotiated(negotiated);
        }

        if let Some(id) = config.id {
            init.set_id(id);
        }

        Ok(init)
    }
}

#[derive(Clone, Debug)]
pub struct DataChannel {
    inner: Rc<RtcDataChannel>,
    message_count: Counter,
    message_bytes: Counter,
}

impl DataChannel {
    pub fn new(inner: RtcDataChannel) -> Self {
        let message_count = counter!("data_channel.message_count");
        let message_bytes = counter!("data_channel.message_bytes");
        Self {
            inner: Rc::new(inner),
            message_count,
            message_bytes,
        }
    }

    pub fn label(&self) -> String {
        self.inner.label()
    }

    pub fn ready_state(&self) -> RtcDataChannelState {
        self.inner.ready_state()
    }

    pub fn buffered_amount(&self) -> u32 {
        self.inner.buffered_amount()
    }

    pub fn send(&self, data: &[u8]) -> Result<(), WebRTCError> {
        if self.ready_state() != RtcDataChannelState::Open {
            return Err(WebRTCError::DataChannelNotOpen(Some(self.ready_state())));
        }

        self.inner
            .send_with_u8_array(data)
            .map_err(WebRTCError::from)
            .map(|_| self.message_count.increment(1))
            .map(|_| self.message_bytes.increment(data.len() as u64))
    }

    pub fn send_str(&self, data: &str) -> Result<(), WebRTCError> {
        if self.ready_state() != RtcDataChannelState::Open {
            return Err(WebRTCError::DataChannelNotOpen(Some(self.ready_state())));
        }
        info!("Sending string: {}", data);
        self.inner
            .send_with_str(data)
            .map_err(WebRTCError::from)
            .map(|_| self.message_count.increment(1))
            .map(|_| self.message_bytes.increment(data.len() as u64))
    }

    pub fn send_array_buffer(&self, data: &ArrayBuffer) -> Result<(), WebRTCError> {
        if self.ready_state() != RtcDataChannelState::Open {
            return Err(WebRTCError::DataChannelNotOpen(Some(self.ready_state())));
        }

        self.inner
            .send_with_array_buffer(data)
            .map_err(WebRTCError::from)
            .map(|_| self.message_count.increment(1))
            .map(|_| self.message_bytes.increment(data.byte_length() as u64))
    }

    pub fn message_stream(&self) -> MessageStream {
        crate::data_channel::futures::message_stream(&self.inner)
    }

    pub fn state_stream(&self) -> DataChannelStateStream {
        crate::data_channel::futures::state_stream(&self.inner)
    }

    pub fn error_stream(&self) -> ErrorStream {
        crate::data_channel::futures::error_stream(&self.inner)
    }

    pub fn close(&self) {
        self.inner.close();
    }

    pub fn is_closed(&self) -> bool {
        matches!(
            self.ready_state(),
            RtcDataChannelState::Closing | RtcDataChannelState::Closed
        )
    }

    pub fn raw(&self) -> &RtcDataChannel {
        &self.inner
    }
}

impl From<RtcDataChannel> for DataChannel {
    fn from(inner: RtcDataChannel) -> Self {
        Self::new(inner)
    }
}
