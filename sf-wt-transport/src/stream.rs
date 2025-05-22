use bytes::Bytes;
use futures::{AsyncRead, AsyncWrite, FutureExt, ready};
use std::{
	io,
	pin::Pin,
	task::{Context, Poll},
};

pub struct Stream {
	send_stream: web_transport::SendStream,
	recv_stream: web_transport::RecvStream,
	read_buf: Option<Bytes>,
}

impl Stream {
	pub fn new(send_stream: web_transport::SendStream, recv_stream: web_transport::RecvStream) -> Self {
		Self {
			send_stream,
			recv_stream,
			read_buf: None,
		}
	}
}

impl sf_core::Stream for Stream {
	type Error = crate::Error;

	fn close_read(&mut self) -> futures::future::BoxFuture<'_, Result<(), Self::Error>> {
		async move {
			// For webtransport, the receive stream is closed by the peer when the send stream is closed.
			Ok(())
		}
		.boxed()
	}

	fn close_send(&mut self) -> futures::future::BoxFuture<'_, Result<(), Self::Error>> {
		async move {
			self.send_stream.finish()?;
			Ok(())
		}
		.boxed()
	}
}

impl AsyncWrite for Stream {
	fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
		let len = buf.len();
		let fut = self.get_mut().send_stream.write(buf);
		let mut fut = Box::pin(fut);

		match ready!(fut.as_mut().poll(cx)) {
			Ok(_) => Poll::Ready(Ok(len)),
			Err(e) => Poll::Ready(Err(io::Error::other(e))),
		}
	}

	fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
		Poll::Ready(Ok(()))
	}

	fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
		match self.get_mut().send_stream.finish() {
			Ok(_) => Poll::Ready(Ok(())),
			Err(e) => Poll::Ready(Err(io::Error::other(e))),
		}
	}
}

impl AsyncRead for Stream {
	fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
		if let Some(bytes) = &mut self.read_buf {
			let len = buf.len();
			buf[..len].copy_from_slice(&bytes[..len]);

			if len < bytes.len() {
				self.read_buf = Some(bytes.slice(len..));
				return Poll::Ready(Ok(len));
			}

			self.read_buf = None;
			return Poll::Ready(Ok(len));
		}

		let bytes = {
			let read_fut = self.recv_stream.read(8192);
			let mut fut = Box::pin(read_fut);
			match ready!(fut.as_mut().poll(cx)) {
				Ok(Some(b)) => b,
				Ok(None) => return Poll::Ready(Ok(0)), // EOF
				Err(e) => return Poll::Ready(Err(io::Error::other(e))),
			}
		};

		let len = buf.len().min(bytes.len());
		buf[..len].copy_from_slice(&bytes[..len]);

		if len < bytes.len() {
			self.read_buf = Some(bytes.slice(len..));
			return Poll::Ready(Ok(len));
		}

		self.read_buf = None;
		Poll::Ready(Ok(len))
	}
}
