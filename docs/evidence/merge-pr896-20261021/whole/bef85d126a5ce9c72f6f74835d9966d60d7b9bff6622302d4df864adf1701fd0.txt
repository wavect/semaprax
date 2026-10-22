//! Shared sealed test-fixture I/O; fixed descriptors grant no source or compiler authority.
#![cfg(test)]
use sha2::{Digest, Sha256};
use std::io::Read;

// Largest exact original fixture; never grow a buffer from untrusted metadata.
pub(super) const OUTPUT_CAP: usize = 5_081_115;
pub(super) struct Spec {
    pub(super) path: &'static str,
    pub(super) length: usize,
    pub(super) sha256: [u8; 32],
}
#[derive(Debug, Eq, PartialEq)]
pub(super) enum Failure {
    Input,
    Bound,
    Allocation,
    Read,
    Trailing,
    Digest,
    Utf8,
}
pub(super) fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
// Fixed repository payloads use the same sealed one-handle reader. Their
// complete immutable bytes remain required at test runtime; no global cache.
#[derive(Clone, Copy)]
#[allow(
    dead_code,
    reason = "Both fixed historical and ordinary-plan descriptors stay complete when the workspace harness uses only the latter."
)]
pub(crate) enum AdditionalFixture {
    HistoricalCompact7,
    HistoricalReceivers12,
    OrdinaryPlanHistory,
    OrdinaryPlanIntegrated,
}
impl AdditionalFixture {
    pub(super) fn spec(self) -> Spec {
        match self {
            Self::HistoricalCompact7 => Spec {
                path: concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/main8a-historical-readers-257-268-20261016/Compact7.json"
                ),
                length: 1084990,
                sha256: [
                    0, 149, 173, 86, 200, 253, 74, 28, 73, 247, 87, 156, 250, 159, 42, 179, 31, 58,
                    58, 117, 45, 255, 18, 239, 111, 255, 168, 184, 62, 65, 150, 117,
                ],
            },
            Self::HistoricalReceivers12 => Spec {
                path: concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/main8a-historical-readers-257-268-20261016/Receivers12.json"
                ),
                length: 2125481,
                sha256: [
                    223, 155, 190, 240, 51, 198, 246, 133, 193, 29, 115, 33, 18, 160, 218, 247,
                    198, 191, 36, 101, 22, 191, 240, 124, 254, 242, 172, 36, 136, 208, 145, 167,
                ],
            },
            Self::OrdinaryPlanHistory => Spec {
                path: concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/ordinary-plan-history-c8740045-v1/inputs.json"
                ),
                length: 784669,
                sha256: [
                    165, 217, 149, 202, 212, 213, 236, 255, 110, 168, 30, 179, 16, 88, 10, 154, 44,
                    170, 246, 49, 80, 113, 141, 73, 146, 114, 108, 185, 155, 135, 114, 120,
                ],
            },
            Self::OrdinaryPlanIntegrated => Spec {
                path: concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/ordinary-plan-integrated-e24-v1/stages.json"
                ),
                length: 1539271,
                sha256: [
                    218, 153, 227, 185, 89, 159, 100, 232, 121, 186, 67, 100, 106, 239, 75, 105,
                    249, 39, 245, 102, 178, 80, 7, 201, 108, 214, 208, 201, 50, 168, 21, 189,
                ],
            },
        }
    }
}
pub(crate) fn additional(fixture: AdditionalFixture) -> String {
    let spec = fixture.spec();
    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)
        .expect("complete authenticated fixed historical repository payload")
}

pub(super) fn read_fixture(
    path: &std::path::Path,
    expected_len: usize,
    expected_sha: [u8; 32],
) -> Result<String, Failure> {
    if expected_len > OUTPUT_CAP {
        return Err(Failure::Bound);
    }
    // Metadata and bytes come from ONE opened handle. A symlink/path swap cannot
    // change that descriptor, and only a regular file with exact length proceeds.
    let mut file = std::fs::File::open(path).map_err(|_| Failure::Input)?;
    let metadata = file.metadata().map_err(|_| Failure::Input)?;
    if !metadata.is_file() || metadata.len() != expected_len as u64 {
        return Err(Failure::Bound);
    }
    read_body(&mut file, expected_len, expected_sha)
}
pub(super) fn read_body(
    input: &mut impl Read,
    expected_len: usize,
    expected_sha: [u8; 32],
) -> Result<String, Failure> {
    if expected_len > OUTPUT_CAP {
        return Err(Failure::Bound);
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(expected_len)
        .map_err(|_| Failure::Allocation)?;
    bytes.resize(expected_len, 0);
    input.read_exact(&mut bytes).map_err(|_| Failure::Read)?;
    let mut extra = [0_u8; 1];
    if input.read(&mut extra).map_err(|_| Failure::Read)? != 0 {
        return Err(Failure::Trailing);
    }
    if digest(&bytes) != expected_sha {
        return Err(Failure::Digest);
    }
    String::from_utf8(bytes).map_err(|_| Failure::Utf8)
}
