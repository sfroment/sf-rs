pub mod connection;
pub mod error;
mod listener;
pub mod platform;
pub mod stream;

use futures::future::BoxFuture;

use connection::Connection;
pub use error::Error;
pub use listener::Listener;
use moq_native::quic;
use multiaddr::{Multiaddr, PeerId};
use sf_core::{Protocol, Transport};

pub struct WtTransport {
	#[cfg(not(target_arch = "wasm32"))]
	config: quic::Config,
	/// Allow dialing the MA by tcp to get the fingerprint.
	allow_tcp_fingerprint: bool,

	#[cfg(not(target_arch = "wasm32"))]
	listener: Option<Listener>,
}

impl WtTransport {
	#[cfg(not(target_arch = "wasm32"))]
	pub fn new(config: quic::Config, allow_tcp_fingerprint: bool) -> Self {
		Self {
			config,
			allow_tcp_fingerprint,
			listener: None,
		}
	}

	#[cfg(target_arch = "wasm32")]
	pub fn new(allow_tcp_fingerprint: bool) -> Self {
		Self { allow_tcp_fingerprint }
	}
}

impl Transport for WtTransport {
	type Listener = Listener;
	type Connection = Connection;
	type Error = Error;
	type DialReturn = BoxFuture<'static, Result<Self::Connection, Self::Error>>;

	fn supported_protocols_for_dialing(&self) -> Protocol {
		Protocol::WebTransport
	}

	fn dial(&self, _: PeerId, _: Multiaddr) -> Self::DialReturn {
		todo!()
	}

	fn listen_on(&mut self, addr: Multiaddr) -> Result<(), Self::Error> {
		let listener = platform::listen_on(&self.config, self.allow_tcp_fingerprint, addr)?;
		self.listener = Some(listener);
		Ok(())
	}
}
