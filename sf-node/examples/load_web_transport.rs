use anyhow::Context;
use clap::Parser;
use futures::StreamExt;
use libp2p_identity::Keypair;
use moq_native::quic;
use sf_core::muxing::StreamMuxerBox;
use sf_core::transport::Transport;
use sf_node::{Builder, Node, NodeEvent};
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
		.with_file(true)
		.with_line_number(true)
		.with_target(true)
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

	builder.with_transport(|keypair| {
		sf_wt_transport::WebTransport::new(config, true, keypair.clone())
			.map(|(peer_id, connection)| (peer_id, StreamMuxerBox::new(connection)))
			.boxed()
	});
	let mut node: Node = builder.build();

	println!("Node created successfully with Peer ID: {}", node.peer_id);

	let address = "/ip4/0.0.0.0/udp/443/quic-v1/webtransport".parse().unwrap();

	node.listen(address).await?;

	//let address = loop {
	//	if let Event::NewListenAddr { address } = node.select_next_some().await {
	//		info!(address = %address, "Listening on");
	//		break address;
	//	}
	//};

	//info!(address = %address, "Listening on!!!!!!");

	loop {
		tracing::info!("loop");
		tokio::select! {
			event = node.next() => {
				tracing::trace!(?event)
			},
			_ = tokio::signal::ctrl_c() => {
				// TODO: Handle shutdown gracefully.
				info!("Ctrl+C received, shutting down");
				break;
			}
		}
	}

	Ok(())
}
