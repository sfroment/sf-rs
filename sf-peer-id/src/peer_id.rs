use std::{
    fmt,
    hash::{self, Hash},
    str::FromStr,
};

use crate::{ParsePeerIDError, hex_char_to_value};

/// A PeerID instance that allow you to identifies peer within the network
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PeerID<const S: usize> {
    /// The actual size of the PeerID
    size: u8,
    /// The bytes of the identifiers
    bytes: [u8; S],
}

impl<const S: usize> PeerID<S> {
    /// Creates a new PeerID from a byte array of length S
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sf-peer::PeerID;
    /// let id = PeerID::<16>::new([0; 16]);
    /// ```
    pub fn from_bytes(data: &[u8]) -> Result<Self, ParsePeerIDError> {
        let size = data.len();
        if size > S {
            return Err(ParsePeerIDError::InvalidLength {
                expected: S,
                actual: size,
            });
        }
        let mut bytes = [0; S];
        let mut i = 0;
        while i < S && i < size {
            bytes[i] = data[i];
            i += 1;
        }
        Ok(Self {
            size: size as u8,
            bytes,
        })
    }

    /// Returns a reference to the bytes of the PeerID
    pub fn as_bytes(&self) -> &[u8; S] {
        &self.bytes
    }

    /// Returns a mutable reference to the bytes of the PeerID
    pub fn as_bytes_mut(&mut self) -> &mut [u8; S] {
        &mut self.bytes
    }

    /// Return the size of the PeerID
    pub fn size(&self) -> usize {
        self.size as usize
    }

    // Return a zeroed PeerID
    pub const fn zeroed() -> Self {
        Self {
            size: S as u8,
            bytes: [0; S],
        }
    }
}

#[cfg(feature = "std")]
impl<const S: usize> FromStr for PeerID<S> {
    type Err = ParsePeerIDError;

    /// Parses a PeerID from a hexadecimal string.
    ///
    /// The string must be of length `2 * S` and contain only hexadecimal characters.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sf-peer::PeerID;
    /// let id = PeerID::<16>::from_str("deadbeefdeadbeefdeadbeefdeadbeef").unwrap();
    /// ```
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let expected_len = 2 * S;
        let current_len = s.len();
        if current_len != expected_len {
            return Err(ParsePeerIDError::InvalidLength {
                expected: expected_len,
                actual: current_len,
            });
        }

        let mut bytes = [0; S];
        for (i, chars) in s.as_bytes().chunks(2).enumerate() {
            let index = i * 2;
            let high_val = hex_char_to_value(chars[0] as char, index)?;
            let low_val = hex_char_to_value(chars[1] as char, index + 1)?;

            bytes[i] = (high_val << 4) | low_val;
        }

        Ok(Self {
            size: (current_len / 2) as u8,
            bytes,
        })
    }
}

#[cfg(feature = "std")]
impl<const S: usize> Hash for PeerID<S> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.bytes.hash(state);
    }
}

#[cfg(feature = "std")]
impl<const S: usize> fmt::Debug for PeerID<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PeerID<{}>", S)?;
        f.write_str("(")?;
        for byte in self.bytes {
            write!(f, "{:02x}", byte)?;
        }
        f.write_str(")")?;
        Ok(())
    }
}

#[cfg(feature = "std")]
impl<const S: usize> fmt::Display for PeerID<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in &self.bytes {
            write!(f, "{:02x}", byte)?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn test_peer_id_creation() {
        let id = PeerID::<16>::zeroed();
        assert_eq!(id.as_bytes(), &[0; 16]);
    }

    #[test]
    fn test_peer_id_equality() {
        let id1 = PeerID::<16>::zeroed();
        let id2 = PeerID::<16>::zeroed();
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_peer_id_inequality() {
        let id1 = PeerID::<16>::zeroed();
        let id2 = PeerID::<16>::from_bytes(&[1; 16]).unwrap();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_peer_id_partial_ordering() {
        let id1 = PeerID::<16>::zeroed();
        let id2 = PeerID::<16>::from_bytes(&[1; 16]).unwrap();
        assert!(id1 < id2);
    }

    #[test]
    fn test_peer_id_clone() {
        let id1 = PeerID::<16>::zeroed();
        assert_eq!(id1, id1.clone());
    }

    #[test]
    fn test_peer_id_copy() {
        let id1 = PeerID::<16>::zeroed();
        let id2 = id1;
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_peer_id_from_str() {
        let id = PeerID::<16>::from_str("deadbeefdeadbeefdeadbeefdeadbeef").unwrap();
        assert_eq!(
            id.as_bytes(),
            &[
                222, 173, 190, 239, 222, 173, 190, 239, 222, 173, 190, 239, 222, 173, 190, 239
            ]
        );
    }

    #[test]
    fn test_peer_id_from_str_invalid_length() {
        let result = PeerID::<16>::from_str("deadbeefdeadbeef");
        assert!(result.is_err());
        if let Err(ParsePeerIDError::InvalidLength { expected, actual }) = result {
            assert_eq!(expected, 32);
            assert_eq!(actual, 16);
        } else {
            panic!("Expected InvalidLength error");
        }
    }

    #[test]
    fn test_peer_id_from_str_invalid_hex() {
        let result = PeerID::<16>::from_str("deadbeefdeadbeefdeadbeefdeadbefg");
        assert!(result.is_err());
        println!("r {}", result.unwrap_err());
        if let Err(ParsePeerIDError::InvalidHexEncoding { c, index }) = result {
            assert_eq!(c, 'g');
            assert_eq!(index, 31);
        } else {
            panic!("Expected InvalidHexEncoding error");
        }
    }

    #[test]
    fn test_peer_id_hash() {
        use std::{collections::hash_map::DefaultHasher, hash::Hasher};

        let id1 = PeerID::<16>::zeroed();
        let id2 = PeerID::<16>::zeroed();

        let mut hasher1 = DefaultHasher::new();
        id1.hash(&mut hasher1);
        let hash1 = hasher1.finish();

        let mut hasher2 = DefaultHasher::new();
        id2.hash(&mut hasher2);
        let hash2 = hasher2.finish();

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_peer_id_debug() {
        let id = PeerID::<16>::from_bytes(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16])
            .unwrap();
        assert_eq!(
            format!("{:?}", id),
            "PeerID<16>(0102030405060708090a0b0c0d0e0f10)"
        );
    }

    #[test]
    fn test_peer_id_display() {
        let id = PeerID::<16>::from_bytes(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16])
            .unwrap();
        assert_eq!(format!("{}", id), "0102030405060708090a0b0c0d0e0f10");
    }

    #[test]
    fn test_peer_id_size() {
        let id = PeerID::<16>::zeroed();
        assert_eq!(id.size(), 16);
    }

    #[test]
    fn test_peer_id_as_bytes_mut() {
        let mut id = PeerID::<16>::zeroed();
        let bytes = id.as_bytes_mut();
        bytes[0] = 1;
        assert_eq!(
            id.as_bytes(),
            &[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );
    }

    #[test]
    fn test_peer_id_from_bytes_invalid_length() {
        let result = PeerID::<2>::from_bytes(&[1, 2, 3]);
        assert!(result.is_err());
        if let Err(ParsePeerIDError::InvalidLength { expected, actual }) = result {
            assert_eq!(expected, 2);
            assert_eq!(actual, 3);
        } else {
            panic!("Expected InvalidLength error");
        }
    }
}
