use crate::WebRTCError;
use futures::stream::Stream;
use gloo_events::EventListener;
use js_sys::{ArrayBuffer, Uint8Array};
use metrics::{Counter, counter};
use once_cell::sync::Lazy;
use std::{
    pin::Pin,
    rc::Rc,
    sync::Mutex,
    task::{Context, Poll},
};
use tracing::{debug, error, info, warn};
use wasm_bindgen::JsCast;
use web_sys::{ErrorEvent, Event, MessageEvent, RtcDataChannel, RtcDataChannelState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Text(String),
    Binary(Vec<u8>),
}

macro_rules! make_event_stream {
    ($stream_name:ident, $fn_name:ident, $event_target:ty, $event_type:literal, $event_class:ty, $item_type:ty, $conversion_logic:expr) => {
        #[pin_project::pin_project]
        pub struct $stream_name {
            #[pin]
            receiver: futures::channel::mpsc::UnboundedReceiver<$item_type>,
            _listener: EventListener,
        }

        impl Stream for $stream_name {
            type Item = $item_type;

            fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
                self.project().receiver.poll_next(cx)
            }
        }

        impl $stream_name {
            #[inline]
            pub(crate) fn new(
                receiver: futures::channel::mpsc::UnboundedReceiver<$item_type>,
                listener: EventListener,
            ) -> $stream_name {
                $stream_name {
                    receiver,
                    _listener: listener,
                }
            }
        }

        pub(crate) fn $fn_name(target: &$event_target) -> $stream_name {
            let (sender, receiver) = futures::channel::mpsc::unbounded();
            let sender = Rc::new(sender);
            let conversion_logic = $conversion_logic;

            let listener = EventListener::new(target, $event_type, move |event| {
                let sender = Rc::clone(&sender);
                match event.dyn_ref::<$event_class>() {
                    Some(evt) => {
                        let item = conversion_logic(evt);
                        if let Some(item_to_send) = item {
                            let _ = sender.unbounded_send(item_to_send);
                        }
                    }
                    None => {
                        error!(
                            "{} stream received unexpected event type: {:?}",
                            stringify!($stream_name),
                            event
                        );
                    }
                }
            });

            $stream_name::new(receiver, listener)
        }
    };
}

static MESSAGE_COUNT: Lazy<Mutex<Counter>> =
    Lazy::new(|| Mutex::new(counter!("data_channel.message_count")));
static MESSAGE_BYTES: Lazy<Mutex<Counter>> =
    Lazy::new(|| Mutex::new(counter!("data_channel.message_bytes")));

make_event_stream!(
    MessageStream,
    message_stream,
    RtcDataChannel,
    "message",
    MessageEvent,
    Result<Message, WebRTCError>,
    |event: &MessageEvent| {
        info!("Received message event: {:?}", event);
        let _ = MESSAGE_COUNT.lock()
        .map_err(|e| {
            error!("Failed to acquire lock on MESSAGE_COUNT: {:?}", e);
            e
        })
        .map(|count| {
            count.increment(1);
        });

        let data = event.data();
        if data.is_string() {
            data.as_string()
                .map(|s| {
                    let _ = MESSAGE_BYTES.lock()
                        .map_err(|e| {
                            error!("Failed to acquire lock on MESSAGE_BYTES: {:?}", e);
                            e
                        }).map(|count| {
                            count.increment(s.len() as u64)
                        });
                    Message::Text(s)
                })
                .map(Ok)
        } else if data.has_type::<ArrayBuffer>() {
            let buffer: ArrayBuffer = data.unchecked_into();
            let u8_array = Uint8Array::new(&buffer);
            let mut vec = vec![0; u8_array.length() as usize];
            u8_array.copy_to(&mut vec);
            let _ = MESSAGE_BYTES.lock()
                .map_err(|e| {
                    error!("Failed to acquire lock on MESSAGE_BYTES: {:?}", e);
                    e
                })
                .map(|count| {
                    count.increment(vec.len() as u64)
                });
            Some(Ok(Message::Binary(vec)))
        } else if data.has_type::<web_sys::Blob>() {
            warn!("Received Blob on DataChannel, which is not directly supported by this MessageStream. Use ArrayBuffer instead.");
            Some(Err(WebRTCError::DataChannelInvalidDataType))
        } else {
            warn!("Received unknown data type on DataChannel: {:?}", data);
            Some(Err(WebRTCError::DataChannelInvalidDataType))
        }
    }
);

#[pin_project::pin_project]
pub struct DataChannelStateStream {
    target: Rc<RtcDataChannel>,
    #[pin]
    receiver: futures::channel::mpsc::UnboundedReceiver<RtcDataChannelState>,
    _open_listener: EventListener,
    _close_listener: EventListener,
}

impl Stream for DataChannelStateStream {
    type Item = RtcDataChannelState;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().receiver.poll_next(cx)
    }
}

pub(crate) fn state_stream(target: &Rc<RtcDataChannel>) -> DataChannelStateStream {
    let (sender, receiver) = futures::channel::mpsc::unbounded();
    let target_clone = Rc::clone(target);

    let _ = sender.unbounded_send(target.ready_state());

    let sender_open = Rc::new(sender.clone());
    let target_open = Rc::clone(&target_clone);
    let target_for_state = Rc::clone(&target_clone);
    let open_listener = EventListener::new(&target_open, "open", move |_event| {
        debug!(
            "DataChannel opened with state: {:?}",
            target_for_state.ready_state()
        );
        let _ = sender_open.unbounded_send(RtcDataChannelState::Open);
    });

    let sender_close = Rc::new(sender);
    let close_listener = EventListener::new(&Rc::clone(target), "close", move |_event| {
        debug!("DataChannel closed {:?}", _event.to_string());
        let current_state = target_clone.ready_state();
        let _ = sender_close.unbounded_send(current_state);
        if current_state == RtcDataChannelState::Closed {
            sender_close.close_channel();
        }
    });

    DataChannelStateStream {
        target: Rc::clone(target),
        receiver,
        _open_listener: open_listener,
        _close_listener: close_listener,
    }
}

make_event_stream!(
    ErrorStream,
    error_stream,
    RtcDataChannel,
    "error",
    Event,
    WebRTCError,
    |event: &Event| {
        if let Some(error_event) = event.dyn_ref::<ErrorEvent>() {
            Some(WebRTCError::EventError(format!(
                "DataChannel Error: {} at {}:{}",
                error_event.message(),
                error_event.filename(),
                error_event.lineno()
            )))
        } else {
            Some(WebRTCError::EventError(
                "Unknown DataChannel Error".to_string(),
            ))
        }
    }
);
