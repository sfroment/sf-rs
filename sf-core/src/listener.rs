//use std::{
//	pin::Pin,
//	task::{Context, Poll},
//};

//use futures::Stream;
//use multiaddr::Multiaddr;

//use crate::{Connection, TransportEvent};

//pub type AcceptResult<T> = Result<T, std::io::Error>;

//pub trait Listener: Stream<Item = TransportEvent> + Send + Sync + Unpin + 'static {
//	type Connection: Connection;
//	type Error: std::error::Error + Send + Sync + 'static;

//	fn local_address(&self) -> Multiaddr;

//	fn poll_if_addr(&mut self, cx: &mut std::task::Context<'_>) -> std::task::Poll<<Self as Stream>::Item>;
//}

//pub trait AbstractListener<C, E>: Send + Sync + Unpin
//where
//	C: Connection + Send + Sync,
//	E: std::error::Error + Send + Sync + 'static,
//{
//	fn local_address(&self) -> Multiaddr;

//	fn poll_stream(&mut self, cx: &mut Context<'_>) -> Poll<Option<TransportEvent>>;

//	fn poll_if_addr(&mut self, cx: &mut Context<'_>) -> Poll<TransportEvent>;
//}

//// Blanket implementation
//impl<T> AbstractListener<T::Connection, T::Error> for T
//where
//	T: Listener + Send + Sync + Unpin + 'static,
//	T::Connection: Connection + Send + Sync,
//	T::Error: std::error::Error + Send + Sync + 'static,
//{
//	fn local_address(&self) -> Multiaddr {
//		Listener::local_address(self)
//	}

//	fn poll_stream(&mut self, cx: &mut Context<'_>) -> Poll<Option<TransportEvent>> {
//		Stream::poll_next(Pin::new(self), cx)
//	}
//	fn poll_if_addr(&mut self, cx: &mut Context<'_>) -> Poll<TransportEvent> {
//		Listener::poll_if_addr(self, cx)
//	}
//}

//impl<C, E> Stream for dyn AbstractListener<C, E> + '_
//where
//	C: Connection + Send + Sync,
//	E: std::error::Error + Send + Sync + 'static,
//{
//	type Item = TransportEvent;

//	fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
//		self.poll_stream(cx)
//	}
//}
