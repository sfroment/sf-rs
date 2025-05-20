pub mod connection;
pub mod error;
mod listener;
pub mod platform;

use connection::Connection;
pub use error::Error;
pub use listener::Listener;
use moq_native::quic;
use multiaddr::{Multiaddr, PeerId};
use sf_core::Transport;

pub struct WtTransport {
	#[cfg(not(target_arch = "wasm32"))]
	config: quic::Config,
	/// Allow dialing the MA by tcp to get the fingerprint.
	allow_tcp_fingerprint: bool,

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

	fn supported_protocols_for_dialing(&self) -> Vec<multiaddr::Protocol<'static>> {
		vec![
			multiaddr::Protocol::Ip4("0.0.0.0".parse().unwrap()),
			multiaddr::Protocol::Udp(0),
			multiaddr::Protocol::QuicV1,
			multiaddr::Protocol::WebTransport,
		]
	}

	async fn dial(&self, _: PeerId, _: Multiaddr) -> Result<Self::Connection, Self::Error> {
		todo!()
	}

	async fn listen_on(&mut self, addr: Multiaddr) -> Result<(), Self::Error> {
		let listener = platform::listen_on(self, addr).await?;
		self.listener = Some(listener);
		Ok(())
	}
}
