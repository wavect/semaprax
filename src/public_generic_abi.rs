//! Reference codec for the Public Generic Descriptor and Carrier v1 wire
//! formats, plus the frozen [Public Generic Boundary Profile
//! v1](../docs/PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md) bounds every submodule
//! reuses.
//!
//! This module is scope reconciliation and contract freeze made executable,
//! not a compiler implementation: it defines no classifier over checked HIR,
//! touches no resolver, backend, or `src/wasm` code, and admits no program
//! source. [`descriptor`] and [`carrier`] encode, decode, and independently
//! replay values a future classifier will eventually produce; in this round
//! those values are hand-constructed fixtures, exercised for byte
//! determinism and fail-closed hostile-input handling only. Public generic
//! ownership remains unsupported and unpublished; nothing here is a public
//! ABI, and nothing here executes a real boundary.
//!
//! | Document | Module |
//! | --- | --- |
//! | [Public Generic Boundary Profile v1](../docs/PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md) | [`boundary_profile`] (bounds only; the admission classifier is not implemented this round) |
//! | [Public Generic Descriptor v1](../docs/PUBLIC-GENERIC-DESCRIPTOR-V1.md) | [`descriptor`] |
//! | [Public Generic Carrier v1](../docs/PUBLIC-GENERIC-CARRIER-V1.md) | [`carrier`] |

pub mod boundary_profile;
pub mod carrier;
pub mod descriptor;

/// Length-prefix one field: an 8-byte little-endian length, then the bytes.
/// Shared framing convention with `public_generic_type` and
/// `public_generic_settlement`.
pub(crate) fn frame(preimage: &mut Vec<u8>, bytes: &[u8]) {
    preimage.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    preimage.extend_from_slice(bytes);
}

/// Read one length-framed field from `input` at `offset`, returning the field
/// bytes and the offset just past them. Fails closed on a truncated prefix,
/// a length that would read past the end of `input`, or a declared length
/// past `max_len`.
pub(crate) fn read_frame(input: &[u8], offset: usize, max_len: usize) -> Option<(&[u8], usize)> {
    let header_end = offset.checked_add(8)?;
    let header = input.get(offset..header_end)?;
    let len = u64::from_le_bytes(header.try_into().ok()?);
    let len = usize::try_from(len).ok()?;
    if len > max_len {
        return None;
    }
    let field_end = header_end.checked_add(len)?;
    let field = input.get(header_end..field_end)?;
    Some((field, field_end))
}

/// Domain-separated SHA-256 over one preimage, rendered as `sha256:<hex>`.
/// Identical convention to `public_generic_type::digest` and
/// `public_generic_settlement::digest`.
pub(crate) fn digest(domain: &[u8], bytes: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_and_read_frame_round_trip() {
        let mut preimage = Vec::new();
        frame(&mut preimage, b"hello");
        frame(&mut preimage, b"");
        frame(&mut preimage, b"world");
        let (first, offset) = read_frame(&preimage, 0, 1024).unwrap();
        assert_eq!(first, b"hello");
        let (second, offset) = read_frame(&preimage, offset, 1024).unwrap();
        assert_eq!(second, b"");
        let (third, offset) = read_frame(&preimage, offset, 1024).unwrap();
        assert_eq!(third, b"world");
        assert_eq!(offset, preimage.len());
    }

    #[test]
    fn read_frame_rejects_truncated_header() {
        assert!(read_frame(&[1, 2, 3], 0, 1024).is_none());
    }

    #[test]
    fn read_frame_rejects_declared_length_past_input() {
        let mut preimage = Vec::new();
        frame(&mut preimage, b"short");
        // Corrupt the length prefix to claim more bytes than are present.
        preimage[0..8].copy_from_slice(&(1_000_000u64).to_le_bytes());
        assert!(read_frame(&preimage, 0, 1024).is_none());
    }

    #[test]
    fn read_frame_rejects_length_over_max() {
        let mut preimage = Vec::new();
        frame(&mut preimage, b"0123456789");
        assert!(read_frame(&preimage, 0, 5).is_none());
    }
}
