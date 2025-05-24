use futures::future::Future;
use multiaddr::{Multiaddr, PeerId};

pub trait Connection: Unpin + Send + Sync + 'static {
	type Output;
	type Error: std::error::Error + Send + Sync + 'static;
	type Close: Future<Output = Result<(), Self::Error>>;
	type Stream: Future<Output = Result<Self::Output, Self::Error>>;

	fn open_stream(&mut self) -> Self::Stream;
	fn accept_stream(&mut self) -> Self::Stream;

	fn close(&mut self) -> Self::Close;
	fn remote_address(&self) -> &Multiaddr;
	fn remote_peer_id(&self) -> Option<PeerId>;
}
