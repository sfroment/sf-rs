pub mod connection;
pub mod error;
mod listener;
pub mod platform;
pub mod stream;

use std::{
	collections::VecDeque,
	pin::Pin,
	task::{Context, Poll},
};

use futures::{StreamExt, future::BoxFuture};

pub use connection::Connection;
pub use error::Error;
pub use listener::Listener;
use moq_native::quic;
use multiaddr::{Multiaddr, PeerId};
use sf_core::{Protocol, Transport, TransportEvent};
pub use stream::Stream;

pub struct WebTransport {
	#[cfg(not(target_arch = "wasm32"))]
	config: quic::Config,
	/// Allow dialing the MA by tcp to get the fingerprint.
	allow_tcp_fingerprint: bool,

	pending_events: VecDeque<TransportEvent>,

	listener: Option<Listener>,
}

impl WebTransport {
	#[cfg(not(target_arch = "wasm32"))]
	pub fn new(config: quic::Config, allow_tcp_fingerprint: bool) -> Self {
		Self {
			config,
			allow_tcp_fingerprint,
			pending_events: VecDeque::new(),
			listener: None,
		}
	}

	#[cfg(target_arch = "wasm32")]
	pub fn new(allow_tcp_fingerprint: bool) -> Self {
		Self { allow_tcp_fingerprint }
	}
}

impl Transport for WebTransport {
	type Connection = Connection;
	type Error = Error;
	type Dial = BoxFuture<'static, Result<Connection, Error>>;

	fn supported_protocols_for_dialing(&self) -> Protocol {
		Protocol::WebTransport
	}

	fn dial(&self, _: PeerId, _: Multiaddr) -> Self::Dial {
		todo!()
	}

	fn listen_on(&mut self, addr: Multiaddr) -> Result<(), Self::Error> {
		let listener = platform::listen_on(&self.config, self.allow_tcp_fingerprint, addr.clone())?;

		self.pending_events
			.push_back(TransportEvent::NewListenAddr { address: addr });
		self.listener = Some(listener);
		Ok(())
	}

	#[tracing::instrument(level = "trace", name = "Transport::poll", skip(self, cx))]
	fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<TransportEvent> {
		if let Some(event) = self.pending_events.pop_front() {
			return Poll::Ready(event);
		}

		if let Some(listener) = self.listener.as_mut() {
			if let Poll::Ready(Some(event)) = listener.poll_next_unpin(cx) {
				return Poll::Ready(event);
			}
		}

		Poll::Pending
	}
}
