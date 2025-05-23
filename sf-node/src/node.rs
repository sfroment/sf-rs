use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};

use multiaddr::{Multiaddr, PeerId, Protocol as MultiaddrProtocol};
use sf_core::{Protocol, Transport as TransportTrait};
use tracing::{error, info};

use crate::connection::Connection;
use crate::error::Error;
use crate::transport::Transport;

pub struct Node {
	peer_id: PeerId,
	transports: HashMap<Protocol, Arc<Mutex<Transport>>>,
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
	pub fn new(peer_id: PeerId, transports: HashMap<Protocol, Arc<Mutex<Transport>>>) -> Self {
		Self { peer_id, transports }
	}

	pub async fn dial(&self, remote_peer_id: PeerId, address: Multiaddr) -> Result<Connection, Error> {
		info!(peer_id = %self.peer_id, %remote_peer_id, %address, "Attempting to dial");

		let protocol = extract_protocol_from_multiaddr(&address)?;

		let transport = self.transports.get(&protocol).ok_or_else(|| {
			error!(peer_id = %self.peer_id, %remote_peer_id, %address, ?protocol, "Transport not found for protocol");
			Error::TransportNotFound(protocol)
		})?;

		let dial = {
			let transport_guard = transport.lock().unwrap();
			transport_guard.dial(remote_peer_id, address.clone())
		};

		dial.await.inspect_err(|e| {
			error!(peer_id = %self.peer_id, %remote_peer_id, %address, ?e, "Failed to dial");
		})
	}

	pub async fn listen(&self, address: Multiaddr) -> Result<(), Error> {
		let protocol = extract_protocol_from_multiaddr(&address)?;

		let transport = self.transports.get(&protocol).ok_or_else(|| {
			error!(peer_id = %self.peer_id, %address, ?protocol, "Transport not found for protocol");
			Error::TransportNotFound(protocol)
		})?;

		transport.lock().unwrap().listen_on(address.clone()).inspect_err(|e| {
			error!(peer_id = %self.peer_id, %address, ?e, "Failed to listen");
		})
	}
}

fn extract_protocol_from_multiaddr(address: &Multiaddr) -> Result<Protocol, Error> {
	let mut components = address.iter();
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
