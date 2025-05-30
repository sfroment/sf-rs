use futures::future::BoxFuture;
use futures::{Stream as FuturesStream, ready};
use moq_native::quic;
use multiaddr::{Multiaddr, PeerId, Protocol};
use sf_core::Transport;
use sf_core::transport::TransportEvent;
use std::net::{IpAddr, SocketAddr};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use crate::connection::Stream;
use crate::error::Error;
use crate::transport::WebTransport;
use crate::{Connection, connection};

pub struct Listener {
	bind: SocketAddr,
	handle: Option<hyper_serve::Handle>,

	accept: tokio::sync::mpsc::Receiver<web_transport::quinn::Session>,
	if_watcher: Option<if_watch::tokio::IfWatcher>,

	pending_event: Option<<Self as FuturesStream>::Item>,

	keypair: libp2p_identity::Keypair,
	accept_ready: bool,
}

impl Listener {
	pub fn new(
		mut quic: quic::Server,
		bind: SocketAddr,
		handle: Option<hyper_serve::Handle>,
		if_watcher: Option<if_watch::tokio::IfWatcher>,
		pending_event: Option<<Self as FuturesStream>::Item>,
		keypair: libp2p_identity::Keypair,
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
			if_watcher,
			pending_event,
			keypair,
			accept_ready: false,
		}
	}

	fn poll_if_addr(&mut self, cx: &mut Context<'_>) -> Poll<<Self as FuturesStream>::Item> {
		let Some(if_watcher) = self.if_watcher.as_mut() else {
			return Poll::Pending;
		};

		loop {
			match ready!(if_watcher.poll_if_event(cx)) {
				Ok(if_watch::IfEvent::Up(inet)) => {
					if let Some(listen_addr) = ip_to_listenaddr(&self.bind, inet.addr()) {
						tracing::debug!(address = %listen_addr, "New listen address");
						return Poll::Ready(TransportEvent::ListenAddress { address: listen_addr });
					}
				}
				Ok(if_watch::IfEvent::Down(inet)) => {
					if let Some(listen_addr) = ip_to_listenaddr(&self.bind, inet.addr()) {
						tracing::debug!(address = %listen_addr, "Expired listen address");
						return Poll::Ready(TransportEvent::AddressExpired { address: listen_addr });
					}
				}
				Err(error) => return Poll::Ready(TransportEvent::ListenerError { error: error.into() }),
			}
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

impl FuturesStream for Listener {
	type Item = TransportEvent<<WebTransport as Transport>::ListenerUpgrade, Error>;

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
					let remote_addr = session.remote_address();
					let remote_addr = socketaddr_to_multiaddr(&remote_addr);
					let local_addr = socketaddr_to_multiaddr(&self.bind);
					let keypair = self.keypair.clone();
					let connecting: BoxFuture<'static, Result<(PeerId, Connection), Error>> =
						Box::pin(async move { upgrade_inbound(session, keypair).await });
					tracing::info!(remote_addr = %remote_addr, local_addr = %local_addr, "New connection");
					let event = TransportEvent::Incoming {
						remote_addr,
						local_addr,
						upgrade: connecting,
					};

					return Poll::Ready(Some(event));
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

unsafe impl Send for Listener {}

unsafe impl Sync for Listener {}

pub(crate) async fn upgrade_inbound(
	session: web_transport::quinn::Session,
	keypair: libp2p_identity::Keypair,
) -> Result<(PeerId, Connection), Error> {
	let mut session: web_transport::Session = session.into();
	let (send, recv) = session.accept_bi().await.unwrap();
	let mut stream = Stream::new(send, recv);

	let remote_public_key = connection::read_public_key(&mut stream).await?;
	let peer_id = PeerId::from_public_key(&remote_public_key);

	connection::send_identity(&mut stream, keypair).await?;

	Ok((peer_id, Connection::new(session)))
}

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
