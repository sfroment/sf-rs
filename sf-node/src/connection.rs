//use std::pin::Pin;

//use crate::{error::Error, stream::Stream};
//use multiaddr::{Multiaddr, PeerId};
//use sf_core::Connection as ConnectionTrait;

//pub enum Connection {
//	WebTransport(sf_wt_transport::Connection),
//}

//impl Connection {
//	pub async fn open_stream(&mut self) -> Result<Stream, Error> {
//		match self {
//			Self::WebTransport(connection) => {
//				let stream = connection
//					.open_stream()
//					.await
//					.map_err(|e| Error::Transport(Box::new(e)))?;
//				Ok(Stream::WebTransport(stream))
//			}
//		}
//	}

//	pub async fn accept_stream(&mut self) -> Result<Stream, Error> {
//		match self {
//			Self::WebTransport(connection) => {
//				let stream = connection
//					.accept_stream()
//					.await
//					.map_err(|e| Error::Transport(Box::new(e)))?;
//				Ok(Stream::WebTransport(stream))
//			}
//		}
//	}
//}

//impl ConnectionTrait for Connection {
//	type Output = Stream;
//	type Error = Error;
//	type Close = Pin<Box<dyn Future<Output = Result<(), Self::Error>> + Send>>;
//	type Stream = Pin<Box<dyn Future<Output = Result<Self::Output, Self::Error>> + Send>>;

//	fn open_stream(&mut self) -> Self::Stream {
//		match self {
//			Self::WebTransport(connection) => {
//				let fut = connection.open_stream();
//				Box::pin(async move {
//					let stream = fut.await.map_err(|e| Error::Transport(Box::new(e)))?;
//					Ok(Stream::WebTransport(stream))
//				})
//			}
//		}
//	}

//	fn accept_stream(&mut self) -> Self::Stream {
//		match self {
//			Self::WebTransport(connection) => {
//				let fut = connection.accept_stream();
//				Box::pin(async move {
//					let stream = fut.await.map_err(|e| Error::Transport(Box::new(e)))?;
//					Ok(Stream::WebTransport(stream))
//				})
//			}
//		}
//	}

//	fn close(&mut self) -> Self::Close {
//		match self {
//			Self::WebTransport(connection) => {
//				let fut = connection.close();
//				Box::pin(async move { fut.await.map_err(|e| Error::Transport(Box::new(e))) })
//			}
//		}
//	}

//	fn remote_address(&self) -> &Multiaddr {
//		match self {
//			Self::WebTransport(connection) => connection.remote_address(),
//		}
//	}

//	fn remote_peer_id(&self) -> Option<PeerId> {
//		match self {
//			Self::WebTransport(connection) => connection.remote_peer_id(),
//		}
//	}
//}

//impl From<sf_wt_transport::Connection> for Connection {
//	fn from(connection: sf_wt_transport::Connection) -> Self {
//		Self::WebTransport(connection)
//	}
//}
