use futures::future::Future;
use multiaddr::{Multiaddr, PeerId};

use crate::{Listener, Protocol};

pub trait Transport: Send + Sync + 'static {
	type Connection: Send + 'static;
	type Error: std::error::Error + Send + Sync;
	type Listener: Listener;
	type Dial: Future<Output = Result<Self::Connection, Self::Error>> + Send;

	fn supported_protocols_for_dialing(&self) -> Protocol;
	fn dial(&self, peer_id: PeerId, address: Multiaddr) -> Self::Dial;
	fn listen_on(&mut self, address: Multiaddr) -> Result<Self::Listener, Self::Error>;
}
