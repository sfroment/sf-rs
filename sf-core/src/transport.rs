use futures::future::Future;
use multiaddr::{Multiaddr, PeerId};
use std::error::Error;

use crate::{Connection, Listener, Protocol};

pub trait Transport: Send + Sync + 'static {
	type Listener: Listener<Connection = Self::Connection>;
	type Connection: Connection;
	type Error: Error + Send + Sync + 'static;
	type DialReturn: Future<Output = Result<Self::Connection, Self::Error>>;

	fn supported_protocols_for_dialing(&self) -> Protocol;
	fn dial(&self, peer_id: PeerId, address: Multiaddr) -> Self::DialReturn;
	fn listen_on(&mut self, address: Multiaddr) -> Result<(), Self::Error>;
}
