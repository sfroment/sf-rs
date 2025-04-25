use unsigned_varint::encode;

use crate::{Error, hex_char_to_value};

#[cfg(feature = "std")]
use std::{
    fmt,
    hash::{self, Hash},
    io,
    str::FromStr,
};

pub type PeerID = FixedSizePeerID<32>;

/// A PeerID instance that allow you to identifies peer within the network
#[derive(Clone, Copy, PartialOrd, Ord)]
pub struct FixedSizePeerID<const S: usize> {
    /// The actual size of the PeerID
    size: u8,
    /// The bytes of the identifiers
    bytes: [u8; S],
}

impl<const S: usize> FixedSizePeerID<S> {
    /// Creates a new PeerID from a byte array of length S
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sf_peer_id::FixedSizePeerID;
    /// let id = FixedSizePeerID::<1>::from_bytes(&[1, 16]).unwrap();
    /// ```
    pub fn from_bytes(mut data: &[u8]) -> Result<Self, Error>
    where
        Self: Sized,
    {
        let len = data.len();
        let (size, bytes) = read_peer_id(&mut data)?;
        if !data.is_empty() {
            return Err(Error::InvalidLength {
                expected: S,
                actual: len,
            });
        }

        Ok(Self { size, bytes })
    }

    /// Returns a reference to the bytes of the PeerID
    pub fn as_bytes(&self) -> &[u8; S] {
        &self.bytes
    }

    /// Returns a mutable reference to the bytes of the PeerID
    pub fn as_bytes_mut(&mut self) -> &mut [u8; S] {
        &mut self.bytes
    }

    /// Return the bytes of struct
    pub fn bytes(&self) -> &[u8] {
        &self.bytes[..self.size as usize]
    }

    /// Return the size of the PeerID
    pub fn size(&self) -> u8 {
        self.size
    }

    /// Write a FixedSizePeerID to a byte stream, returning the number of bytes written
    pub fn write<W: io::Write>(&self, w: W) -> Result<usize, Error> {
        write_peer_id(w, self.size(), self.bytes())
    }

    // Return a zeroed PeerID
    pub const fn zeroed() -> Self {
        Self {
            size: S as u8,
            bytes: [0; S],
        }
    }

    #[cfg(feature = "std")]
    /// Create a random PeerID
    pub fn random() -> Result<Self, Error> {
        use std::time::{SystemTime, UNIX_EPOCH};

        let mut s = String::with_capacity(64);
        let mut seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        for _ in 0..64 {
            let nibble = (seed & 0xF) as u8;
            let hex_char = match nibble {
                0..=9 => (b'0' + nibble) as char,
                10..=15 => (b'a' + (nibble - 10)) as char,
                _ => unreachable!(),
            };
            s.push(hex_char);

            seed = seed.rotate_left(5) ^ (seed.wrapping_mul(31));
        }

        Self::from_str(&s)
    }
}

impl<const S: usize> PartialEq for FixedSizePeerID<S> {
    fn eq(&self, other: &Self) -> bool {
        if self.size != other.size {
            return false;
        }
        self.bytes[..self.size as usize] == other.bytes[..self.size as usize]
    }
}

impl<const S: usize> Eq for FixedSizePeerID<S> {}

fn write_peer_id<W>(mut w: W, size: u8, bytes: &[u8]) -> Result<usize, Error>
where
    W: io::Write,
{
    let mut size_buf = encode::u8_buffer();
    let size = encode::u8(size, &mut size_buf);

    let written = size.len() + bytes.len();

    w.write_all(size).map_err(Error::from)?;
    w.write_all(bytes).map_err(Error::from)?;
    Ok(written)
}

fn read_peer_id<R, const S: usize>(mut r: R) -> Result<(u8, [u8; S]), Error>
where
    R: io::Read,
{
    let size = read_u8(&mut r)?;
    if size > S as u8 {
        return Err(Error::InvalidLength {
            expected: S,
            actual: size as usize,
        });
    }

    let mut bytes = [0; S];
    r.read_exact(&mut bytes[..size as usize])
        .map_err(Error::from)?;
    Ok((size, bytes))
}

#[cfg(feature = "std")]
fn read_u8<R: io::Read>(r: R) -> Result<u8, Error> {
    unsigned_varint::io::read_u8(r).map_err(Error::from)
}

#[cfg(not(feature = "std"))]
fn read_u8<R>(mut r: R) -> Result<u8, Error>
where
    R: io::Read,
{
    use unsigned_varint::decode;

    let mut size_buf = encode::u8_buffer();
    for i in 0..size_buf.len() {
        let n = r.read(&mut size_buf[i..=i]).map_err(Error::from)?;
        if n == 0 {
            return Err(Error::Varint(decode::Error::Insufficient));
        } else if decode::is_last(size_buf[i]) {
            return decode::u8(&size_buf)
                .map(|decoded| decoded.0)
                .map_err(crate::Error::from);
        }
    }

    Err(Error::Varint(decode::Error::Overflow))
}

#[cfg(feature = "std")]
impl<const S: usize> FromStr for FixedSizePeerID<S> {
    type Err = Error;

    /// Parses a PeerID from a hexadecimal string.
    ///
    /// The string must be of length `2 * S` and contain only hexadecimal characters.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sf_peer_id::FixedSizePeerID;
    /// use std::str::FromStr;
    /// let id = FixedSizePeerID::<16>::from_str("deadbeefdeadbeefdeadbeefdeadbeef").unwrap();
    /// ```
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let expected_len = 2 * S;
        let current_len = s.len();
        if current_len > expected_len {
            return Err(Error::InvalidLength {
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
impl<const S: usize> Hash for FixedSizePeerID<S> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.size.hash(state);
        self.bytes[..self.size as usize].hash(state);
    }
}

#[cfg(feature = "std")]
impl<const S: usize> fmt::Debug for FixedSizePeerID<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PeerID<{S}>")?;
        f.write_str("(")?;
        for byte in &self.bytes[..self.size as usize] {
            write!(f, "{byte:02x}")?;
        }
        f.write_str(")")?;
        Ok(())
    }
}

#[cfg(feature = "std")]
impl<const S: usize> fmt::Display for FixedSizePeerID<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in &self.bytes[..self.size as usize] {
            write!(f, "{byte:02x}")?;
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
        let id = FixedSizePeerID::<16>::zeroed();
        assert_eq!(id.as_bytes(), &[0; 16]);
    }

    #[test]
    fn test_peer_id_equality() {
        let id1 = FixedSizePeerID::<16>::zeroed();
        let id2 = FixedSizePeerID::<16>::zeroed();
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_peer_id_inequality() {
        let id1 = FixedSizePeerID::<16>::zeroed();
        let mut arr = [0u8; 17];
        arr[0] = 16;
        arr[1] = 1;
        let id2 = FixedSizePeerID::<16>::from_bytes(&arr).unwrap();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_peer_id_partial_ordering() {
        let id1 = FixedSizePeerID::<16>::zeroed();
        let mut arr = [0u8; 17];
        arr[0] = 16;
        arr[1] = 1;
        let id2 = FixedSizePeerID::<16>::from_bytes(&arr).unwrap();
        assert!(id1 < id2);
    }

    #[test]
    fn test_peer_id_clone() {
        let id1 = FixedSizePeerID::<16>::zeroed();
        assert_eq!(id1, id1.clone());
    }

    #[test]
    fn test_peer_id_copy() {
        let id1 = FixedSizePeerID::<16>::zeroed();
        let id2 = id1;
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_peer_id_from_str() {
        let id = FixedSizePeerID::<16>::from_str("deadbeefdeadbeefdeadbeefdeadbeef").unwrap();
        assert_eq!(
            id.as_bytes(),
            &[
                222, 173, 190, 239, 222, 173, 190, 239, 222, 173, 190, 239, 222, 173, 190, 239
            ]
        );
    }

    #[test]
    fn test_peer_id_from_str_invalid_length() {
        let result =
            FixedSizePeerID::<16>::from_str("deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef");
        assert!(result.is_err());
        if let Err(Error::InvalidLength { expected, actual }) = result {
            assert_eq!(expected, 32);
            assert_eq!(actual, 48);
        } else {
            panic!("Expected InvalidLength error");
        }
    }

    #[test]
    fn test_peer_id_from_str_invalid_hex() {
        let result = FixedSizePeerID::<16>::from_str("deadbeefdeadbeefdeadbeefdeadbefg");
        assert!(result.is_err());
        if let Err(Error::InvalidHexEncoding { c, index }) = result {
            assert_eq!(c, 'g');
            assert_eq!(index, 31);
        } else {
            panic!("Expected InvalidHexEncoding error");
        }
    }

    #[test]
    fn test_peer_id_hash() {
        use std::{collections::hash_map::DefaultHasher, hash::Hasher};

        let id1 = FixedSizePeerID::<16>::zeroed();
        let id2 = FixedSizePeerID::<16>::zeroed();

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
        let id = FixedSizePeerID::<4>::from_str("deadbeef").unwrap();
        assert_eq!(format!("{id:?}"), "PeerID<4>(deadbeef)");
    }

    #[test]
    fn test_peer_id_display() {
        let id = FixedSizePeerID::<4>::from_str("deadbeef").unwrap();
        assert_eq!(format!("{id}"), "deadbeef");
    }

    #[test]
    fn test_peer_id_size() {
        let id = FixedSizePeerID::<16>::zeroed();
        assert_eq!(id.size(), 16);
    }

    #[test]
    fn test_peer_id_as_bytes_mut() {
        let mut id = FixedSizePeerID::<16>::zeroed();
        let bytes = id.as_bytes_mut();
        bytes[0] = 1;
        assert_eq!(
            id.as_bytes(),
            &[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );
    }

    #[test]
    fn test_peer_id_from_bytes_invalid_length() {
        let result = FixedSizePeerID::<3>::from_str("deadbeef");
        assert!(result.is_err());
        if let Err(Error::InvalidLength { expected, actual }) = result {
            assert_eq!(expected, 6);
            assert_eq!(actual, 8);
        } else {
            panic!("Expected InvalidLength error");
        }
    }

    #[test]
    fn test_too_long_peer_id() {
        let mut arr = [0u8; 2];
        arr[0] = 3;
        let result = FixedSizePeerID::<1>::from_bytes(&arr);
        assert!(result.is_err());
        if let Err(Error::InvalidLength { expected, actual }) = result {
            assert_eq!(expected, 1);
            assert_eq!(actual, 3);
        } else {
            panic!("Expected InvalidLength error");
        }
    }

    #[test]
    fn test_peer_id_from_bytes_trailing_data() {
        let input_data = &[10u8, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0xAA, 0xBB];

        let result = FixedSizePeerID::<16>::from_bytes(input_data);

        assert!(result.is_err(), "Expected an error due to trailing data");

        if let Err(Error::InvalidLength { expected, actual }) = result {
            assert_eq!(expected, 16, "Expected capacity S");
            assert_eq!(actual, 13, "Actual length of remaining data");
        } else {
            panic!("Expected InvalidLength error due to trailing data, got {result:?}",);
        }
    }
}
