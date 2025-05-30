pub async fn dial(&mut self, remote_peer_id: PeerId, address: Multiaddr) -> Result<(), Error> {
	info!(peer_id = %self.peer_id, %remote_peer_id, %address, "Attempting to dial");

	let protocol = extract_protocol_from_multiaddr(&address)?;

	let transport = self.transports.get_mut(&protocol).ok_or_else(|| {
		error!(peer_id = %self.peer_id, %remote_peer_id, %address, ?protocol, "Transport not found for protocol");
		Error::TransportNotFound(protocol)
	})?;

	let dial = transport.dial(address.clone()).map_err(|e| Error::Transport(Box::new(e)))?;

	match dial.await {
		Ok(_) => Ok(()),
		Err(e) => {
			error!(peer_id = %self.peer_id, %remote_peer_id, %address, ?e, "Failed to dial");
			Err(Error::Transport(Box::new(e)))
		}
	}
} 
