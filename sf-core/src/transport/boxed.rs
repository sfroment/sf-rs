use std::{
	error::Error,
	fmt, io,
	pin::Pin,
	task::{Context, Poll},
};

use futures::{prelude::*, stream::FusedStream};
use multiaddr::Multiaddr;

use crate::{
	Protocol,
	transport::{Transport, TransportError, TransportEvent},
};

/// Creates a new [`Boxed`] transport from the given transport.
pub(crate) fn boxed<T>(transport: T) -> Boxed<T::Output>
where
	T: Transport + Send + Unpin + 'static,
	T::Error: Send + Sync,
	T::Dial: Send + 'static,
	T::ListenerUpgrade: Send + 'static,
{
	Boxed {
		inner: Box::new(transport) as Box<_>,
	}
}

pub struct Boxed<O> {
	inner: Box<dyn Abstract<O> + Send + Unpin>,
}

type Dial<O> = Pin<Box<dyn Future<Output = io::Result<O>> + Send>>;
type ListenerUpgrade<O> = Pin<Box<dyn Future<Output = io::Result<O>> + Send>>;

trait Abstract<O> {
	fn supported_protocols_for_dialing(&self) -> Protocol;
	fn listen_on(&mut self, addr: Multiaddr) -> Result<(), TransportError<io::Error>>;
	fn dial(&mut self, addr: Multiaddr) -> Result<Dial<O>, TransportError<io::Error>>;
	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<TransportEvent<ListenerUpgrade<O>, io::Error>>;
}

impl<O> fmt::Debug for Boxed<O> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "BoxedTransport")
	}
}

impl<O> Transport for Boxed<O> {
	type Output = O;
	type Error = io::Error;
	type ListenerUpgrade = ListenerUpgrade<O>;
	type Dial = Dial<O>;

	fn supported_protocols_for_dialing(&self) -> Protocol {
		self.inner.supported_protocols_for_dialing()
	}

	fn listen_on(&mut self, addr: Multiaddr) -> Result<(), TransportError<Self::Error>> {
		self.inner.listen_on(addr)
	}

	fn dial(&mut self, addr: Multiaddr) -> Result<Self::Dial, TransportError<Self::Error>> {
		self.inner.dial(addr)
	}

	fn poll(
		mut self: Pin<&mut Self>,
		cx: &mut Context<'_>,
	) -> Poll<TransportEvent<Self::ListenerUpgrade, Self::Error>> {
		Pin::new(self.inner.as_mut()).poll(cx)
	}
}

impl<O> Stream for Boxed<O> {
	type Item = TransportEvent<ListenerUpgrade<O>, io::Error>;

	fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
		Transport::poll(self, cx).map(Some)
	}
}

impl<O> FusedStream for Boxed<O> {
	fn is_terminated(&self) -> bool {
		false
	}
}

impl<T, O> Abstract<O> for T
where
	T: Transport<Output = O> + 'static,
	T::Error: Send + Sync,
	T::Dial: Send + 'static,
	T::ListenerUpgrade: Send + 'static,
{
	fn supported_protocols_for_dialing(&self) -> Protocol {
		Transport::supported_protocols_for_dialing(self)
	}

	fn listen_on(&mut self, addr: Multiaddr) -> Result<(), TransportError<io::Error>> {
		Transport::listen_on(self, addr).map_err(|e| e.map(box_err))
	}

	fn dial(&mut self, addr: Multiaddr) -> Result<Dial<O>, TransportError<io::Error>> {
		let fut = Transport::dial(self, addr)
			.map(|r| r.map_err(box_err))
			.map_err(|e| e.map(box_err))?;
		Ok(Box::pin(fut) as Dial<_>)
	}

	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<TransportEvent<ListenerUpgrade<O>, io::Error>> {
		self.poll(cx).map(|event| {
			event
				.map_upgrade(|upgrade| {
					let up = upgrade.map_err(box_err);
					Box::pin(up) as ListenerUpgrade<O>
				})
				.map_err(box_err)
		})
	}
}

fn box_err<E: Error + Send + Sync + 'static>(e: E) -> io::Error {
	io::Error::other(e)
}
