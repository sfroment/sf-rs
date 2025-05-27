//use crate::{connection::Connection, error::Error};
//use multiaddr::{Multiaddr, PeerId};
//use sf_core::{Protocol, Transport as TransportTrait};
//use std::future::Future;
//use std::pin::Pin;

//pub enum Transport {
//	WebTransport(sf_wt_transport::WebTransport),
//}

//impl TransportTrait for Transport {
//	type Output = Connection;
//	type Error = Error;
//	type Dial = Pin<Box<dyn Future<Output = Result<Self::Output, Self::Error>> + Send>>;

//	fn supported_protocols_for_dialing(&self) -> Protocol {
//		match self {
//			Self::WebTransport(transport) => transport.supported_protocols_for_dialing(),
//		}
//	}

//	fn dial(&self, peer_id: PeerId, address: Multiaddr) -> Self::Dial {
//		match self {
//			Self::WebTransport(transport) => {
//				let fut = transport.dial(peer_id, address);
//				Box::pin(async move {
//					let connection = fut.await.map_err(|e| Error::Transport(Box::new(e)))?;
//					Ok(Connection::WebTransport(connection))
//				})
//			}
//		}
//	}

//	fn listen_on(&mut self, address: Multiaddr) -> Result<(), Self::Error> {
//		match self {
//			Self::WebTransport(transport) => transport.listen_on(address).map_err(|e| Error::Transport(Box::new(e))),
//		}
//	}

//	fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<sf_core::TransportEvent> {
//		match self.get_mut() {
//			Self::WebTransport(transport) => Pin::new(transport).poll(cx),
//		}
//	}
//}

//impl Transport {
//	pub fn supports_address(&self, addr: &Multiaddr) -> bool {
//		let mojave_protocol = self.supported_protocols_for_dialing();
//		addr.iter().any(|protocol| match protocol {
//			multiaddr::Protocol::WebTransport => mojave_protocol == Protocol::WebTransport,
//			_ => false,
//		})
//	}

//	pub fn protocol_name(&self) -> &'static str {
//		match self {
//			Self::WebTransport(_) => "webtransport",
//		}
//	}
//}

//impl From<sf_wt_transport::WebTransport> for Transport {
//	fn from(transport: sf_wt_transport::WebTransport) -> Self {
//		Self::WebTransport(transport)
//	}
//}
