use crate::Error;

#[inline]
pub(crate) fn hex_char_to_value(c: char, i: usize) -> Result<u8, Error> {
    match c {
        '0'..='9' => Ok((c as u8) - b'0'),
        'a'..='f' => Ok((c as u8) - b'a' + 10),
        'A'..='F' => Ok((c as u8) - b'A' + 10),
        _ => Err(Error::InvalidHexEncoding { c, index: i }),
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn test_hex_char_to_value() {
        assert_eq!(hex_char_to_value('0', 0), Ok(0));
        assert_eq!(hex_char_to_value('9', 0), Ok(9));
        assert_eq!(hex_char_to_value('a', 0), Ok(10));
        assert_eq!(hex_char_to_value('f', 0), Ok(15));
        assert_eq!(hex_char_to_value('A', 0), Ok(10));
        assert_eq!(hex_char_to_value('F', 0), Ok(15));
        assert_eq!(
            hex_char_to_value('g', 0),
            Err(Error::InvalidHexEncoding { c: 'g', index: 0 })
        );
    }
}
