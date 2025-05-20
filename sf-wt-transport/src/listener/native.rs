use futures::Stream;
use moq_native::quic;
use multiaddr::Multiaddr;
use std::task::{Context, Poll};
use std::{pin::Pin, sync::Mutex};

use sf_core::{AcceptResult, Listener as CoreListener};

use crate::{connection::Connection, error::Error};

pub struct Listener {
	quic: quic::Server,
	handle: Option<hyper_serve::Handle>,
}

impl Listener {
	pub fn new(quic: quic::Server, handle: Option<hyper_serve::Handle>) -> Self {
		Self { quic, handle }
	}
}

impl Stream for Listener {
	type Item = Result<(Connection, Multiaddr), Error>;

	fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
		todo!()
	}
}

impl CoreListener for Listener {
	type Error = Error;
	type Connection = Connection;

	fn local_address(&self) -> Multiaddr {
		todo!()
	}
}

unsafe impl Send for Listener {}

unsafe impl Sync for Listener {}

pub struct Web {}
