use futures::Stream;
use multiaddr::Multiaddr;
use std::error::Error;

use crate::Connection;

pub type AcceptResult<T> = Result<T, std::io::Error>;

pub trait Listener:
	Stream<Item = Result<(Self::Connection, Multiaddr), Self::Error>> + Send + Sync + Unpin + 'static
{
	type Connection: Connection;
	type Error: Error + Send + Sync + 'static;

	fn local_address(&self) -> Multiaddr;
}
