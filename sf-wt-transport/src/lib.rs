pub mod connection;
pub mod error;
mod listener;
pub mod platform;
pub mod stream;

use futures::future::BoxFuture;

pub use connection::Connection;
pub use error::Error;
pub use listener::Listener;
use moq_native::quic;
use multiaddr::{Multiaddr, PeerId};
use sf_core::{Protocol, Transport};
pub use stream::Stream;

pub struct WebTransport {
	#[cfg(not(target_arch = "wasm32"))]
	config: quic::Config,
	/// Allow dialing the MA by tcp to get the fingerprint.
	allow_tcp_fingerprint: bool,
}

impl WebTransport {
	#[cfg(not(target_arch = "wasm32"))]
	pub fn new(config: quic::Config, allow_tcp_fingerprint: bool) -> Self {
		Self {
			config,
			allow_tcp_fingerprint,
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
	type Listener = Listener;

	fn supported_protocols_for_dialing(&self) -> Protocol {
		Protocol::WebTransport
	}

	fn dial(&self, _: PeerId, _: Multiaddr) -> Self::Dial {
		todo!()
	}

	fn listen_on(&mut self, addr: Multiaddr) -> Result<Self::Listener, Self::Error> {
		let listener = platform::listen_on(&self.config, self.allow_tcp_fingerprint, addr)?;
		Ok(listener)
	}
}
