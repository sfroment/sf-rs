mod connection;
mod routes;
mod web;

use crate::web::{Config as WebConfig, Web};
use anyhow::Context;
use clap::Parser;
use connection::Connection;
use moq_native::quic;
use routes::Routes;
use tracing::info;

#[derive(Parser, Clone)]
pub struct Config {
	/// Listen on this address, both TCP and UDP.
	#[arg(long, short = 'b', default_value = "[::]:443")]
	pub bind: String,

	/// The TLS configuration.
	#[command(flatten)]
	pub tls: moq_native::tls::Args,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	tracing_subscriber::fmt()
		.with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
		.with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
		.init();
	let config = Config::parse();

	let bind = tokio::net::lookup_host(config.bind)
		.await
		.context("invalid bind address")?
		.next()
		.context("invalid bind address")?;

	let tls = config.tls.load()?;
	if tls.server.is_none() {
		anyhow::bail!("missing TLS certificates");
	}

	tracing::info!("TLS fingerprints: {:?}", tls.fingerprints);
	let quic = quic::Endpoint::new(quic::Config { bind, tls: tls.clone() })?;
	let mut server = quic.server.context("missing TLS certificate")?;

	let web = Web::new(WebConfig { bind, tls });
	tokio::spawn(async move { web.run().await.expect("failed to start web server") });

	tracing::info!(addr = %bind, "listening");

	let mut conn_id = 0;

	let routes = Routes::new(quic.client.clone());
	while let Some(conn) = server.accept().await {
		let session = Connection::new(conn_id, conn.into());
		conn_id += 1;

		let router = routes.router.clone();
		tokio::spawn(async move {
			session.run(router).await.ok();
			info!("Session closed");
		});
	}

	Ok(())
}
