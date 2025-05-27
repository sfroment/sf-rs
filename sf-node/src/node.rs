use futures::stream::FusedStream;
use multiaddr::{Multiaddr, PeerId, Protocol as MultiaddrProtocol};
use sf_core::transport;
use sf_core::{Protocol, Transport, transport::Boxed, transport::TransportEvent};
use std::collections::{HashMap, VecDeque};
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use tracing::{error, info};

use crate::NodeEvent;
use crate::error::Error;
use crate::peer_manager::PeerManager;

pub struct Node {
	pub peer_id: PeerId,
	transports: HashMap<Protocol, Boxed<PeerId>>,
	peer_manager: PeerManager,
	pending_events: VecDeque<NodeEvent>,
}

impl Node {
	pub fn new(peer_id: PeerId, transports: HashMap<Protocol, Boxed<PeerId>>) -> Self {
		Self {
			peer_id,
			transports,
			pending_events: VecDeque::new(),
			peer_manager: PeerManager::new(),
		}
	}

	pub async fn dial(&self, remote_peer_id: PeerId, address: Multiaddr) -> Result<(), Error> {
		todo!();
		//info!(peer_id = %self.peer_id, %remote_peer_id, %address, "Attempting to dial");

		//let protocol = extract_protocol_from_multiaddr(&address)?;

		//let transport = self.transports.get(&protocol).ok_or_else(|| {
		//	error!(peer_id = %self.peer_id, %remote_peer_id, %address, ?protocol, "Transport not found for protocol");
		//	Error::TransportNotFound(protocol)
		//})?;

		//let dial = { transport.dial(remote_peer_id, address.clone()) };

		//dial.await.inspect_err(|e| {
		//	error!(peer_id = %self.peer_id, %remote_peer_id, %address, ?e, "Failed to dial");
		//})
	}

	pub async fn listen(&mut self, address: Multiaddr) -> Result<(), Error> {
		todo!();
		let protocol = extract_protocol_from_multiaddr(&address)?;

		//let transport = self.transports.get_mut(&protocol).ok_or_else(|| {
		//	error!(peer_id = %self.peer_id, %address, ?protocol, "Transport not found for protocol");
		//	Error::TransportNotFound(protocol)
		//})?;

		//transport.listen_on(address.clone()).inspect_err(|e| {
		//	error!(peer_id = %self.peer_id, %address, ?e, "Failed to listen");
		//})?;

		//Ok(())
	}

	fn poll_next_event(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<NodeEvent> {
		let this = &mut *self;

		'outer: loop {
			if let Some(event) = this.pending_events.pop_front() {
				return Poll::Ready(event);
			}

			for v in this.transports.values_mut() {
				match Pin::new(v).poll(cx) {
					Poll::Ready(event) => {
						match event {
							TransportEvent::Incoming {
								remote_addr: address,
								local_addr: _,
								upgrade: _,
							} => {
								info!(peer_id = %this.peer_id, %address, "Accepted connection");
							}
							TransportEvent::ListenAddress { address } => {
								info!(peer_id = %this.peer_id, %address, "Listening on");
							}
							TransportEvent::AddressExpired { address } => {
								info!(peer_id = %this.peer_id, %address, "Listen address expired");
							}
							TransportEvent::ListenerError { error } => {
								info!(peer_id = %this.peer_id, ?error, "Failed to listen");
							}
							TransportEvent::ListenerClosed { reason: _ } => {
								info!(peer_id = %this.peer_id, "Listen closed");
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

	fn handle_transport_event(
		&mut self,
		event: TransportEvent<<transport::Boxed<PeerId> as Transport>::ListenerUpgrade, io::Error>,
	) {
		match event {
			TransportEvent::Incoming {
				remote_addr: address,
				local_addr: _,
				upgrade: _,
			} => {
				info!(peer_id = %self.peer_id, %address, "Accepted connection");
			}
			TransportEvent::ListenAddress { address } => {
				info!(peer_id = %self.peer_id, %address, "Listening on");
			}
			TransportEvent::AddressExpired { address } => {
				info!(peer_id = %self.peer_id, %address, "Listen address expired");
			}
			TransportEvent::ListenerError { error } => {
				info!(peer_id = %self.peer_id, ?error, "Failed to listen");
			}
			TransportEvent::ListenerClosed { reason: _ } => {
				info!(peer_id = %self.peer_id, "Listen closed");
			}
		}
	}
}

impl futures::Stream for Node {
	type Item = NodeEvent;

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
	let components = address.iter();
	let mut p2p_protocol: Option<Protocol> = None;

	for component in components {
		match component {
			MultiaddrProtocol::WebTransport => {
				p2p_protocol = Some(Protocol::WebTransport);
				break;
			}
			_ => {}
		}
	}
	p2p_protocol.ok_or_else(|| Error::NoProtocolsInMultiaddr(address.clone()))
}
