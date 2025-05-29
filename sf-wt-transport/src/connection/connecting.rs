use std::{
	net::SocketAddr,
	pin::Pin,
	task::{Context, Poll},
};

use futures::future::BoxFuture;
use multiaddr::PeerId;

use crate::Error;
use crate::connection::Connection;

pub struct Connecting {
	fut: BoxFuture<'static, Result<(PeerId, Connection), Error>>,
}

impl Connecting {
	pub fn new(remote_socket_address: SocketAddr, allow_tcp_fingerprint: bool, remote_peer_id: Option<PeerId>) -> Self {
		let fut = Box::pin(async move {
			let fingerprint = if allow_tcp_fingerprint {
				let response = reqwest::get(format!(
					"http://{}:{}/fingerprint",
					remote_socket_address.ip(),
					remote_socket_address.port()
				))
				.await
				.map_err(Error::ReqwestError)?;
				let fingerprint =
					hex::decode(response.text().await.map_err(Error::ReqwestError)?).map_err(Error::HexError)?;
				Some(fingerprint)
			} else {
				None
			};

			let client = web_transport::ClientBuilder::new()
				.with_congestion_control(web_transport::CongestionControl::LowLatency);

			let client = if let Some(fingerprint) = fingerprint {
				client
					.with_server_certificate_hashes(vec![fingerprint])
					.map_err(Error::WebTransport)?
			} else {
				client.with_system_roots().map_err(Error::WebTransport)?
			};

			let url = url_from_socket_addr(remote_socket_address, "https");

			let session = client.connect(&url).await.map_err(Error::WebTransport)?;

			let peer_id = remote_peer_id.unwrap_or_else(PeerId::random);
			let connection = Connection::new(session);

			Ok((peer_id, connection))
		});

		Self { fut }
	}
}

impl Future for Connecting {
	type Output = Result<(PeerId, Connection), Error>;

	fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		Pin::new(&mut self.fut).poll(cx)
	}
}

fn url_from_socket_addr(addr: SocketAddr, scheme: &str) -> url::Url {
	let host = match addr.ip() {
		std::net::IpAddr::V6(ipv6) => format!("[{ipv6}]"), // brackets required for IPv6 in URLs
		ip => ip.to_string(),
	};
	let url_str = format!("{}://{}:{}", scheme, host, addr.port());
	url::Url::parse(&url_str).expect("invalid URL")
}
