use axum::{
	Router,
	extract::{ConnectInfo, Extension},
};
use core::net;
use hyper_serve::accept::DefaultAcceptor;
use moq_native::quic;
use multiaddr::{Multiaddr, Protocol};
use sf_core::transport::TransportEvent;
use std::net::{IpAddr, SocketAddr};
use tower_http::cors::{Any, CorsLayer};
use tracing::instrument;

use crate::{Error, listener::Listener};

pub(crate) struct WebConfig {
	pub(crate) bind: net::SocketAddr,
	pub(crate) tls: moq_native::tls::Config,
}

pub fn extract_ip_port(addr: Multiaddr) -> Result<(IpAddr, u16), Error> {
	let mut found_ip: Option<IpAddr> = None;
	let mut found_port: Option<u16> = None;

	for p in addr.iter() {
		match p {
			Protocol::Ip4(ip4_addr) => {
				found_ip = Some(IpAddr::V4(ip4_addr));
			}
			Protocol::Ip6(ip6_addr) => {
				found_ip = Some(IpAddr::V6(ip6_addr));
			}
			Protocol::Udp(port_num) => {
				found_port = Some(port_num);
			}
			Protocol::Tcp(port_num) => {
				found_port = Some(port_num);
			}
			_ => {}
		}
	}

	match (found_ip, found_port) {
		(Some(ip), Some(port)) => Ok((ip, port)),
		(None, _) => Err(Error::InvalidMultiaddr(addr)),
		(_, None) => Err(Error::InvalidMultiaddr(addr)),
	}
}

pub fn listen_on(config: &quic::Config, allow_tcp_fingerprint: bool, addr: Multiaddr) -> Result<Listener, Error> {
	let (ip, port) = extract_ip_port(addr.clone())?;
	let bind = SocketAddr::new(ip, port);
	let (if_watcher, pending_event) = if bind.ip().is_unspecified() {
		(Some(if_watch::tokio::IfWatcher::new().map_err(Error::Io)?), None)
	} else {
		(None, Some(TransportEvent::ListenAddress { address: addr.clone() }))
	};

	let quic = quic::Endpoint::new(quic::Config {
		bind,
		tls: config.tls.clone(),
	})
	.map_err(Error::InvalidQuicEndpoint)?;
	let server = quic.server.ok_or(Error::InvalidServer)?;

	let local_addr = server.local_addr().map_err(Error::InvalidQuicEndpoint)?;

	let mut handle = None;
	if allow_tcp_fingerprint {
		let web_server = Web::new(WebConfig {
			bind: local_addr,
			tls: config.tls.clone(),
		});
		handle = Some(web_server.handle.clone());
		tokio::spawn(async move { web_server.run().await.expect("failed to start web server") });
	}

	Ok(Listener::new(
		server,
		local_addr,
		handle,
		addr,
		if_watcher,
		pending_event,
	))
}

struct Web {
	app: Router,
	handle: hyper_serve::Handle,
	server: hyper_serve::Server<DefaultAcceptor>,
}

impl Web {
	pub fn new(config: WebConfig) -> Self {
		let fingerprint = config.tls.fingerprints.first().expect("missing certificate").clone();

		let app = axum::Router::new()
			.route("/fingerprint", axum::routing::get(get_fingerprint))
			.layer(Extension(fingerprint.clone()))
			.layer(CorsLayer::new().allow_origin(Any).allow_methods(Any));

		let handle = hyper_serve::Handle::new();
		let server = hyper_serve::bind(config.bind).handle(handle.clone());

		Self { app, handle, server }
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
