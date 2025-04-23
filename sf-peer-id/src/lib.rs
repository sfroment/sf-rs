#![feature(coverage_attribute)]
#![deny(warnings)]

mod error;
mod hex;
mod peer_id;

pub(crate) use crate::hex::hex_char_to_value;

pub use crate::{error::ParsePeerIDError, peer_id::PeerID};
