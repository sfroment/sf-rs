use futures::FutureExt;
use futures::stream::FusedStream;
use multiaddr::{Multiaddr, PeerId, Protocol as MultiaddrProtocol};
use sf_core::muxing::StreamMuxerBox;
use sf_core::transport;
use sf_core::{Protocol, Transport, transport::Boxed, transport::TransportEvent};
use std::collections::{HashMap, VecDeque};
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use tracing::{error, info};

use crate::error::Error;
use crate::peer::manager;
use crate::{NodeEvent, peer};

pub struct Node {
	pub peer_id: PeerId,
	transports: HashMap<Protocol, Boxed<(PeerId, StreamMuxerBox)>>,
	peer_manager: peer::manager::Manager,
	pending_events: VecDeque<NodeEvent>,
}

impl Node {
	pub fn new(peer_id: PeerId, transports: HashMap<Protocol, Boxed<(PeerId, StreamMuxerBox)>>) -> Self {
		Self {
			peer_id,
			transports,
			pending_events: VecDeque::new(),
			peer_manager: manager::Manager::new(),
		}
	}

	pub async fn dial(&mut self, remote_peer_id: PeerId, remote_address: Multiaddr) -> Result<(), Error> {
		info!(peer_id = %self.peer_id, %remote_peer_id, %remote_address, "Attempting to dial");

		let protocol = extract_protocol_from_multiaddr(&remote_address)?;

		let transport = self.transports.get_mut(&protocol).ok_or_else(|| {
			error!(peer_id = %self.peer_id, %remote_peer_id, %remote_address, ?protocol, "Transport not found for protocol");
			Error::TransportNotFound(protocol)
		})?;

		let dial = transport
			.dial(remote_address.clone())
			.map_err(|e| Error::Transport(Box::new(e)))?
			.boxed();

		self.peer_manager.add_outgoing(dial, remote_address);

		Ok(())
	}

	pub async fn listen(&mut self, address: Multiaddr) -> Result<(), Error> {
		let protocol = extract_protocol_from_multiaddr(&address)?;

		let transport = self.transports.get_mut(&protocol).ok_or_else(|| {
			error!(peer_id = %self.peer_id, %address, ?protocol, "Transport not found for protocol");
			Error::TransportNotFound(protocol)
		})?;

		transport
			.listen_on(address.clone())
			.inspect_err(|e| {
				error!(peer_id = %self.peer_id, %address, ?e, "Failed to listen");
			})
			.map_err(|e| Error::Transport(Box::new(e)))?;

		Ok(())
	}

	fn poll_next_event(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<NodeEvent> {
		let this = &mut *self;

		'outer: loop {
			if let Some(event) = this.pending_events.pop_front() {
				return Poll::Ready(event);
			}

			match this.peer_manager.poll(cx) {
				Poll::Pending => {}
				Poll::Ready(event) => {
					this.handle_peer_event(event);
					continue 'outer;
				}
			}

			for v in this.transports.values_mut() {
				match Pin::new(v).poll(cx) {
					Poll::Ready(event) => {
						this.handle_transport_event(event);
						continue 'outer;
					}
					Poll::Pending => {}
				}
			}

			return Poll::Pending;
		}
	}

	fn handle_peer_event(&mut self, event: peer::manager::PeerEvent) {
		info!(peer_id = %self.peer_id, ?event, "Peer event");
	}

	fn handle_transport_event(
		&mut self,
		event: TransportEvent<<transport::Boxed<(PeerId, StreamMuxerBox)> as Transport>::ListenerUpgrade, io::Error>,
	) {
		match event {
			TransportEvent::Incoming {
				remote_addr,
				local_addr,
				upgrade,
			} => {
				self.peer_manager.add_incoming(upgrade, local_addr, remote_addr);
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
		if component == MultiaddrProtocol::WebTransport {
  				p2p_protocol = Some(Protocol::WebTransport);
  				break;
  			}
	}
	p2p_protocol.ok_or_else(|| Error::NoProtocolsInMultiaddr(address.clone()))
}
