//use std::pin::Pin;

//use futures::future::Future;
//use multiaddr::{Multiaddr, PeerId};

//// Define a type alias for the boxed error to make it cleaner
//pub type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

//pub trait Connection: Unpin + Send + Sync + 'static {
//	type Output: Send;
//	type Error: std::error::Error + Send + Sync + 'static;
//	type Close: Future<Output = Result<(), Self::Error>> + Send;
//	type Stream: Future<Output = Result<Self::Output, Self::Error>> + Send;

//	fn open_stream(&mut self) -> Self::Stream;
//	fn accept_stream(&mut self) -> Self::Stream;

//	fn close(&mut self) -> Self::Close;
//	fn remote_address(&self) -> &Multiaddr;
//	fn remote_peer_id(&self) -> Option<PeerId>;
//}

//pub trait AbstractConnection<Output, Error>: Send + Sync
//where
//	Output: Send + 'static,
//	Error: std::error::Error + Send + Sync + 'static,
//{
//	fn open_stream(&mut self) -> Pin<Box<dyn Future<Output = Result<Output, Error>> + Send + '_>>;

//	fn accept_stream(&mut self) -> Pin<Box<dyn Future<Output = Result<Output, Error>> + Send + '_>>;

//	fn close(&mut self) -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send + '_>>;

//	fn remote_address(&self) -> &Multiaddr;

//	fn remote_peer_id(&self) -> Option<PeerId>;
//}

//// Blanket implementation
//impl<T> AbstractConnection<T::Output, T::Error> for T
//where
//	T: Connection,
//	T::Output: Send + 'static,
//	T::Error: std::error::Error + Send + Sync + 'static,
//{
//	fn open_stream(&mut self) -> Pin<Box<dyn Future<Output = Result<T::Output, T::Error>> + Send + '_>> {
//		Box::pin(Connection::open_stream(self))
//	}

//	fn accept_stream(&mut self) -> Pin<Box<dyn Future<Output = Result<T::Output, T::Error>> + Send + '_>> {
//		Box::pin(Connection::accept_stream(self))
//	}

//	fn close(&mut self) -> Pin<Box<dyn Future<Output = Result<(), T::Error>> + Send + '_>> {
//		Box::pin(Connection::close(self))
//	}

//	fn remote_address(&self) -> &Multiaddr {
//		Connection::remote_address(self)
//	}

//	fn remote_peer_id(&self) -> Option<PeerId> {
//		Connection::remote_peer_id(self)
//	}
//}

////// Define a concrete error wrapper that implements std::error::Error
////#[derive(Debug)]
////pub struct ErrorWrapper(BoxedError);

////impl std::fmt::Display for ErrorWrapper {
////	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
////		self.0.fmt(f)
////	}
////}

////impl std::error::Error for ErrorWrapper {
////	fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
////		self.0.source()
////	}
////}

////impl From<BoxedError> for ErrorWrapper {
////	fn from(err: BoxedError) -> Self {
////		Self(err)
////	}
////}

////// Object-safe trait that uses the concrete error type
////trait ObjectSafeConnection<Output>: Send + Sync + Unpin {
////	fn open_stream(&mut self) -> Pin<Box<dyn Future<Output = Result<Output, ErrorWrapper>> + Send + '_>>;
////	fn accept_stream(&mut self) -> Pin<Box<dyn Future<Output = Result<Output, ErrorWrapper>> + Send + '_>>;
////	fn close(&mut self) -> Pin<Box<dyn Future<Output = Result<(), ErrorWrapper>> + Send + '_>>;
////	fn remote_address(&self) -> &Multiaddr;
////	fn remote_peer_id(&self) -> Option<PeerId>;
////}

////// Boxed Connection
////pub struct BoxedConnection<Output> {
////	inner: Box<dyn ObjectSafeConnection<Output> + Send + Unpin>,
////}

////impl<Output> BoxedConnection<Output>
////where
////	Output: Send + 'static,
////{
////	pub fn new<T, E>(connection: T) -> Self
////	where
////		T: AbstractConnection<Output, E> + Send + Unpin + 'static,
////		E: std::error::Error + Send + Sync + 'static,
////	{
////		// Wrapper is now generic over T_conn (the connection type) and E_conn_err (its error type)
////		struct Wrapper<T_conn, E_conn_err>(T_conn, std::marker::PhantomData<fn() -> E_conn_err>);

////		// The impl for Wrapper now uses its own generic parameters T_wrapper and E_wrapper
////		impl<T_wrapper, O_wrapper, E_wrapper_err> ObjectSafeConnection<O_wrapper> for Wrapper<T_wrapper, E_wrapper_err>
////		where
////			T_wrapper: AbstractConnection<O_wrapper, E_wrapper_err> + Send + Sync + Unpin,
////			O_wrapper: Send + 'static,
////			E_wrapper_err: std::error::Error + Send + Sync + 'static,
////		{
////			fn open_stream(&mut self) -> Pin<Box<dyn Future<Output = Result<O_wrapper, ErrorWrapper>> + Send + '_>> {
////				Box::pin(async move {
////					self.0
////						.open_stream()
////						.await
////						.map_err(|e| ErrorWrapper::from(Box::new(e) as BoxedError))
////				})
////			}

////			fn accept_stream(&mut self) -> Pin<Box<dyn Future<Output = Result<O_wrapper, ErrorWrapper>> + Send + '_>> {
////				Box::pin(async move {
////					self.0
////						.accept_stream()
////						.await
////						.map_err(|e| ErrorWrapper::from(Box::new(e) as BoxedError))
////				})
////			}

////			fn close(&mut self) -> Pin<Box<dyn Future<Output = Result<(), ErrorWrapper>> + Send + '_>> {
////				Box::pin(async move {
////					self.0
////						.close()
////						.await
////						.map_err(|e| ErrorWrapper::from(Box::new(e) as BoxedError))
////				})
////			}

////			fn remote_address(&self) -> &Multiaddr {
////				self.0.remote_address()
////			}

////			fn remote_peer_id(&self) -> Option<PeerId> {
////				self.0.remote_peer_id()
////			}
////		}

////		Self {
////			// Instantiate Wrapper with PhantomData for the error type E from new()
////			inner: Box::new(Wrapper(connection, std::marker::PhantomData)),
////		}
////	}

////	// Expose the methods
////	pub async fn open_stream(&mut self) -> Result<Output, ErrorWrapper> {
////		self.inner.open_stream().await
////	}

////	pub async fn accept_stream(&mut self) -> Result<Output, ErrorWrapper> {
////		self.inner.accept_stream().await
////	}

////	pub async fn close(&mut self) -> Result<(), ErrorWrapper> {
////		self.inner.close().await
////	}

////	pub fn remote_address(&self) -> &Multiaddr {
////		self.inner.remote_address()
////	}

////	pub fn remote_peer_id(&self) -> Option<PeerId> {
////		self.inner.remote_peer_id()
////	}
////}
