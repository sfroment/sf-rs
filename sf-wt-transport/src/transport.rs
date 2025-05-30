use std::{
	collections::VecDeque,
	net::SocketAddr,
	pin::Pin,
	task::{Context, Poll},
};

use futures::{StreamExt, future::BoxFuture};

use moq_native::quic;
use multiaddr::{Multiaddr, PeerId};
use sf_core::{
	Protocol, Transport,
	transport::{TransportError, TransportEvent},
};

use crate::{
	Error,
	connection::{self, Connection},
	listener::Listener,
	platform,
};

pub struct WebTransport {
	#[cfg(not(target_arch = "wasm32"))]
	config: quic::Config,
	/// Allow dialing the MA by tcp to get the fingerprint.
	allow_tcp_fingerprint: bool,

	pending_events: VecDeque<TransportEvent<BoxFuture<'static, Result<(PeerId, Connection), Error>>, Error>>,

	keypair: libp2p_identity::Keypair,

	listener: Option<Listener>,
}

impl WebTransport {
	#[cfg(not(target_arch = "wasm32"))]
	pub fn new(config: quic::Config, allow_tcp_fingerprint: bool, keypair: libp2p_identity::Keypair) -> Self {
		Self {
			config,
			allow_tcp_fingerprint,
			pending_events: VecDeque::new(),
			keypair,
			listener: None,
		}
	}

	#[cfg(target_arch = "wasm32")]
	pub fn new(allow_tcp_fingerprint: bool) -> Self {
		Self { allow_tcp_fingerprint }
	}
}

impl Transport for WebTransport {
	type Output = (PeerId, Connection);
	type Error = Error;
	type Dial = BoxFuture<'static, Result<Self::Output, Self::Error>>;
	type ListenerUpgrade = BoxFuture<'static, Result<Self::Output, Self::Error>>;

	fn supported_protocols_for_dialing(&self) -> Protocol {
		Protocol::WebTransport
	}

	fn dial(&mut self, ma: Multiaddr) -> Result<Self::Dial, TransportError<Self::Error>> {
		let (remote_socket_address, peer_id) = remote_ma_to_socketaddr(&ma).unwrap();
		tracing::debug!(%remote_socket_address, ?peer_id, "dial");

		let allow_tcp_fingerprint = self.allow_tcp_fingerprint;
		let keypair = self.keypair.clone();

		Ok(Box::pin(async move {
			connection::upgrade_outbound(remote_socket_address, allow_tcp_fingerprint, keypair).await
		}))
	}

	fn listen_on(&mut self, addr: Multiaddr) -> Result<(), TransportError<Self::Error>> {
		let listener = platform::listen_on(
			&self.config,
			self.allow_tcp_fingerprint,
			addr.clone(),
			self.keypair.clone(),
		)
		.map_err(TransportError::Other)?;

		self.pending_events
			.push_back(TransportEvent::ListenAddress { address: addr });
		self.listener = Some(listener);
		Ok(())
	}

	#[tracing::instrument(level = "trace", name = "Transport::poll", skip(self, cx))]
	fn poll(
		mut self: Pin<&mut Self>,
		cx: &mut Context<'_>,
	) -> Poll<TransportEvent<Self::ListenerUpgrade, Self::Error>> {
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

fn url_from_socket_addr(addr: SocketAddr, scheme: &str) -> url::Url {
	let host = match addr.ip() {
		std::net::IpAddr::V6(ipv6) => format!("[{ipv6}]"), // brackets required for IPv6 in URLs
		ip => ip.to_string(),
	};
	let url_str = format!("{}://{}:{}", scheme, host, addr.port());
	url::Url::parse(&url_str).expect("invalid URL")
}

fn remote_ma_to_socketaddr(ma: &Multiaddr) -> Result<(SocketAddr, Option<PeerId>), Error> {
	if let Some((addr, peer_id)) = multiaddr_to_socketaddr(ma) {
		return Ok((addr, peer_id));
	}
	Err(Error::InvalidMultiaddr(ma.clone()))
}

fn multiaddr_to_socketaddr(addr: &Multiaddr) -> Option<(SocketAddr, Option<PeerId>)> {
	let mut iter = addr.iter();
	let proto1 = iter.next()?;
	let proto2 = iter.next()?;
	// quic version
	let _ = iter.next()?;
	// webtransport part
	let proto3 = iter.next()?;

	match proto3 {
		multiaddr::Protocol::WebTransport => {}
		_ => return None,
	}

	let mut peer_id = None;
	for proto in iter {
		match proto {
			multiaddr::Protocol::P2p(id) => {
				peer_id = Some(id);
			}
			_ => return None,
		}
	}

	match (proto1, proto2) {
		(multiaddr::Protocol::Ip4(ip), multiaddr::Protocol::Udp(port)) => {
			Some((SocketAddr::new(ip.into(), port), peer_id))
		}
		(multiaddr::Protocol::Ip6(ip), multiaddr::Protocol::Udp(port)) => {
			Some((SocketAddr::new(ip.into(), port), peer_id))
		}
		_ => None,
	}
}
