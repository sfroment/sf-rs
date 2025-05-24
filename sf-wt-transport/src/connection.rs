use std::sync::Arc;

use futures::future::BoxFuture;
use multiaddr::{Multiaddr, PeerId};
use tokio::sync::Mutex;
use web_transport::Session;

use crate::error::Error;
use crate::stream::Stream;

pub struct Connection {
	session: Arc<Mutex<Session>>,
	remote_address: Multiaddr,
	remote_peer_id: Option<PeerId>,
}

impl Connection {
	pub fn new(session: Session) -> Self {
		Self {
			session: Arc::new(Mutex::new(session)),
			remote_address: "/ip4/127.0.0.1/tcp/0".parse().unwrap(),
			remote_peer_id: None,
		}
	}
}

impl sf_core::Connection for Connection {
	type Error = Error;
	type Output = Stream;

	type Close = BoxFuture<'static, Result<(), Self::Error>>;
	type Stream = BoxFuture<'static, Result<Self::Output, Self::Error>>;

	fn open_stream(&mut self) -> Self::Stream {
		let session = Arc::clone(&self.session);
		Box::pin(async move {
			let mut session = session.lock().await;
			let (send, recv) = session.open_bi().await?;
			Ok(Stream::new(send, recv))
		})
	}

	fn accept_stream(&mut self) -> Self::Stream {
		let session = Arc::clone(&self.session);
		Box::pin(async move {
			let mut session = session.lock().await;
			let (send, recv) = session.accept_bi().await?;
			Ok(Stream::new(send, recv))
		})
	}

	fn close(&mut self) -> Self::Close {
		let session = Arc::clone(&self.session);
		Box::pin(async move {
			let mut session = session.lock().await;
			session.close(0u32, "Closing connection");
			Ok(())
		})
	}

	fn remote_address(&self) -> &Multiaddr {
		&self.remote_address
	}

	fn remote_peer_id(&self) -> Option<PeerId> {
		self.remote_peer_id
	}
}
