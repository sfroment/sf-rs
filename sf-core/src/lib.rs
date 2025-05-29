mod connection;
pub mod muxing;
mod protocol;
mod stream;
pub mod transport;

pub use muxing::StreamMuxer;
pub use protocol::*;
pub use transport::Transport;

pub mod util {
	use std::convert::Infallible;

	/// A safe version of [`std::intrinsics::unreachable`].
	#[inline(always)]
	pub fn unreachable(x: Infallible) -> ! {
		match x {}
	}
}
