use asynchronous_codec::{BytesCodec, Framed, LengthCodec};
use bytes::Bytes;
use futures::{AsyncReadExt, AsyncWriteExt, SinkExt, StreamExt, TryStreamExt};
use multiaddr::PeerId;
use std::net::SocketAddr;

use crate::connection::Connection;
use crate::{Error, connection::Stream};

/// Upgrade an outbound [`sf_core::Transport::dial`] connection to a WebTransport connection.
pub(crate) async fn upgrade_outbound(
	remote_socket_address: SocketAddr,
	allow_tcp_fingerprint: bool,
	keypair: libp2p_identity::Keypair,
) -> Result<(PeerId, Connection), Error> {
	tracing::info!(remote_socket_address = %remote_socket_address, "upgrade outbound");
	let fingerprint = if allow_tcp_fingerprint {
		let response = reqwest::get(format!(
			"http://{}:{}/fingerprint",
			remote_socket_address.ip(),
			remote_socket_address.port()
		))
		.await
		.map_err(Error::ReqwestError)?;
		let fingerprint = hex::decode(response.text().await.map_err(Error::ReqwestError)?).map_err(Error::HexError)?;
		Some(fingerprint)
	} else {
		None
	};

	let client =
		web_transport::ClientBuilder::new().with_congestion_control(web_transport::CongestionControl::LowLatency);

	let client = if let Some(fingerprint) = fingerprint {
		client
			.with_server_certificate_hashes(vec![fingerprint])
			.map_err(Error::WebTransport)?
	} else {
		client.with_system_roots().map_err(Error::WebTransport)?
	};

	let url = url_from_socket_addr(remote_socket_address, "https");

	let mut session = client.connect(&url).await.map_err(Error::WebTransport)?;
	let connection = Connection::new(session.clone());
	let (send, recv) = session.open_bi().await.map_err(Error::WebTransport)?;
	let stream = Stream::new(send, recv);
	let mut framed = Framed::new(stream, LengthCodec);
	send_identity(&mut framed, keypair).await?;

	let remote_public_key = read_public_key(&mut framed).await?;
	let peer_id = PeerId::from_public_key(&remote_public_key);

	Ok((peer_id, connection))
}

pub(crate) async fn read_public_key(
	framed: &mut Framed<Stream, LengthCodec>,
) -> Result<libp2p_identity::PublicKey, Error> {
	let bytes = framed
		.next()
		.await
		.transpose()?
		.ok_or_else(|| Error::Io(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "stream ended")))?;
	let remote_public_key = libp2p_identity::PublicKey::try_decode_protobuf(&bytes).map_err(Error::Libp2pIdentity)?;
	Ok(remote_public_key)
}

pub(crate) async fn send_identity(
	framed: &mut Framed<Stream, LengthCodec>,
	keypair: libp2p_identity::Keypair,
) -> Result<(), Error> {
	let public = keypair.public();

	framed.send(public.encode_protobuf().into()).await?;

	Ok(())
}

fn url_from_socket_addr(addr: SocketAddr, scheme: &str) -> url::Url {
	let host = match addr.ip() {
		std::net::IpAddr::V6(ipv6) => format!("[{ipv6}]"),
		ip => ip.to_string(),
	};
	let url_str = format!("{}://{}:{}", scheme, host, addr.port());
	url::Url::parse(&url_str).expect("invalid URL")
}
