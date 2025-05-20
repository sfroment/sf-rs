use multiaddr::{Multiaddr, PeerId};
use sf_core::{AcceptResult, Connection as CoreConnection};

use crate::error::Error;

pub struct Connection {}

impl CoreConnection for Connection {
	type Error = Error;

	async fn read(&mut self) -> Result<Vec<u8>, Error> {
		todo!()
	}

	async fn write(&mut self, data: &[u8]) -> Result<(), Error> {
		todo!()
	}

	async fn close(&mut self) -> Result<(), Error> {
		todo!()
	}

	fn remote_address(&self) -> &Multiaddr {
		todo!()
	}

	fn remote_peer_id(&self) -> Option<PeerId> {
		todo!()
	}
}
