use std::collections::HashMap;

use multiaddr::PeerId;
use sf_core::{
	Protocol, Transport,
	muxing::StreamMuxerBox,
	transport::{self},
};

use crate::Node;

pub struct Builder {
	keypair: libp2p_identity::Keypair,
	transports: HashMap<Protocol, transport::Boxed<(PeerId, StreamMuxerBox)>>,
}

impl Builder {
	pub fn new(keypair: libp2p_identity::Keypair) -> Self {
		Self {
			keypair,
			transports: HashMap::new(),
		}
	}

	pub fn with_transport(&mut self, transport: transport::Boxed<(PeerId, StreamMuxerBox)>) {
		self.transports
			.insert(transport.supported_protocols_for_dialing(), transport.boxed());
	}

	pub fn build(self) -> Node {
		Node::new(self.keypair.public().to_peer_id(), self.transports)
	}
}
