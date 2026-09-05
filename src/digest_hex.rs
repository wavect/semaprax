//! Allocation-free lowercase hexadecimal formatting for digest outputs.

use std::fmt;

/// Adapts digest output bytes to the lowercase hexadecimal formatter.
///
/// This preserves the formatting surface that `digest` 0.10 exposed while
/// keeping canonical digest rendering independent of its backing array type.
#[doc(hidden)]
pub struct LowerHex<T>(pub T);

impl<T: AsRef<[u8]>> fmt::LowerHex for LowerHex<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        const LOWER: &[u8; 16] = b"0123456789abcdef";
        let digest = self.0.as_ref();
        if digest.len() != 32 {
            return Err(fmt::Error);
        }
        let mut encoded = [0u8; 64];
        for (index, byte) in digest.iter().copied().enumerate() {
            encoded[index * 2] = LOWER[usize::from(byte >> 4)];
            encoded[index * 2 + 1] = LOWER[usize::from(byte & 0x0f)];
        }
        formatter.write_str(std::str::from_utf8(&encoded).map_err(|_| fmt::Error)?)
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::{self, Write as _};

    use super::LowerHex;

    fn render(digest: &[u8]) -> Result<String, fmt::Error> {
        let mut output = String::new();
        write!(output, "{:x}", LowerHex(digest))?;
        Ok(output)
    }

    #[test]
    fn every_byte_renders_both_nibbles_in_order() {
        assert_eq!(render(&[0x00; 32]).unwrap(), "0".repeat(64));
        assert_eq!(render(&[0xff; 32]).unwrap(), "f".repeat(64));

        // A byte below 0x10 keeps its leading zero nibble. Dropping it would
        // shorten the digest and let distinct digests share a rendering.
        let mut digest = [0x00; 32];
        digest[0] = 0x0a;
        digest[31] = 0x01;
        let rendered = render(&digest).unwrap();
        assert_eq!(rendered.len(), 64);
        assert!(rendered.starts_with("0a"), "{rendered}");
        assert!(rendered.ends_with("01"), "{rendered}");

        // High and low nibbles are not transposed.
        let mut digest = [0x00; 32];
        digest[0] = 0xa0;
        assert!(render(&digest).unwrap().starts_with("a0"));
    }

    #[test]
    fn rendering_is_injective_and_fixed_width_over_every_byte_value() {
        let mut seen = std::collections::BTreeSet::new();
        for value in 0x00..=0xffu8 {
            let mut digest = [0x00; 32];
            digest[7] = value;
            let rendered = render(&digest).unwrap();
            assert_eq!(rendered.len(), 64, "{value:#04x} changed the width");
            assert_eq!(
                &rendered[14..16],
                format!("{value:02x}"),
                "{value:#04x} rendered at the wrong offset"
            );
            assert!(seen.insert(rendered), "{value:#04x} aliased another byte");
        }
        assert_eq!(seen.len(), 256);
    }

    #[test]
    fn digests_that_are_not_thirty_two_bytes_are_refused() {
        // Canonical digests are exactly 32 bytes. A shorter or longer input
        // must fail the formatter rather than emit a truncated or overlong
        // hexadecimal string that would still parse as a digest downstream.
        for length in [0usize, 1, 31, 33, 64] {
            assert!(
                render(&vec![0x11; length]).is_err(),
                "{length}-byte input was rendered"
            );
        }
        assert!(render(&[0x11; 32]).is_ok());
    }
}
