use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use futures::StreamExt;
use multiaddr::{Multiaddr, PeerId, Protocol as MultiaddrProtocol};
use sf_core::{Protocol, Transport as TransportTrait};
use tracing::{error, info};

use crate::connection::Connection;
use crate::error::Error;
use crate::listener::Listener;
use crate::transport::Transport;

pub struct Node {
	peer_id: PeerId,
	transports: HashMap<Protocol, Arc<Mutex<Transport>>>,
	active_listeners: Vec<Listener>,
	//pending_event: Option,
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

pub enum Event {
	NewConnection,
}

impl Node {
	pub fn new(peer_id: PeerId, transports: HashMap<Protocol, Arc<Mutex<Transport>>>) -> Self {
		Self {
			peer_id,
			transports,
			active_listeners: Vec::new(),
		}
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

	pub async fn listen(&mut self, address: Multiaddr) -> Result<(), Error> {
		let protocol = extract_protocol_from_multiaddr(&address)?;

		let transport = self.transports.get(&protocol).ok_or_else(|| {
			error!(peer_id = %self.peer_id, %address, ?protocol, "Transport not found for protocol");
			Error::TransportNotFound(protocol)
		})?;

		let listener = transport.lock().unwrap().listen_on(address.clone()).inspect_err(|e| {
			error!(peer_id = %self.peer_id, %address, ?e, "Failed to listen");
		})?;

		self.active_listeners.push(listener);

		Ok(())
	}

	fn poll_next_event(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Event> {
		let this = &mut *self;

		'outer: loop {
			for v in this.active_listeners.iter_mut() {
				match Pin::new(v).poll_next_unpin(cx) {
					Poll::Ready(Some((_connection, address))) => {
						info!(peer_id = %this.peer_id, %address, "Accepted connection");
						//return Poll::Ready(Event::NewConnection);
						continue 'outer;
					}
					Poll::Ready(None) => {}
					Poll::Pending => {}
				}
			}

			return Poll::Pending;
		}
	}
}

impl futures::Stream for Node {
	type Item = Event;

	fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
		self.poll_next_event(cx).map(Some)
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
