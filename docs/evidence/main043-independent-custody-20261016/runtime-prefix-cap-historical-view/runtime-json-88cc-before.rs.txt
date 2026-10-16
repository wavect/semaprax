//! Authenticated test-only repository data. No compiler or budget authority.
//! Runtime files are required at the exact compile-time repository location.
#![cfg(test)]
use sha2::{Digest, Sha256};
use std::io::Read;

#[path = "runtime_json/inventory.rs"]
mod inventory;
pub(super) use inventory::Fixture;
#[path = "runtime_json/membership.rs"]
mod membership;
// The sole measured-arm source switch; both transport sources are common.
#[path = "runtime_json/pair_custody.rs"]
mod pair_custody;
#[path = "runtime_json/transport_runtime.rs"]
mod selected_transport;
pub(super) fn pair_normalize(source: &str, current: &str) -> String {
    pair_custody::normalize(source, current)
}
pub(super) fn pair_previous(source: &str, current: &str) -> String {
    pair_custody::previous(source, current)
}

// Largest exact original fixture; never grow a buffer from untrusted metadata.
const OUTPUT_CAP: usize = 5_081_115;
struct Spec {
    path: &'static str,
    length: usize,
    sha256: [u8; 32],
}
#[derive(Debug, Eq, PartialEq)]
enum Failure {
    Input,
    Bound,
    Allocation,
    Read,
    Trailing,
    Digest,
    Utf8,
}
fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
// Sealed sibling inventory only; no arbitrary path or live-source API.
// Independent complete Main8a immutable tables, through the unchanged sealed reader.
pub(super) fn main8a_metadata(fixture: super::main8a_custody::Fixture) -> String {
    let spec = fixture.spec();
    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)
        .expect("complete authenticated Main8a immutable metadata")
}
pub(super) fn metadata(fixture: super::metadata_json::Fixture) -> String {
    let spec = fixture.spec();
    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)
        .expect("complete authenticated immutable source metadata")
}
// Final merge input capsules use the same fixed, bounded authenticated reader.
pub(super) fn main8a_input(fixture: super::main8a_custody::Input) -> String {
    let spec = fixture.spec();
    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)
        .expect("complete authenticated Main8a input capsule")
}
// Fixed final sibling only; the original one-descriptor reader is unchanged.
pub(super) fn final_custody(fixture: super::final_custody::Fixture) -> String {
    let spec = fixture.spec();
    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)
        .expect("complete authenticated final source custody")
}
// Sealed successor inventory; identical private descriptor reader.
pub(super) fn successor_custody(fixture: super::successor_custody::Fixture) -> String {
    let spec = fixture.spec();
    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)
        .expect("complete authenticated successor source custody")
}
pub(super) fn prefix_custody(fixture: super::prefix_custody::Fixture) -> String {
    let spec = fixture.spec();
    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)
        .expect("complete authenticated prefix source custody")
}
pub(super) fn joined_custody(fixture: super::joined_custody::Fixture) -> String {
    let spec = fixture.spec();
    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)
        .expect("complete authenticated joined source custody")
}
pub(super) fn release_custody(fixture: super::release_custody::Fixture) -> String {
    let spec = fixture.spec();
    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)
        .expect("complete authenticated release source custody")
}
pub(super) fn load(fixture: Fixture) -> String {
    let spec = fixture.spec();
    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)
        .expect("complete authenticated immutable projection JSON fixture")
}
// The compiled index proves absence against these exact immutable fixture specs.
// An unused fixture is not opened on this route; the all-fixture owner separately
// authenticates availability, complete bytes and exact key multiplicity.
pub(super) fn project(fixture: Fixture, source: &str, current: &str) -> String {
    if membership::keys(fixture).binary_search(&source).is_err() {
        return current.to_owned();
    }
    super::project(&selected_transport::load(fixture), source, current)
}
fn read_fixture(
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
fn read_body(
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
#[path = "runtime_json/tests.rs"]
mod tests;

// Separate fixed a3 tables; the original single-fd/length/SHA/UTF8 reader is unchanged.
pub(super) fn maina3_metadata(fixture: super::maina3_custody::Fixture) -> String {
    let spec = fixture.spec();
    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)
        .expect("complete authenticated a3 immutable metadata")
}
pub(super) fn maina3_input(fixture: super::maina3_custody::Input) -> String {
    let spec = fixture.spec();
    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)
        .expect("complete authenticated a3 input capsule")
}

// Full incoming arrays use the same sealed reader; metadata never authorizes live work.
pub(super) fn main44_metadata(fixture: super::main44_custody::Fixture) -> String {
    let spec = fixture.spec();
    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256).expect("complete authenticated Main44 metadata")
}
pub(super) fn main44_input() -> String {
    let spec = super::main44_custody::input_spec();
    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256).expect("complete authenticated Main44 source domain")
}

// Independent fixed Main043 descriptors through the original sealed bounded reader.
pub(super) fn main043_metadata(fixture: super::main043_custody::Fixture) -> String {
    let spec = fixture.spec();
    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256).expect("complete authenticated Main043 metadata")
}
pub(super) fn main043_input() -> String {
    let spec = super::main043_custody::input_spec();
    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256).expect("complete authenticated Main043 helper input")
}
