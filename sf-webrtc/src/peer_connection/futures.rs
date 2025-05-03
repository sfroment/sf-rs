use crate::{IceCandidate, WebRTCError, data_channel::DataChannel};
use futures::stream::Stream;
use gloo_console::error;
use gloo_events::EventListener;
use std::{
    pin::Pin,
    rc::Rc,
    task::{Context, Poll},
};
use wasm_bindgen::JsCast;
use web_sys::{
    Event, RtcDataChannelEvent, RtcIceConnectionState, RtcIceGatheringState, RtcPeerConnection,
    RtcPeerConnectionIceEvent, RtcPeerConnectionState, RtcSignalingState,
};

macro_rules! make_event_stream {
    ($stream_name:ident, $fn_name:ident, $event_target:ty, $event_type:literal, $event_class:ty, $item_type:ty, $conversion_logic:expr) => {
        #[pin_project::pin_project]
        pub struct $stream_name {
            #[pin]
            receiver: futures::channel::mpsc::UnboundedReceiver<$item_type>,
            _listener: EventListener,
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

        impl Stream for $stream_name {
            type Item = $item_type;

            fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
                self.project().receiver.poll_next(cx)
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

make_event_stream!(
    IceCandidateStream,
    ice_candidate_stream,
    RtcPeerConnection,
    "icecandidate",
    RtcPeerConnectionIceEvent,
    Result<IceCandidate, WebRTCError>,
    |event: &RtcPeerConnectionIceEvent| {
        if let Some(candidate) = event.candidate() {
            return Some(candidate.try_into());
        }
        Some(Ok(IceCandidate::end_of_candidates()))
    }
);

make_event_stream!(
    DataChannelStream,
    data_channel_stream,
    RtcPeerConnection,
    "datachannel",
    RtcDataChannelEvent,
    DataChannel,
    |event: &RtcDataChannelEvent| { Some(event.channel().into()) }
);

make_event_stream!(
    NegotiationNeededStream,
    negotiation_needed_stream,
    RtcPeerConnection,
    "negotiationneeded",
    Event,
    (),
    |_event: &Event| { Some(()) }
);

pub type ConnectionStateChange = RtcPeerConnectionState;
make_event_stream!(
    ConnectionStateStream,
    connection_state_stream,
    RtcPeerConnection,
    "connectionstatechange",
    Event,
    ConnectionStateChange,
    |event: &Event| {
        event
            .target()
            .and_then(|target| target.dyn_into::<RtcPeerConnection>().ok())
            .map(|pc| pc.connection_state())
    }
);

pub type IceConnectionStateChange = RtcIceConnectionState;
make_event_stream!(
    IceConnectionStateStream,
    ice_connection_state_stream,
    RtcPeerConnection,
    "iceconnectionstatechange",
    Event,
    IceConnectionStateChange,
    |event: &Event| {
        event
            .target()
            .and_then(|target| target.dyn_into::<RtcPeerConnection>().ok())
            .map(|pc| pc.ice_connection_state())
    }
);

pub type IceGatheringStateChange = RtcIceGatheringState;
make_event_stream!(
    IceGatheringStateStream,
    ice_gathering_state_stream,
    RtcPeerConnection,
    "icegatheringstatechange",
    Event,
    IceGatheringStateChange,
    |event: &Event| {
        event
            .target()
            .and_then(|target| target.dyn_into::<RtcPeerConnection>().ok())
            .map(|pc| pc.ice_gathering_state())
    }
);

pub type SignalingStateChange = RtcSignalingState;
make_event_stream!(
    SignalingStateStream,
    signaling_state_stream,
    RtcPeerConnection,
    "signalingstatechange",
    Event,
    SignalingStateChange,
    |event: &Event| {
        event
            .target()
            .and_then(|target| target.dyn_into::<RtcPeerConnection>().ok())
            .map(|pc| pc.signaling_state())
    }
);
