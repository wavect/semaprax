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
