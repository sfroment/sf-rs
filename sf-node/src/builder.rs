use std::{collections::HashMap, error::Error, sync::Arc};

use multiaddr::Multiaddr;
use sf_core::{Connection, Listener, P2PProtocol, Stream, Transport};

type BoxedError = Box<dyn Error + Send + Sync + 'static>;
type BoxedStream = Box<dyn Stream<Error = BoxedError>>;
type BoxedConnection =
	Box<dyn Connection<Error = BoxedError, Stream = BoxedStream, CloseReturn = BoxedError, StreamReturn = BoxedStream>>;
type BoxedListener = Box<
	dyn Listener<
			Connection = BoxedConnection,
			Error = BoxedError,
			Item = Result<(BoxedConnection, Multiaddr), BoxedError>,
		>,
>;
type BoxedTransportError = Box<dyn Error + Send + Sync + 'static>;
type DynTransportObject = Arc<
	dyn Transport<
			Connection = BoxedConnection,
			Listener = BoxedListener,
			Error = BoxedTransportError,
			DialReturn = BoxedError,
			ListenReturn = BoxedError,
		>,
>;

pub struct Builder {
	keypair: libp2p_identity::Keypair,

	transports: HashMap<P2PProtocol, Box<DynTransportObject>>,
}

impl Builder {
	pub fn new(keypair: libp2p_identity::Keypair) -> Self {
		Self {
			keypair,
			transports: HashMap::new(),
		}
	}

	pub fn add_transport(
		&mut self,
		transport: impl Transport<
			Connection = BoxedConnection,
			Listener = BoxedListener,
			Error = BoxedTransportError,
			DialReturn = BoxedError,
			ListenReturn = BoxedError,
		>,
	) {
		self.transports.insert(
			transport.supported_protocols_for_dialing(),
			Box::new(Arc::new(transport)),
		);
	}

	pub fn build(self) -> Result<Node, BoxedError> {
		Ok(Node::new(self.keypair, self.transports))
	}
}
