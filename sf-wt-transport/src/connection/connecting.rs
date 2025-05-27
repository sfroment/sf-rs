use std::{
	pin::Pin,
	task::{Context, Poll},
};

use multiaddr::PeerId;

use crate::Error;

pub struct Connecting {}

impl Connecting {
	pub fn new() -> Self {
		Self {}
	}
}

impl Future for Connecting {
	type Output = Result<PeerId, Error>;

	fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		//let connection = match futures::ready!(self.connecting.poll_unpin(cx)) {
		//	Either::Right(_) => return Poll::Ready(Err(Error::HandshakeTimedOut)),
		//	Either::Left((connection, _)) => connection.map_err(ConnectionError)?,
		//};

		//let peer_id = Self::remote_peer_id(&connection);
		//let muxer = Connection::new(connection);

		let peer_id = PeerId::random();
		Poll::Ready(Ok(peer_id))
	}
}
