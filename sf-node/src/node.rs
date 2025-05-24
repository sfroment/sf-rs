use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::stream::FusedStream;
use multiaddr::{Multiaddr, PeerId, Protocol as MultiaddrProtocol};
use sf_core::{Protocol, Transport as TransportTrait, TransportEvent};
use tracing::{error, info};

use crate::connection::Connection;
use crate::error::Error;
use crate::transport::Transport;

pub struct Node {
	pub peer_id: PeerId,
	transports: HashMap<Protocol, Transport>,
}

#[derive(Debug)]
pub enum Event {
	NewConnection,

	NewListenAddr { address: Multiaddr },
}

impl Node {
	pub fn new(peer_id: PeerId, transports: HashMap<Protocol, Transport>) -> Self {
		Self { peer_id, transports }
	}

	pub async fn dial(&self, remote_peer_id: PeerId, address: Multiaddr) -> Result<Connection, Error> {
		info!(peer_id = %self.peer_id, %remote_peer_id, %address, "Attempting to dial");

		let protocol = extract_protocol_from_multiaddr(&address)?;

		let transport = self.transports.get(&protocol).ok_or_else(|| {
			error!(peer_id = %self.peer_id, %remote_peer_id, %address, ?protocol, "Transport not found for protocol");
			Error::TransportNotFound(protocol)
		})?;

		let dial = { transport.dial(remote_peer_id, address.clone()) };

		dial.await.inspect_err(|e| {
			error!(peer_id = %self.peer_id, %remote_peer_id, %address, ?e, "Failed to dial");
		})
	}

	pub async fn listen(&mut self, address: Multiaddr) -> Result<(), Error> {
		let protocol = extract_protocol_from_multiaddr(&address)?;

		let transport = self.transports.get_mut(&protocol).ok_or_else(|| {
			error!(peer_id = %self.peer_id, %address, ?protocol, "Transport not found for protocol");
			Error::TransportNotFound(protocol)
		})?;

		transport.listen_on(address.clone()).inspect_err(|e| {
			error!(peer_id = %self.peer_id, %address, ?e, "Failed to listen");
		})?;

		Ok(())
	}

	fn poll_next_event(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Event> {
		let this = &mut *self;

		'outer: loop {
			for v in this.transports.values_mut() {
				match Pin::new(v).poll(cx) {
					Poll::Ready(event) => {
						match event {
							TransportEvent::NewConnection { address } => {
								info!(peer_id = %this.peer_id, %address, "Accepted connection");
							}
							TransportEvent::NewListenAddr { address } => {
								info!(peer_id = %this.peer_id, %address, "Listening on");
							}
						}
						continue 'outer;
					}
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

impl FusedStream for Node {
	fn is_terminated(&self) -> bool {
		false
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
