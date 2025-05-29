use futures::{
	SinkExt, StreamExt,
	channel::{mpsc, oneshot},
	future::{BoxFuture, Either, poll_fn},
	prelude::*,
	ready,
	stream::{FuturesUnordered, SelectAll},
};
use multiaddr::{Multiaddr, PeerId};
use sf_core::muxing::{StreamMuxerBox, StreamMuxerExt};
use std::{
	convert::Infallible,
	task::{Context, Poll},
};
use tracing::Instrument;
use web_time::{Duration, Instant};

use crate::{
	executor::{Executor, get_executor},
	peer::task,
	peer::{PendingInboundConnectionError, PendingOutboundConnectionError},
};

struct TaskExecutor(Box<dyn Executor + Send>);

impl TaskExecutor {
	pub fn new() -> Self {
		Self(Box::new(get_executor()))
	}

	#[track_caller]
	pub fn spawn(&self, future: impl Future<Output = ()> + Send + 'static) {
		let future = future.boxed();

		self.0.exec(future);
	}
}

pub struct Manager {
	pending_peer_events_tx: mpsc::Sender<task::PendingPeerEvent>,
	pending_peer_events_rx: mpsc::Receiver<task::PendingPeerEvent>,
	new_peer_dropped_listeners: FuturesUnordered<oneshot::Receiver<StreamMuxerBox>>,
	peer_events: SelectAll<mpsc::Receiver<task::PeerEvent>>,
	task_executor: TaskExecutor,
}

impl Manager {
	pub fn new() -> Self {
		let (pending_peer_events_tx, pending_peer_events_rx) = mpsc::channel(0);

		Self {
			pending_peer_events_tx,
			pending_peer_events_rx,
			new_peer_dropped_listeners: Default::default(),
			peer_events: Default::default(),
			task_executor: TaskExecutor::new(),
		}
	}

	pub(crate) fn add_incoming<TFut>(&mut self, fut: TFut, local_addr: Multiaddr, remote_addr: Multiaddr)
	where
		TFut: Future<Output = Result<(PeerId, StreamMuxerBox), std::io::Error>> + Send + 'static,
	{
		let (abort_notifier, abort_receiver) = oneshot::channel();

		let span = tracing::debug_span!(parent: tracing::Span::none(), "new_incoming_connection", remote_addr = %remote_addr, id = %local_addr);
		span.follows_from(tracing::Span::current());

		self.task_executor
			.spawn(task::new_pending_peer(fut, abort_receiver, self.pending_peer_events_tx.clone()).instrument(span));
	}

	pub(crate) fn add_outgoing<TFut>(&mut self, fut: TFut, local_addr: Multiaddr, remote_addr: Multiaddr)
	where
		TFut: Future<Output = Result<(PeerId, StreamMuxerBox), std::io::Error>> + Send + 'static,
	{
		let (abort_notifier, abort_receiver) = oneshot::channel();

		let span = tracing::debug_span!(parent: tracing::Span::none(), "new_outgoing_connection", remote_addr = %remote_addr, id = %local_addr);
		span.follows_from(tracing::Span::current());

		self.task_executor
			.spawn(task::new_pending_peer(fut, abort_receiver, self.pending_peer_events_tx.clone()).instrument(span));
	}

	pub(crate) fn poll(&mut self, cx: &mut Context<'_>) -> Poll<PeerEvent> {
		match self.peer_events.poll_next_unpin(cx) {
			Poll::Pending => {}
			Poll::Ready(None) => {
				todo!()
			}
			Poll::Ready(Some(event)) => {
				return self.handle_peer_event(event);
			}
		}

		loop {
			if let Poll::Ready(Some(event)) = self.new_peer_dropped_listeners.poll_next_unpin(cx) {
				if let Ok(connection) = event {
					self.task_executor.spawn(async move {
						if let Err(e) = connection.close().await {
							tracing::error!(?e, "Failed to close connection");
						}
					});
				}
				continue;
			}

			let event = match self.pending_peer_events_rx.poll_next_unpin(cx) {
				Poll::Ready(Some(event)) => event,
				Poll::Ready(None) => unreachable!("Shouldn't be reachable"),
				Poll::Pending => break,
			};

			return self.handle_pending_peer_event(event);
		}

		//self.executor.advance_local(cx);

		Poll::Pending
	}

	#[inline]
	fn handle_peer_event(&mut self, event: task::PeerEvent) -> Poll<PeerEvent> {
		todo!()
	}

	#[inline]
	fn handle_pending_peer_event(&mut self, event: task::PendingPeerEvent) -> Poll<PeerEvent> {
		match event {
			task::PendingPeerEvent::ConnectionEstablished { output } => {
				self.handle_pending_peer_event_connection_established(output)
			}
			task::PendingPeerEvent::PendingFailed { error } => self.handle_pending_peer_event_pending_failed(error),
		}
	}

	#[inline]
	fn handle_pending_peer_event_connection_established(
		&mut self,
		output: (PeerId, StreamMuxerBox),
	) -> Poll<PeerEvent> {
		let (peer_id, stream_muxer_box) = output;

		Poll::Ready(PeerEvent::ConnectionEstablished {
			peer_id,
			stream_muxer_box,
		})
	}

	#[inline]
	fn handle_pending_peer_event_pending_failed(
		&mut self,
		error: Either<PendingOutboundConnectionError, PendingInboundConnectionError>,
	) -> Poll<PeerEvent> {
		match error {
			Either::Left(error) => Poll::Ready(PeerEvent::PendingOutboundConnectionError(error)),
			Either::Right(error) => Poll::Ready(PeerEvent::PendingInboundConnectionError(error)),
		}
	}
}

pub(crate) enum PeerEvent {
	PendingOutboundConnectionError(PendingOutboundConnectionError),
	PendingInboundConnectionError(PendingInboundConnectionError),

	ConnectionEstablished {
		peer_id: PeerId,
		stream_muxer_box: StreamMuxerBox,
	},
}

struct PendingPeer {
	/// When dropped, notifies the task which then knows to terminate.
	abort_notifier: Option<oneshot::Sender<Infallible>>,
	/// The moment we became aware of this possible connection, useful for timing metrics.
	accepted_at: Instant,
}

impl PendingPeer {
	/// Aborts the connection attempt, closing the connection.
	fn abort(&mut self) {
		if let Some(notifier) = self.abort_notifier.take() {
			drop(notifier);
		}
	}
}
