use anyhow::Context;
use clap::Parser;
use futures::StreamExt;
use libp2p_identity::Keypair;
use moq_native::quic;
use sf_node::{Builder, Event, Node};
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

	info!("Creating a node with WebTransport...");

	let keypair = Keypair::generate_ed25519();
	let mut builder = Builder::new(keypair);
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

	let config = quic::Config { bind, tls };

	let transport = sf_wt_transport::WebTransport::new(config, true);
	builder.with_web_transport(transport);
	let node: Node = builder.build();

	println!("Node created successfully with Peer ID: {}", node.peer_id);

	let address =
		"/ip4/127.0.0.1/udp/443/quic-v1/webtransport/p2p/12D3KooWQWBgSAg1Z4kjoonCwSmCwmtbP4ZQFAYyna6oYQPLhc8i"
			.parse()?;

	let peer_id = "12D3KooWQWBgSAg1Z4kjoonCwSmCwmtbP4ZQFAYyna6oYQPLhc8i".parse()?;

	node.dial(peer_id, address).await?;
	Ok(())
}
