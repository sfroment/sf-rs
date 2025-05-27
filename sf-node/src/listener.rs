//use std::{
//	pin::Pin,
//	task::{Context, Poll},
//};

//use futures::Stream;
//use multiaddr::Multiaddr;
//use sf_core::{Listener as ListenerTrait, TransportEvent};

//use crate::{connection::Connection, error::Error};

//pub enum Listener {
//	WebTransport(sf_wt_transport::Listener),
//}

//impl ListenerTrait for Listener {
//	type Connection = Connection;
//	type Error = Error;

//	fn local_address(&self) -> Multiaddr {
//		match self {
//			Self::WebTransport(listener) => listener.local_address(),
//		}
//	}

//	fn poll_if_addr(&mut self, cx: &mut Context<'_>) -> Poll<<Self as Stream>::Item> {
//		match self {
//			Self::WebTransport(listener) => listener.poll_if_addr(cx),
//		}
//	}
//}

//impl Stream for Listener {
//	type Item = TransportEvent;

//	fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
//		match self.get_mut() {
//			Self::WebTransport(listener) => {
//				let result = Pin::new(listener).poll_next(cx);
//				match result {
//					Poll::Ready(Some(event)) => Poll::Ready(Some(event)),
//					Poll::Ready(None) => Poll::Ready(None),
//					Poll::Pending => Poll::Pending,
//				}
//			}
//		}
//	}
//}

//impl From<sf_wt_transport::Listener> for Listener {
//	fn from(listener: sf_wt_transport::Listener) -> Self {
//		Self::WebTransport(listener)
//	}
//}
