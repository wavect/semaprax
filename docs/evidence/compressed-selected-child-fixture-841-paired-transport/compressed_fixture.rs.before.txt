//! Bounded test-only evidence decoding. No measured-route work or global cache.
#![cfg(test)]
use flate2::bufread::GzDecoder;
use sha2::{Digest, Sha256};
use std::io::Read;

const OUTPUT_CAP: usize = 5_196_870;
const COMPRESSED_CAP: usize = 524_288;
const BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/evidence/compressed-selected-child-fixture-841/exact-source-inverses.json.gz"
));
#[derive(Debug, Eq, PartialEq)]
enum Failure {
    Bound,
    CompressedDigest,
    Allocation,
    Decode,
    OutputExceeded,
    TrailingInput,
    DecodedDigest,
    Utf8,
}
fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
fn decode(
    bytes: &[u8],
    expected_len: usize,
    compressed_sha: [u8; 32],
    raw_sha: [u8; 32],
) -> Result<String, Failure> {
    // Both limits precede the decoder and its output allocation. No read_to_end,
    // growing output buffer or unbounded filename/payload input is admitted.
    if expected_len > OUTPUT_CAP || bytes.len() > COMPRESSED_CAP {
        return Err(Failure::Bound);
    }
    if digest(bytes) != compressed_sha {
        return Err(Failure::CompressedDigest);
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(expected_len)
        .map_err(|_| Failure::Allocation)?;
    output.resize(expected_len, 0);
    let mut decoder = GzDecoder::new(bytes);
    decoder
        .read_exact(&mut output)
        .map_err(|_| Failure::Decode)?;
    // Force CRC/length trailer validation and refuse even one byte beyond the
    // fixed output slice. The decoder never receives a growing output Vec.
    let mut extra = [0u8; 1];
    if decoder.read(&mut extra).map_err(|_| Failure::Decode)? != 0 {
        return Err(Failure::OutputExceeded);
    }
    // bufread retains unread input: reject trailing bytes and additional members.
    if !decoder.into_inner().is_empty() {
        return Err(Failure::TrailingInput);
    }
    if output.len() != expected_len || digest(&output) != raw_sha {
        return Err(Failure::DecodedDigest);
    }
    String::from_utf8(output).map_err(|_| Failure::Utf8)
}
pub(super) fn load() -> String {
    check_decoder_contract();
    decode(BYTES, OUTPUT_CAP, GZIP_SHA, RAW_SHA).expect("complete authenticated immutable fixture")
}
const GZIP_SHA: [u8; 32] = [
    108, 122, 171, 139, 39, 30, 147, 21, 28, 217, 60, 121, 234, 223, 41, 85, 216, 240, 207, 159,
    174, 59, 220, 223, 129, 110, 24, 28, 242, 191, 202, 17,
];
const RAW_SHA: [u8; 32] = [
    47, 243, 227, 113, 200, 191, 222, 135, 110, 222, 79, 94, 116, 69, 162, 96, 103, 133, 142, 74,
    13, 132, 97, 12, 202, 148, 213, 179, 204, 193, 36, 14,
];

// These tiny controls execute in this same source-only test, outside every
// allocation observer and active compiler budget; there is no production route.
fn check_decoder_contract() {
    let small: &[u8] = &[
        31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 171, 86, 74, 202, 47, 205, 75, 73, 77, 81, 178, 42, 41,
        42, 77, 173, 229, 2, 0, 79, 228, 19, 196, 17, 0, 0, 0,
    ];
    let bad_utf8: &[u8] = &[
        31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 251, 15, 0, 0, 0, 0, 255, 1, 0, 0, 0,
    ];
    let small_raw = b"{\"bounded\":true}\n";
    let compressed = digest(small);
    let raw = digest(small_raw);
    assert_eq!(
        decode(small, small_raw.len(), compressed, raw)
            .unwrap()
            .as_bytes(),
        small_raw
    );
    assert_eq!(
        decode(small, OUTPUT_CAP + 1, compressed, raw),
        Err(Failure::Bound)
    );
    assert_eq!(
        decode(small, small_raw.len(), [0; 32], raw),
        Err(Failure::CompressedDigest)
    );
    assert_eq!(
        decode(small, small_raw.len(), compressed, [0; 32]),
        Err(Failure::DecodedDigest)
    );
    assert_eq!(
        decode(small, small_raw.len() - 1, compressed, raw),
        Err(Failure::OutputExceeded)
    );
    assert_eq!(
        decode(small, small_raw.len() + 1, compressed, raw),
        Err(Failure::Decode)
    );
    let mut corrupt = small.to_vec();
    *corrupt.last_mut().unwrap() ^= 1;
    assert_eq!(
        decode(&corrupt, small_raw.len(), digest(&corrupt), raw),
        Err(Failure::Decode)
    );
    let mut trailing = small.to_vec();
    trailing.push(0);
    assert_eq!(
        decode(&trailing, small_raw.len(), digest(&trailing), raw),
        Err(Failure::TrailingInput)
    );
    assert_eq!(
        decode(bad_utf8, 1, digest(bad_utf8), digest(&[255])),
        Err(Failure::Utf8)
    );
}

/// Whole current inputs are authenticated before entering EVERY unchanged
/// legacy JSON inverse. The four old drift domains are exact immutable e4b
/// Git blobs, not slices or inferred formatting; changed self projects to851.
pub(super) fn prior_source<'a>(path: &str, current: &'a str) -> &'a str {
    let (expected, checkpoint, checkpoint_sha): ([u8;32], &str, [u8;32]) = match path {
        "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs" => (CURRENT_SHA_0, include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/compressed-selected-child-fixture-841/legacy-input-0.txt")), [215, 100, 232, 214, 251, 211, 115, 162, 219, 180, 9, 83, 186, 171, 246, 136, 251, 68, 89, 86, 39, 96, 253, 94, 21, 69, 203, 155, 115, 185, 230, 176]),
        "src/cleanup_plan/replay/live_oracle/source_custody/b.rs" => (CURRENT_SHA_1, include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/compressed-selected-child-fixture-841/legacy-input-1.txt")), [77, 83, 75, 84, 51, 7, 141, 141, 101, 236, 159, 9, 199, 247, 99, 97, 74, 23, 124, 215, 67, 170, 2, 70, 93, 230, 76, 2, 62, 80, 145, 115]),
        "tests/hir_module_boundaries.rs" => (CURRENT_SHA_2, include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/compressed-selected-child-fixture-841/legacy-input-2.txt")), [5, 90, 143, 125, 221, 139, 159, 91, 45, 18, 18, 33, 84, 222, 5, 58, 10, 104, 35, 183, 92, 41, 133, 128, 205, 71, 112, 98, 140, 78, 136, 81]),
        "tests/workspace/semantic_graph/source_inventory.rs" => (CURRENT_SHA_3, include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/compressed-selected-child-fixture-841/legacy-input-3.txt")), [74, 25, 32, 246, 171, 223, 249, 0, 54, 254, 154, 250, 190, 104, 124, 9, 27, 8, 163, 141, 79, 38, 162, 142, 46, 124, 127, 235, 103, 140, 189, 20]),
        "src/cleanup_plan/replay/selected_context/child/tests/source.rs" => (CURRENT_SHA_4, include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/compressed-selected-child-fixture-841/source.rs.before.txt")), [157, 216, 64, 176, 250, 187, 161, 109, 89, 100, 163, 136, 14, 190, 161, 194, 40, 52, 18, 203, 131, 219, 132, 85, 172, 195, 27, 141, 140, 238, 113, 69]),
        _ => return current,
    };
    assert_eq!(
        digest(current.as_bytes()),
        expected,
        "complete current {path}"
    );
    assert_eq!(
        digest(checkpoint.as_bytes()),
        checkpoint_sha,
        "complete checkpoint {path}"
    );
    checkpoint
}
const CURRENT_SHA_0: [u8; 32] = [
    183, 35, 19, 195, 174, 141, 165, 119, 223, 72, 85, 233, 229, 125, 222, 98, 61, 11, 91, 116,
    140, 174, 21, 32, 15, 104, 203, 134, 142, 165, 24, 212,
];
const CURRENT_SHA_1: [u8; 32] = [
    127, 127, 12, 108, 82, 73, 89, 16, 145, 167, 254, 125, 220, 98, 66, 99, 28, 235, 99, 246, 176,
    5, 3, 64, 54, 226, 78, 94, 165, 186, 92, 80,
];
const CURRENT_SHA_2: [u8; 32] = [
    208, 79, 181, 17, 206, 56, 218, 87, 202, 82, 188, 199, 75, 193, 9, 114, 0, 138, 229, 39, 60,
    214, 206, 113, 228, 154, 191, 149, 223, 215, 23, 85,
];
const CURRENT_SHA_3: [u8; 32] = [
    123, 87, 118, 189, 202, 184, 61, 175, 28, 16, 255, 147, 42, 29, 254, 2, 119, 2, 228, 115, 9,
    95, 125, 134, 127, 163, 53, 24, 178, 100, 181, 153,
];
const CURRENT_SHA_4: [u8; 32] = [
    55, 171, 9, 110, 116, 49, 7, 85, 141, 37, 147, 143, 204, 246, 44, 197, 227, 99, 107, 209, 10,
    215, 172, 117, 29, 104, 132, 215, 55, 139, 152, 154,
];
