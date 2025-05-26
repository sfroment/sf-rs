use std::{
	pin::Pin,
	task::{Context, Poll},
};

use futures::future::Future;
use multiaddr::{Multiaddr, PeerId};

use crate::Protocol;

pub trait Transport: Send + Sync + 'static {
	type Connection: Send + 'static;
	type Error: std::error::Error + Send + Sync;
	type Dial: Future<Output = Result<Self::Connection, Self::Error>> + Send;

	fn supported_protocols_for_dialing(&self) -> Protocol;
	fn dial(&self, peer_id: PeerId, address: Multiaddr) -> Self::Dial;
	fn listen_on(&mut self, address: Multiaddr) -> Result<(), Self::Error>;

	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<TransportEvent>;
}

pub enum TransportEvent {
	NewConnection { address: Multiaddr },
	ListenAddr { address: Multiaddr },

	AddrExpired { address: Multiaddr },

	ListenError { error: std::io::Error },
}
