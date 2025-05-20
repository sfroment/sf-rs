use multiaddr::Multiaddr;
use std::task::Poll;

use sf_core::{AcceptResult, Listener as CoreListener};

use crate::{connection::Connection, error::Error};

pub struct Listener {}

impl CoreListener for Listener {
	type Error = Error;
	type Connection = Connection;

	fn poll_accept(&mut self) -> Poll<AcceptResult<(Self::Connection, Multiaddr)>> {
		unreachable!("WASM listener does not support accept")
	}

	fn local_address(&self) -> Multiaddr {
		unreachable!("WASM listener does not support local address")
	}
}
