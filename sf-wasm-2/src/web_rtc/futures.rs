use futures::{Sink, Stream};
use pin_project::{pin_project, pinned_drop};
use sf_metrics::Metrics;
use sf_peer_id::PeerID;
use std::{
    pin::Pin,
    task::{Context, Poll},
};
use wasm_bindgen::JsError;
use web_sys::MessageEvent;

use super::WebRTCError;

#[pin_project(PinnedDrop)]
pub struct WebRTC<M: Metrics> {
    m: M,
    from: PeerID,
    to: PeerID,
}

pub enum State {}

impl<M> WebRTC<M>
where
    M: Metrics,
{
    pub(crate) fn new() -> Self {
        unimplemented!()
    }

    pub(crate) fn close(&self) -> Option<JsError> {
        unimplemented!()
    }

    pub(crate) fn state(&self) -> State {
        unimplemented!()
    }
}

impl<M> TryFrom<web_sys::RtcPeerConnection> for WebRTC<M>
where
    M: Metrics,
{
    type Error = WebRTCError;

    fn try_from(_peer_connection: web_sys::RtcPeerConnection) -> Result<Self, Self::Error> {
        unimplemented!()
    }
}

impl<M> Sink<MessageEvent> for WebRTC<M>
where
    M: Metrics,
{
    type Error = WebRTCError;

    fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        unimplemented!()
    }

    fn start_send(self: Pin<&mut Self>, _item: MessageEvent) -> Result<(), Self::Error> {
        unimplemented!()
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        unimplemented!()
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        unimplemented!()
    }
}

impl<M> Stream for WebRTC<M>
where
    M: Metrics,
{
    type Item = Result<MessageEvent, WebRTCError>;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        unimplemented!()
    }
}

#[pinned_drop]
impl<M> PinnedDrop for WebRTC<M>
where
    M: Metrics,
{
    fn drop(self: Pin<&mut Self>) {
        unimplemented!()
    }
}
