use axum::{
	Router,
	extract::{ConnectInfo, Extension},
};
use core::net;
use hyper_serve::accept::DefaultAcceptor;
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};
use tracing::instrument;

pub struct Config {
	pub bind: net::SocketAddr,
	pub tls: moq_native::tls::Config,
}

pub struct Web {
	app: Router,
	server: hyper_serve::Server<DefaultAcceptor>,
}

impl Web {
	pub fn new(config: Config) -> Self {
		let fingerprint = config.tls.fingerprints.first().expect("missing certificate").clone();

		let app = axum::Router::new()
			.route("/fingerprint", axum::routing::get(get_fingerprint))
			.layer(Extension(fingerprint.clone()))
			.layer(CorsLayer::new().allow_origin(Any).allow_methods(Any));

		let server = hyper_serve::bind(config.bind);

		Self { app, server }
	}

	pub async fn run(self) -> anyhow::Result<()> {
		self.server
			.serve(self.app.into_make_service_with_connect_info::<SocketAddr>())
			.await?;
		Ok(())
	}
}

#[instrument(name = "get_fingerprint", skip_all, fields(remote_addr = %addr))]
async fn get_fingerprint(
	ConnectInfo(addr): ConnectInfo<SocketAddr>,
	Extension(fingerprint): Extension<String>,
) -> String {
	fingerprint
}
