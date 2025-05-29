use std::convert::Infallible;

use futures::{
	SinkExt, StreamExt,
	channel::{mpsc, oneshot},
	future::{Either, Future, poll_fn},
};
use multiaddr::PeerId;
use sf_core::{muxing::StreamMuxerBox, transport::TransportError};

use crate::peer::{PendingInboundConnectionError, PendingOutboundConnectionError};

pub(crate) enum PendingPeerEvent {
	ConnectionEstablished {
		output: (PeerId, StreamMuxerBox),
	},
	PendingFailed {
		error: Either<PendingOutboundConnectionError, PendingInboundConnectionError>,
	},
}

pub(crate) enum PeerEvent {}

pub(crate) async fn new_pending_peer<TFut>(
	future: TFut,
	abort_receiver: oneshot::Receiver<Infallible>,
	mut events: mpsc::Sender<PendingPeerEvent>,
) where
	TFut: Future<Output = Result<(PeerId, StreamMuxerBox), std::io::Error>> + Send + 'static,
{
	match futures::future::select(abort_receiver, Box::pin(future)).await {
		Either::Left((Err(oneshot::Canceled), _)) => {
			let _ = events
				.send(PendingPeerEvent::PendingFailed {
					error: Either::Right(PendingInboundConnectionError::Aborted),
				})
				.await;
		}
		Either::Left((Ok(v), _)) => sf_core::util::unreachable(v),
		Either::Right((Ok(output), _)) => {
			let _ = events.send(PendingPeerEvent::ConnectionEstablished { output }).await;
		}
		Either::Right((Err(e), _)) => {
			let _ = events
				.send(PendingPeerEvent::PendingFailed {
					error: Either::Right(PendingInboundConnectionError::Transport(TransportError::Other(e))),
				})
				.await;
		}
	}
}
