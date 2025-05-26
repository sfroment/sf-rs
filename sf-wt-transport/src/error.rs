use multiaddr::Multiaddr;

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[cfg(not(target_arch = "wasm32"))]
	#[error("invalid server")]
	InvalidServer,
	#[cfg(not(target_arch = "wasm32"))]
	#[error("invalid quic endpoint {0}")]
	InvalidQuicEndpoint(anyhow::Error),

	#[cfg(not(target_arch = "wasm32"))]
	#[error("invalid multiaddr: {0}")]
	InvalidMultiaddr(Multiaddr),

	#[cfg(not(target_arch = "wasm32"))]
	#[error("invalid web transport session: {0}")]
	InvalidWebTransportSession(web_transport::Error),

	#[cfg(target_arch = "wasm32")]
	#[error("invalid web transport wasm session: {0:?}")]
	InvalidWebTransportSessionWasm(web_transport::Error),

	#[cfg(not(target_arch = "wasm32"))]
	#[error("io error: {0}")]
	Io(std::io::Error),

	#[error("reqwest error: {0}")]
	ReqwestError(reqwest::Error),

	#[error("hex error: {0}")]
	HexError(hex::FromHexError),

	#[error("web transport error: {0}")]
	WebTransport(web_transport::Error),

	#[error("moq transfork error: {0}")]
	MoqTransfork(moq_transfork::Error),
}

#[cfg(not(target_arch = "wasm32"))]
impl From<web_transport::Error> for Error {
	fn from(error: web_transport::Error) -> Self {
		Self::InvalidWebTransportSession(error)
	}
}

#[cfg(target_arch = "wasm32")]
impl From<web_transport::Error> for Error {
	fn from(error: web_transport::Error) -> Self {
		Self::InvalidWebTransportSessionWasm(error)
	}
}

pub type Result<T> = std::result::Result<T, Error>;
