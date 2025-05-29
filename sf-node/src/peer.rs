use std::io;

use sf_core::transport::TransportError;

pub(crate) mod manager;
pub(crate) mod task;

pub(crate) enum PendingOutboundConnectionError {
	Aborted,
}

pub(crate) enum PendingInboundConnectionError {
	Aborted,
	Transport(TransportError<io::Error>),
}
