use core::fmt;
use unsigned_varint::decode;

#[cfg(not(feature = "std"))]
use core2::io;
#[cfg(feature = "std")]
use std::io;

/// Error type for parsing a PeerID from a string.
#[derive(Debug)]
pub enum Error {
    /// The input string had an incorrect length.
    InvalidLength { expected: usize, actual: usize },
    /// The input string contained non-hexadecimal characters or had an odd number of digits.
    InvalidHexEncoding { c: char, index: usize },
    /// Io error
    Io(io::Error),
    /// Varint error
    Varint(decode::Error),

    /// Getrandom error
    #[cfg(all(feature = "wasm", target_arch = "wasm32"))]
    Getrandom(getrandom::Error),

    // Serde error
    #[cfg(all(feature = "wasm", target_arch = "wasm32"))]
    Serde(serde_wasm_bindgen::Error),
}

impl PartialEq for Error {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Error::InvalidLength { expected, actual },
                Error::InvalidLength {
                    expected: e,
                    actual: a,
                },
            ) => expected == e && actual == a,
            (
                Error::InvalidHexEncoding { c, index },
                Error::InvalidHexEncoding { c: cc, index: i },
            ) => c == cc && index == i,
            (Error::Io(err), Error::Io(other_err)) => err.kind() == other_err.kind(),
            (Error::Varint(err), Error::Varint(other_err)) => err == other_err,
            #[cfg(all(feature = "wasm", target_arch = "wasm32"))]
            (Error::Getrandom(err), Error::Getrandom(other_err)) => err == other_err,
            _ => false,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidLength { expected, actual } => {
                write!(f, "Expected length: {expected}, but got: {actual}")
            }
            Error::InvalidHexEncoding { c, index } => {
                write!(f, "Invalid hex character '{c}' at index {index}")
            }
            Error::Io(err) => {
                write!(f, "IO error: {err}")
            }
            Error::Varint(err) => {
                write!(f, "Varint error: {err}")
            }
            #[cfg(all(feature = "wasm", target_arch = "wasm32"))]
            Error::Getrandom(err) => {
                write!(f, "Getrandom error: {err}")
            }
            #[cfg(all(feature = "wasm", target_arch = "wasm32"))]
            Error::Serde(err) => {
                write!(f, "Serde error: {err}")
            }
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::Io(err)
    }
}

impl From<decode::Error> for Error {
    fn from(err: decode::Error) -> Self {
        Error::Varint(err)
    }
}

#[cfg(all(feature = "wasm", target_arch = "wasm32"))]
impl From<getrandom::Error> for Error {
    fn from(err: getrandom::Error) -> Self {
        Error::Getrandom(err)
    }
}

#[cfg(feature = "std")]
impl From<unsigned_varint::io::ReadError> for Error {
    fn from(err: unsigned_varint::io::ReadError) -> Self {
        match err {
            unsigned_varint::io::ReadError::Io(e) => Error::Io(e),
            unsigned_varint::io::ReadError::Decode(e) => Error::Varint(e),
            _ => {
                // This case should not happen, but if it does, we can convert it to a Varint error.
                Error::Varint(decode::Error::Insufficient)
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn test_parse_id_error_display() {
        let error = Error::InvalidLength {
            expected: 16,
            actual: 20,
        };
        assert_eq!(format!("{error}"), "Expected length: 16, but got: 20");

        let error = Error::InvalidHexEncoding { c: 'g', index: 5 };
        assert_eq!(format!("{error}"), "Invalid hex character 'g' at index 5");

        let io_error = io::Error::other("IO error");
        let error = Error::Io(io_error);
        assert_eq!(format!("{error}"), "IO error: IO error");

        let varint_error = decode::Error::Insufficient;
        let error = Error::Varint(varint_error);
        assert_eq!(format!("{error}"), "Varint error: not enough input bytes");
    }

    #[test]
    fn test_partial_eq() {
        let error1 = Error::InvalidLength {
            expected: 16,
            actual: 20,
        };
        let error2 = Error::InvalidLength {
            expected: 16,
            actual: 20,
        };
        let error2_bis = Error::InvalidLength {
            expected: 16,
            actual: 21,
        };
        let error2_ter = Error::InvalidLength {
            expected: 17,
            actual: 20,
        };
        assert_eq!(error1, error2);
        assert_ne!(error1, error2_bis);
        assert_ne!(error1, error2_ter);

        let error3 = Error::InvalidHexEncoding { c: 'g', index: 5 };
        let error3_ter = Error::InvalidHexEncoding { c: 'g', index: 5 };
        let error3_bis = Error::InvalidHexEncoding { c: 'g', index: 6 };
        let error3_bis_bis = Error::InvalidHexEncoding { c: 'e', index: 6 };
        assert_ne!(error1, error3);
        assert_ne!(error3, error3_bis);
        assert_eq!(error3, error3_ter);
        assert_ne!(error3, error3_bis_bis);

        let io_error = io::Error::other("IO error");
        let error4 = Error::Io(io_error);
        assert_ne!(error1, error4);

        let varint_error = decode::Error::Insufficient;
        let error5 = Error::Varint(varint_error);
        assert_ne!(error1, error5);

        let error6 = Error::Varint(decode::Error::Insufficient);
        assert_eq!(error5, error6);

        let io_error = io::Error::other("IO error");
        let error7 = Error::Io(io_error);
        assert_eq!(error4, error7);

        assert!(error4 != error5);
    }

    #[test]
    fn test_error_from_io() {
        let err = io::Error::other("IO error");
        let error = Error::from(err);
        assert_eq!(error, Error::Io(io::Error::other("IO error")));
    }

    #[test]
    fn test_error_from_varint() {
        let varint_error = decode::Error::Insufficient;
        let error: Error = Error::from(varint_error);
        assert_eq!(error, Error::Varint(decode::Error::Insufficient));
    }

    #[test]
    fn test_error_from_unsigned_varint_io() {
        let read_error = unsigned_varint::io::ReadError::Io(io::Error::other("IO error"));
        let error: Error = Error::from(read_error);
        assert_eq!(error, Error::Io(io::Error::other("IO error")));

        let decode_error = unsigned_varint::io::ReadError::Decode(decode::Error::Insufficient);
        let error: Error = Error::from(decode_error);
        assert_eq!(error, Error::Varint(decode::Error::Insufficient));
    }
}
