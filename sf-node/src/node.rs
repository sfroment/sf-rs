use std::collections::HashMap;

use multiaddr::{Multiaddr, PeerId, Protocol as MultiaddrProtocol};
use sf_core::{Protocol, Transport};
use tracing::{error, info};

use crate::builder::{BoxedConnection, DynTransportObject};
use crate::error::Error;

pub struct Node {
	peer_id: PeerId,
	transports: HashMap<Protocol, Box<DynTransportObject>>,
	//active_listeners: HashMap<Protocol, Box<DynTransportObject>>,
}

fn spawn<F>(future: F)
where
	F: Future + Send + 'static,
	F::Output: Send + 'static,
{
	#[cfg(not(target_arch = "wasm32"))]
	tokio::spawn(future);
	#[cfg(target_arch = "wasm32")]
	wasm_bindgen_futures::spawn_local(future);
}

impl Node {
	pub fn new(peer_id: PeerId, transports: HashMap<Protocol, Box<DynTransportObject>>) -> Self {
		Self { peer_id, transports }
	}

	pub async fn dial(&self, remote_peer_id: PeerId, address: Multiaddr) -> Result<BoxedConnection, Error> {
		info!(peer_id = %self.peer_id, %remote_peer_id, %address, "Attempting to dial");

		let protocol = extract_protocol_from_multiaddr(&address)?;

		let transport = self.transports.get(&protocol).ok_or_else(|| {
			error!(peer_id = %self.peer_id, %remote_peer_id, %address, ?protocol, "Transport not found for protocol");
			Error::TransportNotFound(protocol)
		})?;
		transport.dial(remote_peer_id, address);

		todo!()
	}
}

/// Extracts the primary P2P protocol from a Multiaddr.
/// For example, /ip4/.../tcp/... returns P2PProtocol::Tcp.
/// /ip4/.../udp/.../quic-v1/... returns P2PProtocol::QuicV1.
/// /ip4/.../udp/.../quic-v1/webtransport/... returns P2PProtocol::WebTransport.
fn extract_protocol_from_multiaddr(address: &Multiaddr) -> Result<Protocol, Error> {
	let mut components = address.iter();
	// Iterate through protocols to find the "highest level" P2P protocol
	// This order matters if addresses are nested in unusual ways.
	// We look for WebTransport first, then QuicV1, then TCP, then Udp (as a base for Quic/WebTransport).
	let mut p2p_protocol: Option<Protocol> = None;

	for component in components {
		match component {
			MultiaddrProtocol::WebTransport => {
				p2p_protocol = Some(Protocol::WebTransport); // WebTransport overrides QUIC
				break; // WebTransport is the most specific we look for here
			}
			_ => {}
		}
	}
	p2p_protocol.ok_or_else(|| Error::NoProtocolsInMultiaddr(address.clone()))
}
