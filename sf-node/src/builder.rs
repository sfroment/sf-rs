use std::collections::HashMap;

use sf_core::{Protocol, Transport as TransportTrait};

use crate::{Node, transport::Transport};

pub struct Builder {
	keypair: libp2p_identity::Keypair,
	transports: HashMap<Protocol, Transport>,
}

impl Builder {
	pub fn new(keypair: libp2p_identity::Keypair) -> Self {
		Self {
			keypair,
			transports: HashMap::new(),
		}
	}

	pub fn with_web_transport(&mut self, transport: sf_wt_transport::WebTransport) {
		self.transports
			.insert(transport.supported_protocols_for_dialing(), transport.into());
	}

	pub fn build(self) -> Node {
		Node::new(self.keypair.public().to_peer_id(), self.transports)
	}
}
