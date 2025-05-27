use std::{future::Future, pin::Pin};

use futures::{
	AsyncRead, AsyncWrite,
	task::{Context, Poll},
};
use multiaddr::Multiaddr;

pub use self::boxed::{StreamMuxerBox, SubstreamBox};

mod boxed;

/// Provides multiplexing for a connection by allowing users to open substreams.
///
/// A substream created by a [`StreamMuxer`] is a type that implements [`AsyncRead`] and
/// [`AsyncWrite`]. The [`StreamMuxer`] itself is modelled closely after [`AsyncWrite`]. It features
/// `poll`-style functions that allow the implementation to make progress on various tasks.
pub trait StreamMuxer {
	/// Type of the object that represents the raw substream where data can be read and written.
	type Substream: AsyncRead + AsyncWrite;

	/// Error type of the muxer
	type Error: std::error::Error;

	/// Poll for new inbound substreams.
	///
	/// This function should be called whenever callers are ready to accept more inbound streams. In
	/// other words, callers may exercise back-pressure on incoming streams by not calling this
	/// function if a certain limit is hit.
	fn poll_inbound(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<Self::Substream, Self::Error>>;

	/// Poll for a new, outbound substream.
	fn poll_outbound(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<Self::Substream, Self::Error>>;

	/// Poll to close this [`StreamMuxer`].
	///
	/// After this has returned `Poll::Ready(Ok(()))`, the muxer has become useless and may be
	/// safely dropped.
	///
	/// > **Note**: You are encouraged to call this method and wait for it to return `Ready`, so
	/// > that the remote is properly informed of the shutdown. However, apart from
	/// > properly informing the remote, there is no difference between this and
	/// > immediately dropping the muxer.
	fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>>;

	/// Poll to allow the underlying connection to make progress.
	///
	/// In contrast to all other `poll`-functions on [`StreamMuxer`], this function MUST be called
	/// unconditionally. Because it will be called regardless, this function can be used by
	/// implementations to return events about the underlying connection that the caller MUST deal
	/// with.
	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<StreamMuxerEvent, Self::Error>>;
}

/// An event produced by a [`StreamMuxer`].
#[derive(Debug)]
pub enum StreamMuxerEvent {
	/// The address of the remote has changed.
	AddressChange(Multiaddr),
}

/// Extension trait for [`StreamMuxer`].
pub trait StreamMuxerExt: StreamMuxer + Sized {
	/// Convenience function for calling [`StreamMuxer::poll_inbound`]
	/// for [`StreamMuxer`]s that are `Unpin`.
	fn poll_inbound_unpin(&mut self, cx: &mut Context<'_>) -> Poll<Result<Self::Substream, Self::Error>>
	where
		Self: Unpin,
	{
		Pin::new(self).poll_inbound(cx)
	}

	/// Convenience function for calling [`StreamMuxer::poll_outbound`]
	/// for [`StreamMuxer`]s that are `Unpin`.
	fn poll_outbound_unpin(&mut self, cx: &mut Context<'_>) -> Poll<Result<Self::Substream, Self::Error>>
	where
		Self: Unpin,
	{
		Pin::new(self).poll_outbound(cx)
	}

	/// Convenience function for calling [`StreamMuxer::poll`]
	/// for [`StreamMuxer`]s that are `Unpin`.
	fn poll_unpin(&mut self, cx: &mut Context<'_>) -> Poll<Result<StreamMuxerEvent, Self::Error>>
	where
		Self: Unpin,
	{
		Pin::new(self).poll(cx)
	}

	/// Convenience function for calling [`StreamMuxer::poll_close`]
	/// for [`StreamMuxer`]s that are `Unpin`.
	fn poll_close_unpin(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>>
	where
		Self: Unpin,
	{
		Pin::new(self).poll_close(cx)
	}

	/// Returns a future for closing this [`StreamMuxer`].
	fn close(self) -> Close<Self> {
		Close(self)
	}
}

impl<S> StreamMuxerExt for S where S: StreamMuxer {}

pub struct Close<S>(S);

impl<S> Future for Close<S>
where
	S: StreamMuxer + Unpin,
{
	type Output = Result<(), S::Error>;

	fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		self.0.poll_close_unpin(cx)
	}
}
