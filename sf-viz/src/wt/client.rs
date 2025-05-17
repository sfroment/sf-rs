use std::sync::Arc;

use dioxus::hooks::{use_context_provider, use_signal};
use moq_transfork::web_transport;
use tokio::sync::Mutex;
use tracing::info;

#[derive(Clone)]
pub struct Client {
	session: moq_transfork::Session,
}

impl Client {
	pub async fn new(url: &str) -> anyhow::Result<Self> {
		info!("url: {}", url);
		let url = url::Url::parse(url)?;

		let host_str = url.host_str().ok_or_else(|| anyhow::anyhow!("URL has no host"))?;
		let port = url.port().unwrap_or(80);

		let body = reqwest::get(format!("http://{host_str}:{port}/fingerprint")).await?;

		info!("body: {:?}", body);

		let fingerprint = hex::decode(body.text().await?)?;
		let client = web_transport::ClientBuilder::new()
			.with_congestion_control(web_transport::CongestionControl::LowLatency)
			.with_server_certificate_hashes(vec![fingerprint])
			.map_err(|e| anyhow::anyhow!("Failed to create WebTransport client: {}", e))?;

		let session = client
			.connect(&url)
			.await
			.map_err(|e| anyhow::anyhow!("Failed to connect to WebTransport endpoint: {}", e))?;
		let session = moq_transfork::Session::connect(session)
			.await
			.map_err(|e| anyhow::anyhow!("Failed to connect to session: {}", e))?;

		Ok(Self { session })
	}
}

pub type ClientContext = Arc<Mutex<Option<Client>>>;

pub fn use_client_context_provider() {
	use_context_provider(|| Arc::new(Mutex::new(None::<Client>)));
}
