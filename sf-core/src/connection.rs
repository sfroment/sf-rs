use futures::future::Future;
use multiaddr::{Multiaddr, PeerId};
use std::error::Error;

use crate::Stream;

pub trait Connection: Send + Sync + 'static {
	type Error: Error + Send + Sync + 'static;
	type Stream: Stream;
	type CloseReturn: Future<Output = Result<(), Self::Error>>;
	type StreamReturn: Future<Output = Result<Self::Stream, Self::Error>>;

	fn open_stream(&mut self) -> Self::StreamReturn;
	fn accept_stream(&mut self) -> Self::StreamReturn;

	fn close(&mut self) -> Self::CloseReturn;
	fn remote_address(&self) -> &Multiaddr;
	fn remote_peer_id(&self) -> Option<PeerId>;
}
