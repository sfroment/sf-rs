// Copyright 2017 Parity Technologies (UK) Ltd.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the "Software"),
// to deal in the Software without restriction, including without limitation
// the rights to use, copy, modify, merge, publish, distribute, sublicense,
// and/or sell copies of the Software, and to permit persons to whom the
// Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

use std::{
	pin::Pin,
	task::{Context, Poll},
};

use futures::prelude::*;
use multiaddr::Multiaddr;

use crate::{
	Protocol,
	transport::{Transport, TransportError, TransportEvent},
};

/// See `Transport::map`.
#[derive(Debug, Copy, Clone)]
#[pin_project::pin_project]
pub struct Map<T, F> {
	#[pin]
	transport: T,
	fun: F,
}

impl<T, F> Map<T, F> {
	pub(crate) fn new(transport: T, fun: F) -> Self {
		Map { transport, fun }
	}

	pub fn inner(&self) -> &T {
		&self.transport
	}

	pub fn inner_mut(&mut self) -> &mut T {
		&mut self.transport
	}
}

impl<T, F, D> Transport for Map<T, F>
where
	T: Transport,
	F: FnOnce(T::Output) -> D + Clone,
{
	type Output = D;
	type Error = T::Error;
	type ListenerUpgrade = MapFuture<T::ListenerUpgrade, F>;
	type Dial = MapFuture<T::Dial, F>;

	fn supported_protocols_for_dialing(&self) -> Protocol {
		self.transport.supported_protocols_for_dialing()
	}

	fn listen_on(&mut self, addr: Multiaddr) -> Result<(), TransportError<Self::Error>> {
		self.transport.listen_on(addr)
	}

	fn dial(&mut self, addr: Multiaddr) -> Result<Self::Dial, TransportError<Self::Error>> {
		let future = self.transport.dial(addr.clone())?;
		Ok(MapFuture {
			inner: future,
			args: Some(self.fun.clone()),
		})
	}

	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<TransportEvent<Self::ListenerUpgrade, Self::Error>> {
		let this = self.project();
		match this.transport.poll(cx) {
			Poll::Ready(TransportEvent::Incoming {
				upgrade,
				local_addr,
				remote_addr,
			}) => Poll::Ready(TransportEvent::Incoming {
				upgrade: MapFuture {
					inner: upgrade,
					args: Some(this.fun.clone()),
				},
				local_addr,
				remote_addr,
			}),
			Poll::Ready(other) => {
				let mapped = other.map_upgrade(|_upgrade| unreachable!("case already matched"));
				Poll::Ready(mapped)
			}
			Poll::Pending => Poll::Pending,
		}
	}
}

/// Custom `Future` to avoid boxing.
///
/// Applies a function to the inner future's result.
#[pin_project::pin_project]
#[derive(Clone, Debug)]
pub struct MapFuture<T, F> {
	#[pin]
	inner: T,
	args: Option<F>,
}

impl<T, A, F, B> Future for MapFuture<T, F>
where
	T: TryFuture<Ok = A>,
	F: FnOnce(A) -> B,
{
	type Output = Result<B, T::Error>;

	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		let this = self.project();
		let item = match TryFuture::try_poll(this.inner, cx) {
			Poll::Pending => return Poll::Pending,
			Poll::Ready(Ok(v)) => v,
			Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
		};
		let f = this.args.take().expect("MapFuture has already finished.");
		Poll::Ready(Ok(f(item)))
	}
}
