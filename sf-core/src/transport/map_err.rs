use std::{
	error,
	pin::Pin,
	task::{Context, Poll},
};

use futures::prelude::*;
use multiaddr::Multiaddr;

use crate::{
	Protocol,
	transport::{Transport, TransportError, TransportEvent},
};

/// See `Transport::map_err`.
#[derive(Debug, Copy, Clone)]
#[pin_project::pin_project]
pub struct MapErr<T, F> {
	#[pin]
	transport: T,
	map: F,
}

impl<T, F> MapErr<T, F> {
	/// Internal function that builds a `MapErr`.
	pub(crate) fn new(transport: T, map: F) -> MapErr<T, F> {
		MapErr { transport, map }
	}
}

impl<T, F, TErr> Transport for MapErr<T, F>
where
	T: Transport,
	F: FnOnce(T::Error) -> TErr + Clone,
	TErr: error::Error,
{
	type Output = T::Output;
	type Error = TErr;
	type ListenerUpgrade = MapErrListenerUpgrade<T, F>;
	type Dial = MapErrDial<T, F>;

	fn supported_protocols_for_dialing(&self) -> Protocol {
		self.transport.supported_protocols_for_dialing()
	}

	fn listen_on(&mut self, addr: Multiaddr) -> Result<(), TransportError<Self::Error>> {
		let map = self.map.clone();
		self.transport.listen_on(addr).map_err(|err| err.map(map))
	}

	fn dial(&mut self, addr: Multiaddr) -> Result<Self::Dial, TransportError<Self::Error>> {
		let map = self.map.clone();
		match self.transport.dial(addr) {
			Ok(future) => Ok(MapErrDial {
				inner: future,
				map: Some(map),
			}),
			Err(err) => Err(err.map(map)),
		}
	}

	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<TransportEvent<Self::ListenerUpgrade, Self::Error>> {
		let this = self.project();
		let map = &*this.map;
		this.transport.poll(cx).map(|ev| {
			ev.map_upgrade(move |value| MapErrListenerUpgrade {
				inner: value,
				map: Some(map.clone()),
			})
			.map_err(map.clone())
		})
	}
}

/// Listening upgrade future for `MapErr`.
#[pin_project::pin_project]
pub struct MapErrListenerUpgrade<T: Transport, F> {
	#[pin]
	inner: T::ListenerUpgrade,
	map: Option<F>,
}

impl<T, F, TErr> Future for MapErrListenerUpgrade<T, F>
where
	T: Transport,
	F: FnOnce(T::Error) -> TErr,
{
	type Output = Result<T::Output, TErr>;

	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		let this = self.project();
		match Future::poll(this.inner, cx) {
			Poll::Ready(Ok(value)) => Poll::Ready(Ok(value)),
			Poll::Pending => Poll::Pending,
			Poll::Ready(Err(err)) => {
				let map = this.map.take().expect("poll() called again after error");
				Poll::Ready(Err(map(err)))
			}
		}
	}
}

/// Dialing future for `MapErr`.
#[pin_project::pin_project]
pub struct MapErrDial<T: Transport, F> {
	#[pin]
	inner: T::Dial,
	map: Option<F>,
}

impl<T, F, TErr> Future for MapErrDial<T, F>
where
	T: Transport,
	F: FnOnce(T::Error) -> TErr,
{
	type Output = Result<T::Output, TErr>;

	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		let this = self.project();
		match Future::poll(this.inner, cx) {
			Poll::Ready(Ok(value)) => Poll::Ready(Ok(value)),
			Poll::Pending => Poll::Pending,
			Poll::Ready(Err(err)) => {
				let map = this.map.take().expect("poll() called again after error");
				Poll::Ready(Err(map(err)))
			}
		}
	}
}
