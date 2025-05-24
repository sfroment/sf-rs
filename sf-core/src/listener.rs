use futures::Stream;
use multiaddr::Multiaddr;

use crate::{Connection, TransportEvent};

pub type AcceptResult<T> = Result<T, std::io::Error>;

pub trait Listener: Stream<Item = TransportEvent> + Send + Sync + Unpin + 'static {
	type Connection: Connection;
	type Error: std::error::Error + Send + Sync + 'static;

	fn local_address(&self) -> Multiaddr;
}
