mod stream;
mod upgrade;

use std::{pin::Pin, task::Context, task::Poll};

use futures::future::{BoxFuture, FutureExt};
use sf_core::muxing::{StreamMuxer, StreamMuxerEvent};
use web_transport::Session;

pub use stream::Stream;
pub(crate) use upgrade::{read_public_key, send_identity, upgrade_outbound};

use crate::Error;

pub struct Connection {
	session: Session,

	incoming: Option<BoxFuture<'static, Result<(web_transport::SendStream, web_transport::RecvStream), Error>>>,
	outgoing: Option<BoxFuture<'static, Result<(web_transport::SendStream, web_transport::RecvStream), Error>>>,

	closing: Option<BoxFuture<'static, web_transport::Error>>,
}

impl Connection {
	pub fn new(session: Session) -> Self {
		Self {
			session,
			incoming: None,
			outgoing: None,
			closing: None,
		}
	}
}

impl StreamMuxer for Connection {
	type Substream = Stream;
	type Error = Error;

	fn poll_inbound(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<Self::Substream, Self::Error>> {
		let this = self.get_mut();

		let incoming = this.incoming.get_or_insert_with(|| {
			let mut session = this.session.clone();
			async move { session.accept_bi().await.map_err(Error::WebTransport) }.boxed()
		});

		let (send, recv) = futures::ready!(incoming.poll_unpin(cx))?;
		this.incoming.take();
		let stream = Stream::new(send, recv);
		Poll::Ready(Ok(stream))
	}

	fn poll_outbound(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<Self::Substream, Self::Error>> {
		let this = self.get_mut();

		let outgoing = this.outgoing.get_or_insert_with(|| {
			let mut session = this.session.clone();
			async move { session.open_bi().await.map_err(Error::WebTransport) }.boxed()
		});

		let (send, recv) = futures::ready!(outgoing.poll_unpin(cx))?;
		this.outgoing.take();
		let stream = Stream::new(send, recv);
		Poll::Ready(Ok(stream))
	}

	fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<StreamMuxerEvent, Self::Error>> {
		Poll::Pending
	}

	fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
		let this = self.get_mut();

		let closing = this.closing.get_or_insert_with(|| {
			this.session.close(0, "");
			let session = this.session.clone();
			async move { session.closed().await }.boxed()
		});

		let error = futures::ready!(closing.poll_unpin(cx));
		Poll::Ready(Err(Error::WebTransport(error)))
	}
}
