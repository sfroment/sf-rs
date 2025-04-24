use core::{mem, slice};

use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{self, Error, SeqAccess, Visitor},
    ser,
};

use crate::FixedSizePeerID;

#[repr(C, packed)]
struct PeerIDBuffer<const S: usize> {
    size_repr: u8,
    bytes_repr: [u8; S],
}

impl<const S: usize> PeerIDBuffer<S> {
    fn new() -> Self {
        Self {
            size_repr: 0,
            bytes_repr: [0; S],
        }
    }

    fn as_slice(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self as *const _ as *const u8, mem::size_of::<Self>()) }
    }
    fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { slice::from_raw_parts_mut(self as *mut _ as *mut u8, mem::size_of::<Self>()) }
    }
}

impl<const SIZE: usize> Serialize for FixedSizePeerID<SIZE> {
    fn serialize<S>(&self, serialized: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut buffer = PeerIDBuffer::<SIZE>::new();
        let written = self
            .write(buffer.as_mut_slice())
            .map_err(|_| ser::Error::custom("Failed to serialize FixedSizePeerID"))?;

        serialized.serialize_bytes(&buffer.as_slice()[..written])
    }
}

struct PeerIDVisitor<const SIZE: usize>;

impl<'de, const SIZE: usize> Visitor<'de> for PeerIDVisitor<SIZE> {
    type Value = FixedSizePeerID<SIZE>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a FixedSizePeerID in bytes")
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: Error,
    {
        FixedSizePeerID::<SIZE>::from_bytes(v)
            .map_err(|_| de::Error::custom("Failed to deserialize FixedSizePeerID"))
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut buffer = PeerIDBuffer::<SIZE>::new();
        let bytes = buffer.as_mut_slice();

        let mut i = 0;
        let len = bytes.len();
        while let Some(byte) = seq.next_element()? {
            if i >= len {
                return Err(de::Error::custom(
                    "Failed to deserialize too many bytes for FixedSizePeerID",
                ));
            }
            bytes[i] = byte;
            i += 1;
        }

        FixedSizePeerID::<SIZE>::from_bytes(&bytes[..i])
            .map_err(|_| de::Error::custom("Failed to deserialize FixedSizePeerID"))
    }
}

impl<'de, const SIZE: usize> Deserialize<'de> for FixedSizePeerID<SIZE> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_bytes(PeerIDVisitor::<SIZE>)
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use std::str::FromStr;

    use serde_json::json;

    use super::*;

    #[test]
    fn test_peer_id_serde() {
        let size: u8 = 4;
        let expected_json = format!("[{size},222,173,190,239]");
        let peer_id = FixedSizePeerID::<4>::from_str("deadbeef").unwrap();

        let json = serde_json::to_string(&peer_id).unwrap();
        assert_eq!(json, expected_json);

        let deserialized: FixedSizePeerID<4> = serde_json::from_str(&json).unwrap();
        assert_eq!(peer_id, deserialized);
    }

    #[test]
    fn test_serde_visit_seq_too_many_bytes_error() {
        let too_long_json_array = r#"[1, 2, 3, 4, 5, 6]"#;
        let result: Result<FixedSizePeerID<4>, _> = serde_json::from_str(too_long_json_array);

        assert!(result.is_err());
        let error_message = result.unwrap_err().to_string();

        assert!(
            error_message.contains("Failed to deserialize too many bytes for FixedSizePeerID"),
            "Error message did not contain the expected 'too many bytes' error. Got: {}",
            error_message
        );
    }

    #[test]
    fn test_serde_expecting() {
        let res = serde_json::from_value::<FixedSizePeerID<1>>(json!(null)).unwrap_err();
        assert_eq!(
            res.to_string(),
            "invalid type: null, expected a FixedSizePeerID in bytes"
        );
    }

    #[test]
    fn test_serde_visit_bytes() {
        const PEER_ID_SIZE: usize = 4;

        let peer_id =
            FixedSizePeerID::<PEER_ID_SIZE>::from_bytes(&[4, 100, 101, 102, 103]).unwrap();

        let options = bincode::config::standard()
            .with_little_endian()
            .with_fixed_int_encoding();

        let mut slice = [0u8; 13];
        let res = bincode::serde::encode_into_slice(peer_id, &mut slice, options).unwrap();
        println!("res: {} {:?}", res, slice);

        let decoded: FixedSizePeerID<PEER_ID_SIZE> =
            bincode::serde::decode_from_slice(&slice, options)
                .unwrap()
                .0;

        assert_eq!(peer_id, decoded);
    }
}
