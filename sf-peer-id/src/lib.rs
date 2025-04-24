#![feature(coverage_attribute)]
#![deny(warnings)]

mod error;
mod hex;
mod peer_id;
#[cfg(feature = "serde")]
mod serde;

pub(crate) use crate::hex::hex_char_to_value;

pub use crate::{
    error::Error,
    peer_id::{FixedSizePeerID, PeerID},
};
