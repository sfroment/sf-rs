use core::fmt;

/// Error type for parsing a PeerID from a string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParsePeerIDError {
    /// The input string had an incorrect length.
    InvalidLength { expected: usize, actual: usize },
    /// The input string contained non-hexadecimal characters or had an odd number of digits.
    InvalidHexEncoding { c: char, index: usize },
}

impl fmt::Display for ParsePeerIDError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParsePeerIDError::InvalidLength { expected, actual } => {
                write!(f, "Expected length: {}, but got: {}", expected, actual)
            }
            ParsePeerIDError::InvalidHexEncoding { c, index } => {
                write!(f, "Invalid hex character '{}' at index {}", c, index)
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ParsePeerIDError {}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn test_parse_id_error_display() {
        let error = ParsePeerIDError::InvalidLength {
            expected: 16,
            actual: 20,
        };
        assert_eq!(format!("{}", error), "Expected length: 16, but got: 20");

        let error = ParsePeerIDError::InvalidHexEncoding { c: 'g', index: 5 };
        assert_eq!(format!("{}", error), "Invalid hex character 'g' at index 5");
    }
}
