use futures::{Stream, ready};
use moq_native::quic;
use multiaddr::{Multiaddr, Protocol};
use sf_core::{Connection as ConnectionTrait, Listener as ListenerTrait, TransportEvent};
use std::net::{IpAddr, SocketAddr};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use crate::connection::Connection;
use crate::error::Error;

pub struct Listener {
	bind: SocketAddr,
	handle: Option<hyper_serve::Handle>,
	addr: Multiaddr,

	accept: tokio::sync::mpsc::Receiver<web_transport::quinn::Session>,
	if_watcher: Option<if_watch::tokio::IfWatcher>,

	pending_event: Option<<Self as Stream>::Item>,
	accept_ready: bool,
}

impl Listener {
	pub fn new(
		mut quic: quic::Server,
		bind: SocketAddr,
		handle: Option<hyper_serve::Handle>,
		addr: Multiaddr,
		if_watcher: Option<if_watch::tokio::IfWatcher>,
		pending_event: Option<<Self as Stream>::Item>,
	) -> Self {
		let (tx, rx) = tokio::sync::mpsc::channel(16);

		tokio::spawn(async move {
			while let Some(session) = quic.accept().await {
				if tx.send(session).await.is_err() {
					break;
				}
			}
		});

		Self {
			accept: rx,
			bind,
			handle,
			addr,
			if_watcher,
			pending_event,
			accept_ready: false,
		}
	}
}

impl Drop for Listener {
	fn drop(&mut self) {
		if let Some(handle) = self.handle.take() {
			handle.graceful_shutdown(Some(Duration::from_secs(10)));
		}
	}
}

impl Stream for Listener {
	type Item = TransportEvent;

	fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
		loop {
			tracing::info!("poll_next");
			if let Some(event) = self.pending_event.take() {
				return Poll::Ready(Some(event));
			}
			if let Poll::Ready(event) = self.poll_if_addr(cx) {
				return Poll::Ready(Some(event));
			}

			match self.accept.poll_recv(cx) {
				Poll::Ready(Some(session)) => {
					self.accept_ready = false;
					let connection = Connection::new(session.into());
					let address = connection.remote_address().clone();
					tracing::trace!(address = %address, "New connection");
					return Poll::Ready(Some(TransportEvent::NewConnection { address }));
				}
				Poll::Ready(None) => {
					tracing::info!("poll_next quic none");
					// TODO: maybe shall close here ?
					continue;
				}
				Poll::Pending => {}
			};

			return Poll::Pending;
		}
	}
}

impl ListenerTrait for Listener {
	type Error = Error;
	type Connection = Connection;

	fn local_address(&self) -> Multiaddr {
		self.addr.clone()
	}

	fn poll_if_addr(&mut self, cx: &mut Context<'_>) -> Poll<<Self as Stream>::Item> {
		let Some(if_watcher) = self.if_watcher.as_mut() else {
			return Poll::Pending;
		};

		loop {
			match ready!(if_watcher.poll_if_event(cx)) {
				Ok(if_watch::IfEvent::Up(inet)) => {
					if let Some(listen_addr) = ip_to_listenaddr(&self.bind, inet.addr()) {
						tracing::debug!(address = %listen_addr, "New listen address");
						return Poll::Ready(TransportEvent::ListenAddr { address: listen_addr });
					}
				}
				Ok(if_watch::IfEvent::Down(inet)) => {
					if let Some(listen_addr) = ip_to_listenaddr(&self.bind, inet.addr()) {
						tracing::debug!(address = %listen_addr, "Expired listen address");
						return Poll::Ready(TransportEvent::AddrExpired { address: listen_addr });
					}
				}
				Err(error) => return Poll::Ready(TransportEvent::ListenError { error }),
			}
		}
	}
}

unsafe impl Send for Listener {}

unsafe impl Sync for Listener {}

fn ip_to_listenaddr(endpoint_addr: &SocketAddr, ip: IpAddr) -> Option<Multiaddr> {
	// True if either both addresses are Ipv4 or both Ipv6.
	if !is_same(&endpoint_addr.ip(), &ip) {
		return None;
	}
	let socket_addr = SocketAddr::new(ip, endpoint_addr.port());
	Some(socketaddr_to_multiaddr(&socket_addr))
}

fn socketaddr_to_multiaddr(socket_addr: &SocketAddr) -> Multiaddr {
	Multiaddr::empty()
		.with(socket_addr.ip().into())
		.with(Protocol::Udp(socket_addr.port()))
		.with(Protocol::QuicV1)
		.with(Protocol::WebTransport)
}

fn is_same(a: &IpAddr, b: &IpAddr) -> bool {
	matches!((a, b), (IpAddr::V4(_), IpAddr::V4(_)) | (IpAddr::V6(_), IpAddr::V6(_)))
}
