#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[cfg(not(target_arch = "wasm32"))]
	#[error("invalid server")]
	InvalidServer,
	#[cfg(not(target_arch = "wasm32"))]
	#[error("invalid quic endpoint {0}")]
	InvalidQuicEndpoint(anyhow::Error),

	#[cfg(not(target_arch = "wasm32"))]
	#[error("invalid multiaddr")]
	InvalidMultiaddr,
}
