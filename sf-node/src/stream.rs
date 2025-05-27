//use futures::{AsyncRead, AsyncWrite, future::BoxFuture};
//use sf_core::Stream as StreamTrait;
//use std::{
//	io,
//	pin::Pin,
//	task::{Context, Poll},
//};

//use crate::error::Error;

//pub enum Stream {
//	WebTransport(sf_wt_transport::Stream),
//}

//impl StreamTrait for Stream {
//	type Error = Error;

//	fn close_send(&mut self) -> BoxFuture<'_, Result<(), Self::Error>> {
//		match self {
//			Self::WebTransport(stream) => {
//				Box::pin(async move { stream.close_send().await.map_err(|e| Error::Transport(Box::new(e))) })
//			}
//		}
//	}

//	fn close_read(&mut self) -> BoxFuture<'_, Result<(), Self::Error>> {
//		match self {
//			Self::WebTransport(stream) => {
//				Box::pin(async move { stream.close_read().await.map_err(|e| Error::Transport(Box::new(e))) })
//			}
//		}
//	}

//	fn close(&mut self) -> BoxFuture<'_, Result<(), Self::Error>> {
//		match self {
//			Self::WebTransport(stream) => {
//				Box::pin(async move { stream.close().await.map_err(|e| Error::Transport(Box::new(e))) })
//			}
//		}
//	}
//}

//impl AsyncRead for Stream {
//	fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
//		match self.get_mut() {
//			Self::WebTransport(stream) => Pin::new(stream).poll_read(cx, buf).map_err(Into::into),
//		}
//	}
//}

//impl AsyncWrite for Stream {
//	fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
//		match self.get_mut() {
//			Self::WebTransport(stream) => Pin::new(stream).poll_write(cx, buf).map_err(Into::into),
//		}
//	}

//	fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
//		match self.get_mut() {
//			Self::WebTransport(stream) => Pin::new(stream).poll_flush(cx).map_err(Into::into),
//		}
//	}

//	fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
//		match self.get_mut() {
//			Self::WebTransport(stream) => Pin::new(stream).poll_close(_cx).map_err(Into::into),
//		}
//	}
//}
