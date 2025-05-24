use futures::{FutureExt, Stream};
use moq_native::quic;
use multiaddr::Multiaddr;
use sf_core::Connection as CoreConnection;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use crate::connection::Connection;
use crate::error::Error;

pub struct Listener {
	quic: quic::Server,
	handle: Option<hyper_serve::Handle>,
	addr: Multiaddr,
}

impl Listener {
	pub fn new(quic: quic::Server, handle: Option<hyper_serve::Handle>, addr: Multiaddr) -> Self {
		Self { quic, handle, addr }
	}
}

impl Drop for Listener {
	fn drop(&mut self) {
		if let Some(handle) = self.handle.take() {
			handle.graceful_shutdown(Some(Duration::from_secs(10)));
		}
	}
}

impl Stream for Listener {
	type Item = (Connection, Multiaddr);

	fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
		match self.get_mut().quic.accept().boxed().poll_unpin(cx) {
			Poll::Ready(Some(session)) => {
				let connection = Connection::new(session.into());
				let addr = connection.remote_address().clone();
				Poll::Ready(Some((connection, addr)))
			}
			Poll::Ready(None) => Poll::Ready(None),
			Poll::Pending => Poll::Pending,
		}
	}
}

impl sf_core::Listener for Listener {
	type Error = Error;
	type Connection = Connection;

	fn local_address(&self) -> Multiaddr {
		self.addr.clone()
	}
}

unsafe impl Send for Listener {}

unsafe impl Sync for Listener {}
