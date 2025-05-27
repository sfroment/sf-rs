#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Protocol {
	WebTransport,
	WebRTC,
	Quic,
}
