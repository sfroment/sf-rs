use super::{Error, Listener, WtTransport};
use multiaddr::Multiaddr;

pub fn listen_on(_: &WtTransport, _: Multiaddr) -> Result<Listener, Error> {
	unreachable!("listen_on is not supported on wasm32");
}
