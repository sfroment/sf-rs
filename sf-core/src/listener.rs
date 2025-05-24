use futures::Stream;
use multiaddr::Multiaddr;

use crate::Connection;

pub type AcceptResult<T> = Result<T, std::io::Error>;

pub trait Listener: Stream<Item = (Self::Connection, Multiaddr)> + Send + Sync + Unpin + 'static {
	type Connection: Connection;
	type Error: std::error::Error + Send + Sync + 'static;

	fn local_address(&self) -> Multiaddr;
}
