//! Git-format SHA1 compatibility only. This is not collision detection and must
//! never be used for candidate approval, semantic identities or modern integrity.
use super::{CandidateGitObjectKind, GitObjectFormat};
use sha2::{Digest, Sha256};

pub(super) fn object_oid(
    format: GitObjectFormat,
    kind: CandidateGitObjectKind,
    bytes: &[u8],
) -> String {
    let header = format!("{} {}\0", kind.name(), bytes.len());
    match format {
        GitObjectFormat::Sha256 => {
            let mut hash = Sha256::new();
            hash.update(header.as_bytes());
            hash.update(bytes);
            format!("{:x}", crate::digest_hex::LowerHex(hash.finalize()))
        }
        GitObjectFormat::Sha1 => hex(&sha1(&[header.as_bytes(), bytes])),
    }
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
fn sha1(parts: &[&[u8]]) -> [u8; 20] {
    let mut state = [
        0x67452301u32,
        0xefcdab89,
        0x98badcfe,
        0x10325476,
        0xc3d2e1f0,
    ];
    let mut block = [0u8; 64];
    let mut used = 0;
    let mut length = 0u64;
    for part in parts {
        length += part.len() as u64;
        for &byte in *part {
            block[used] = byte;
            used += 1;
            if used == 64 {
                compress(&mut state, &block);
                used = 0;
            }
        }
    }
    block[used] = 0x80;
    used += 1;
    if used > 56 {
        block[used..].fill(0);
        compress(&mut state, &block);
        block.fill(0);
    } else {
        block[used..56].fill(0);
    }
    block[56..].copy_from_slice(&(length * 8).to_be_bytes());
    compress(&mut state, &block);
    let mut output = [0u8; 20];
    for (word, slot) in state.iter().zip(output.chunks_exact_mut(4)) {
        slot.copy_from_slice(&word.to_be_bytes());
    }
    output
}
fn compress(state: &mut [u32; 5], block: &[u8; 64]) {
    let mut words = [0u32; 80];
    for (word, bytes) in words[..16].iter_mut().zip(block.chunks_exact(4)) {
        *word = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    }
    for i in 16..80 {
        words[i] = (words[i - 3] ^ words[i - 8] ^ words[i - 14] ^ words[i - 16]).rotate_left(1);
    }
    let [mut a, mut b, mut c, mut d, mut e] = *state;
    for (i, word) in words.iter().enumerate() {
        let (f, k) = match i {
            0..=19 => ((b & c) | (!b & d), 0x5a827999u32),
            20..=39 => (b ^ c ^ d, 0x6ed9eba1),
            40..=59 => ((b & c) | (b & d) | (c & d), 0x8f1bbcdc),
            _ => (b ^ c ^ d, 0xca62c1d6),
        };
        let next = a
            .rotate_left(5)
            .wrapping_add(f)
            .wrapping_add(e)
            .wrapping_add(k)
            .wrapping_add(*word);
        e = d;
        d = c;
        c = b.rotate_left(30);
        b = a;
        a = next;
    }
    for (state, value) in state.iter_mut().zip([a, b, c, d, e]) {
        *state = state.wrapping_add(value);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn standard_sha1_known_answers_and_chunk_boundaries() {
        for (message, expected) in [
            ("", "da39a3ee5e6b4b0d3255bfef95601890afd80709"),
            ("abc", "a9993e364706816aba3e25717850c26c9cd0d89d"),
            (
                "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
                "84983e441c3bd26ebaae4aa1f95129e5e54670f1",
            ),
        ] {
            assert_eq!(hex(&sha1(&[message.as_bytes()])), expected);
            for split in 0..=message.len() {
                assert_eq!(
                    hex(&sha1(&[
                        &message.as_bytes()[..split],
                        &message.as_bytes()[split..]
                    ])),
                    expected
                );
            }
        }
        assert_eq!(
            hex(&sha1(&[&vec![b'a'; 1_000_000]])),
            "34aa973cd4c4daa4f61eeb2bdbad27316534016f"
        );
    }
    #[test]
    fn git_empty_object_known_answers() {
        assert_eq!(
            object_oid(GitObjectFormat::Sha1, CandidateGitObjectKind::Blob, b""),
            "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391"
        );
        assert_eq!(
            object_oid(GitObjectFormat::Sha1, CandidateGitObjectKind::Tree, b""),
            "4b825dc642cb6eb9a060e54bf8d69288fbee4904"
        );
    }
}
