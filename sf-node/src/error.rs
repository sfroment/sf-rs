use multiaddr::Multiaddr;
use sf_core::Protocol;

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("no protocols in multiaddr: {0}")]
	NoProtocolsInMultiaddr(Multiaddr),

	#[error("transport not found for protocol: {0:?}")]
	TransportNotFound(Protocol),
}
