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
#[allow(
    dead_code,
    reason = "Retained compatibility entry for independently compiled historical and semantic controls."
)]
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
pub(super) fn maina3_input(fixture: super::maina3_custody::Input, source: &str) -> String {
    let spec = fixture.spec(source);
    let payload = read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)
        .expect("complete authenticated a3 input capsule");
    maina3_part_source(&payload, source);
    payload
}
fn maina3_part_source(payload: &str, source: &str) {
    let rows: serde_json::Value = serde_json::from_str(payload).expect("fixed a3 part rows");
    assert_eq!(
        rows.as_array()
            .unwrap()
            .iter()
            .filter(|row| row["source"] == source)
            .count(),
        1,
        "exact known a3 source in selected bounded part"
    );
}

// Full incoming arrays use the same sealed reader; metadata never authorizes live work.
pub(super) fn main44_metadata(fixture: super::main44_custody::Fixture) -> String {
    let spec = fixture.spec();
    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)
        .expect("complete authenticated Main44 metadata")
}
pub(super) fn main44_input() -> String {
    let spec = super::main44_custody::input_spec();
    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)
        .expect("complete authenticated Main44 source domain")
}

// Independent fixed Main043 descriptors through the original sealed bounded reader.
pub(super) fn main043_metadata(fixture: super::main043_custody::Fixture) -> String {
    let spec = fixture.spec();
    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)
        .expect("complete authenticated Main043 metadata")
}
pub(super) fn main043_input() -> String {
    let spec = super::main043_custody::input_spec();
    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)
        .expect("complete authenticated Main043 helper input")
}

fn main043_raw_same_file(a: &std::fs::Metadata, b: &std::fs::Metadata) -> bool {
    if !a.is_file() || !b.is_file() || a.len() != b.len() || a.modified().ok() != b.modified().ok()
    {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if a.dev() != b.dev()
            || a.ino() != b.ino()
            || a.ctime() != b.ctime()
            || a.ctime_nsec() != b.ctime_nsec()
        {
            return false;
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if a.file_attributes() & 0x400 != 0
            || b.file_attributes() & 0x400 != 0
            || a.creation_time() != b.creation_time()
            || a.last_write_time() != b.last_write_time()
        {
            return false;
        }
    }
    true
}

fn main043_raw_file(
    path: &std::path::Path,
    length: usize,
    expected: &str,
) -> Result<String, &'static str> {
    if length > OUTPUT_CAP {
        return Err("bound");
    }
    let path_before = std::fs::symlink_metadata(path).map_err(|_| "missing")?;
    if path_before.file_type().is_symlink() || !path_before.is_file() {
        return Err("length-or-nonregular");
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x0020_0000);
    }
    #[cfg(not(any(unix, windows)))]
    {
        return Err("platform");
    }
    let mut file = options.open(path).map_err(|_| "open")?;
    let metadata = file.metadata().map_err(|_| "metadata")?;
    if !main043_raw_same_file(&path_before, &metadata)
        || metadata.len() != u64::try_from(length).map_err(|_| "bound")?
    {
        return Err("length-or-nonregular");
    }
    // The one opened no-follow regular handle is checked before any buffer.
    let value = read_body(&mut file, length, main043_raw_pin(expected)).map_err(|_| "content")?;
    let after = file.metadata().map_err(|_| "metadata")?;
    let path_after = std::fs::symlink_metadata(path).map_err(|_| "metadata")?;
    if path_after.file_type().is_symlink()
        || !main043_raw_same_file(&metadata, &after)
        || !main043_raw_same_file(&after, &path_after)
    {
        return Err("metadata-drift");
    }
    Ok(value)
}

// RAW audit inputs remain the complete current include_str images. This owner
// composes exact later inverses ahead of every unchanged original recipe.
fn main043_raw_pin(text: &str) -> [u8; 32] {
    assert_eq!(text.len(), 64);
    assert!(text
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
    let mut result = [0_u8; 32];
    for (index, byte) in result.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).unwrap();
    }
    result
}

fn main043_raw_subject(source: &str) -> (&'static str, &'static str, &'static str) {
    match source {
        "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs" => (concat!(env!("CARGO_MANIFEST_DIR"), "/crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs"), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs")), concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/source-fronts-aaa2-20261020/whole/3b06439c8daabdb9be2c25da3ce633d18f640f37709dceb8fb09865bfe1916f8.txt")),
        "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory_leaf_views.rs" => (concat!(env!("CARGO_MANIFEST_DIR"), "/crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory_leaf_views.rs"), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory_leaf_views.rs")), concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/source-fronts-aaa2-20261020/whole/842979bf706a0ce014e74436eefbc722ccbef7e605f5aaeba3893dca7e55781f.txt")),
        "src/cleanup_plan/replay.rs" => (concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay.rs"), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay.rs")), concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/source-fronts-aaa2-20261020/whole/73a3f6013227cf0af2475046ccd2f2ed7f73bb96aef9a8a0c9e7f69906f0a651.txt")),
        "src/cleanup_plan/replay/expression_facts/arena.rs" => (concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/arena.rs"), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/arena.rs")), concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/source-fronts-aaa2-20261020/whole/c075f4e758cd36ab40328df263bbd8b4836c8012abdcc6c4fd23fd95efb13a59.txt")),
        "src/cleanup_plan/replay/expression_facts/errors.rs" => (concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/errors.rs"), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/errors.rs")), concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/source-fronts-aaa2-20261020/whole/753b8f3988581911db3fade57d7de677922ca90f16e8282a05caf641879d93c8.txt")),
        "src/cleanup_plan/replay/expression_facts/receiving.rs" => (concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving.rs"), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving.rs")), concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/source-fronts-aaa2-20261020/whole/d44da2c3d647fa0e9dadeb06898b54f12292a582f3db4020036af2bb2b2b0a25.txt")),
        "src/cleanup_plan/replay/expression_facts/receiving/retained.rs" => (concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained.rs"), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained.rs")), concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/source-fronts-aaa2-20261020/whole/74cc8eceace6dc734e1a6a2631bab7d85baee2d2e7f19df35f26cdd6e98f9d74.txt")),
        "src/cleanup_plan/replay/live_oracle/source_custody/a.rs" => (concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/live_oracle/source_custody/a.rs"), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/live_oracle/source_custody/a.rs")), concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/source-fronts-aaa2-20261020/whole/e410e69fa3ab135803fe03e401a11be444e8c65e90f40f0e97bdf90dac3b246e.txt")),
        "src/cleanup_plan/replay/live_oracle/source_custody/a_tail.rs" => (concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/live_oracle/source_custody/a_tail.rs"), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/live_oracle/source_custody/a_tail.rs")), concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/source-fronts-aaa2-20261020/whole/20c5c864bc3eb84f4d832b1f67ea5898279950b5ac7ba06a7381a2adb4b50637.txt")),
        "src/cleanup_plan/replay/live_oracle/source_custody/b.rs" => (concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/live_oracle/source_custody/b.rs"), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/live_oracle/source_custody/b.rs")), concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/source-fronts-aaa2-20261020/whole/dfda5c8a4e70539ccedc8dfbee0adabcf2ec7e5696d97db0c6b3c583f47ba25f.txt")),
        "src/cleanup_plan/replay/live_oracle/source_custody/b_tail.rs" => (concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/live_oracle/source_custody/b_tail.rs"), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/live_oracle/source_custody/b_tail.rs")), concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/source-fronts-aaa2-20261020/whole/007cdd753efd8749cc7ca4f221569ab1ecd779a85590970bf459c51d34840011.txt")),
        "src/cleanup_plan/replay/selected_context.rs" => (concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_context.rs"), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_context.rs")), concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/source-fronts-aaa2-20261020/whole/98777200528b6dad5a6c49ccc761c7b5e5bddb26a64b1cffae0e31ba2ff2290d.txt")),
        "src/cleanup_plan/replay/selected_context/paid.rs" => (concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_context/paid.rs"), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_context/paid.rs")), concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/source-fronts-aaa2-20261020/whole/516a679a771357368fec0a5f9e1746708ef9657057b69747f0f7531a0bb07ed9.txt")),
        "src/cleanup_plan/replay/selected_entry/paid.rs" => (concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_entry/paid.rs"), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_entry/paid.rs")), concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/source-fronts-aaa2-20261020/whole/807dc71528f353dc7ff49ec568c78cac7f9c9d8a5abd0dd5ff658b3fe77ca669.txt")),
        "src/cleanup_plan/replay/selected_entry/tests/session_errors.rs" => (concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_entry/tests/session_errors.rs"), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_entry/tests/session_errors.rs")), concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/source-fronts-aaa2-20261020/whole/ace17b7f4e44235a0e058e169934da367fa1d4f96f8cddcfa19200eea9fa9552.txt")),
        "src/cleanup_plan/replay/selected_owner/paid.rs" => (concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_owner/paid.rs"), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_owner/paid.rs")), concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/source-fronts-aaa2-20261020/whole/328ed1b055f8d73185ee2137eb6da0fa59d3bce73e6ddb8a17de71dcae6160c6.txt")),
        "src/cleanup_plan/replay/selected_profile.rs" => (concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_profile.rs"), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_profile.rs")), concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/source-fronts-aaa2-20261020/whole/dc436ef85505b73d04cda47a9a2b68878ac0bdcc4e36e371d9f65268e3142a05.txt")),
        "src/cleanup_plan/replay/selected_profile/paid.rs" => (concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_profile/paid.rs"), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_profile/paid.rs")), concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/source-fronts-aaa2-20261020/whole/03f11956c2e39a3b58f429d48ad439fcaa1c1390520a5c1cd179ec148479596f.txt")),
        "src/cleanup_plan/replay/semantic_refusal.rs" => (concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/semantic_refusal.rs"), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/semantic_refusal.rs")), concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/source-fronts-aaa2-20261020/whole/a89778cddeb300e1f7441e6800aa8f1178c690203bc4db15b3357f21723be056.txt")),
        "src/cleanup_plan/replay/semantic_refusal/tests.rs" => (concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/semantic_refusal/tests.rs"), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/semantic_refusal/tests.rs")), concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/source-fronts-aaa2-20261020/whole/a3d350eb3b65bbd27691e861cc711b44501dc345b32395e9c70428fd7a09165e.txt")),
        "src/hir/declaration_index/identity_payer.rs" => (concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/declaration_index/identity_payer.rs"), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/declaration_index/identity_payer.rs")), concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/source-fronts-aaa2-20261020/whole/0575cab5f032f3b6a8dcebc5cd1788625b997f99432f7ea4ec6964201367fcd5.txt")),
        "tests/hir_module_boundaries.rs" => (concat!(env!("CARGO_MANIFEST_DIR"), "/tests/hir_module_boundaries.rs"), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/hir_module_boundaries.rs")), concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/source-fronts-aaa2-20261020/whole/fc913ecc7db54fa1e4ef480952e459b972a04171e40e7237964aff5d81be7e95.txt")),
        "tests/workspace/semantic_graph/source_inventory.rs" => (concat!(env!("CARGO_MANIFEST_DIR"), "/tests/workspace/semantic_graph/source_inventory.rs"), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/workspace/semantic_graph/source_inventory.rs")), concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/source-fronts-aaa2-20261020/whole/68338a176c682de788c747003b39a6c030128da1c39275f72ae24b5651410d19.txt")),
        _ => panic!("foreign RAW source subject"),
    }
}

fn main043_check_raw_subject(packet: &serde_json::Value, source: &str) -> String {
    let matches = packet["current_sources"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["source"] == source)
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "unique complete RAW source");
    let row = matches[0];
    let length = usize::try_from(row["length"].as_u64().unwrap()).unwrap();
    let expected = row["sha256"].as_str().unwrap();
    let (path, compiled, archive) = main043_raw_subject(source);
    assert_eq!(
        row["archive"].as_str().unwrap(),
        archive
            .strip_prefix(env!("CARGO_MANIFEST_DIR"))
            .unwrap()
            .trim_start_matches('/')
    );
    let physical = main043_raw_file(std::path::Path::new(path), length, expected).unwrap();
    let held = main043_raw_file(std::path::Path::new(archive), length, expected).unwrap();
    assert_eq!(physical, compiled, "complete current compiled RAW source");
    assert_eq!(physical, held, "complete current RAW source archive");
    physical
}

pub(super) fn main043_raw_metadata(fixture: super::main043_custody::Fixture) -> String {
    use super::main043_custody::Fixture as F;
    let (name, key, forward) = match fixture {
        F::ExitPrefix => ("ExitPrefix", "after", false),
        F::ExitDirectory => ("ExitDirectory", "after", false),
        F::LegacyNominal => ("LegacyNominal", "new", true),
        F::Lifecycle => ("Lifecycle", "after", true),
        F::Prefix => ("Prefix", "after", false),
        F::Receiving => ("Receiving", "after", false),
        F::Edge => ("Edge", "after", false),
        F::Selected => ("Selected", "new", true),
        F::Status => ("Status", "after", false),
        _ => return main043_metadata(fixture),
    };
    let original = main043_metadata(fixture);
    let raw = main043_raw_file(
        std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/source-fronts-aaa2-20261020/raw-diffs.json"
        )),
        703578,
        "71df60c9609881ddfc71baf22d7d33fcdbbd35c06bb1b421a76f8d46e53e61d5",
    )
    .unwrap();
    let packet: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(
        packet["schema"],
        "semaprax.main043-raw-inverse-successor.v1"
    );
    let matching = packet["fixtures"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["name"] == name)
        .collect::<Vec<_>>();
    assert_eq!(matching.len(), 1, "closed RAW fixture");
    let spec = matching[0];
    assert_eq!(spec["key"], key);
    assert_eq!(
        spec["direction"],
        if forward { "forward" } else { "reverse" }
    );
    assert_eq!(
        spec["original_length"].as_u64().unwrap(),
        original.len() as u64
    );
    assert_eq!(
        main043_raw_pin(spec["original_sha256"].as_str().unwrap()),
        digest(original.as_bytes())
    );
    let before: serde_json::Value = serde_json::from_str(&original).unwrap();
    let mut rows = before.clone();
    let mut seen = std::collections::BTreeSet::new();
    for patch in spec["rows"].as_array().unwrap() {
        let index = usize::try_from(patch["row"].as_u64().unwrap()).unwrap();
        assert!(seen.insert(index));
        let source = patch["source"].as_str().unwrap();
        let row = &mut rows.as_array_mut().unwrap()[index];
        assert_eq!(row["source"], source);
        let front_index = usize::try_from(patch["front"].as_u64().unwrap()).unwrap();
        let front = &packet["fronts"].as_array().unwrap()[front_index];
        assert_eq!(front["index"].as_u64().unwrap(), front_index as u64);
        assert_eq!(front["source"], source);
        let mut current = main043_check_raw_subject(&packet, source);
        assert_eq!(
            current.len() as u64,
            front["current_length"].as_u64().unwrap()
        );
        assert_eq!(
            digest(current.as_bytes()),
            main043_raw_pin(front["current_sha256"].as_str().unwrap())
        );
        for child in front["extra_compiled_children"].as_array().unwrap() {
            // These are the audited include! splits of this exact raw owner.
            // Authenticate full physical/compiled/archive bytes before history.
            main043_check_raw_subject(&packet, child.as_str().unwrap());
        }
        let old_steps = row["edits"].as_array().unwrap().clone();
        let old_outer = if forward {
            old_steps.first()
        } else {
            old_steps.last()
        }
        .unwrap();
        let expected = old_outer[key].as_str().unwrap();
        assert_eq!(
            expected.len() as u64,
            front["previous_length"].as_u64().unwrap()
        );
        assert_eq!(
            digest(expected.as_bytes()),
            main043_raw_pin(front["previous_sha256"].as_str().unwrap())
        );
        let mut outer = Vec::new();
        for edit in front["steps"].as_array().unwrap() {
            let after = edit["after"].as_str().unwrap();
            let previous = edit["before"].as_str().unwrap();
            assert!(!after.is_empty());
            assert_eq!(
                current.matches(after).count(),
                1,
                "complete unique RAW successor step"
            );
            current = current.replacen(after, previous, 1);
            let mut e = serde_json::Map::new();
            e.insert(key.to_owned(), after.into());
            e.insert("before".to_owned(), previous.into());
            outer.push(serde_json::Value::Object(e));
        }
        assert_eq!(
            current, expected,
            "whole independently sealed previous RAW image"
        );
        let steps = row["edits"].as_array_mut().unwrap();
        if forward {
            outer.extend(old_steps.clone());
            *steps = outer;
        } else {
            steps.extend(outer.into_iter().rev());
        }
        let mut restored = row.clone();
        restored["edits"] = old_steps.into();
        assert_eq!(
            restored, before[index],
            "all original RAW row fields and recipes retained"
        );
    }
    assert_eq!(
        rows.as_array().unwrap().len(),
        before.as_array().unwrap().len()
    );
    let result = rows.to_string();
    assert!(result.len() <= OUTPUT_CAP);
    result
}

#[cfg(test)]
mod main043_raw_tests {
    use super::*;

    #[test]
    fn raw_successor_reader_keeps_bounds_digests_utf8_and_nofollow_refusals() {
        let directory = std::env::temp_dir().join(format!(
            "semaprax-main043-raw-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        std::fs::create_dir(&directory).unwrap();
        struct Remove(std::path::PathBuf);
        impl Drop for Remove {
            fn drop(&mut self) {
                std::fs::remove_dir_all(&self.0).unwrap();
            }
        }
        let _remove = Remove(directory.clone());
        let file = directory.join("raw");
        let bytes = b"complete\n";
        std::fs::write(&file, bytes).unwrap();
        let sha = format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)));
        assert_eq!(
            main043_raw_file(&file, bytes.len(), &sha)
                .unwrap()
                .as_bytes(),
            bytes
        );
        assert!(main043_raw_file(&file, OUTPUT_CAP + 1, &sha).is_err());
        assert!(main043_raw_file(&file, bytes.len() + 1, &sha).is_err());
        assert!(main043_raw_file(&file, bytes.len() - 1, &sha).is_err());
        assert!(main043_raw_file(&file, bytes.len(), &"0".repeat(64)).is_err());
        assert!(main043_raw_file(&directory, 0, &sha).is_err());
        assert!(main043_raw_file(&directory.join("missing"), 0, &sha).is_err());
        std::fs::write(&file, [255_u8]).unwrap();
        let invalid = format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest([255_u8]))
        );
        assert!(main043_raw_file(&file, 1, &invalid).is_err());
        #[cfg(unix)]
        {
            std::fs::write(&file, bytes).unwrap();
            let link = directory.join("link");
            std::os::unix::fs::symlink(&file, &link).unwrap();
            assert!(main043_raw_file(&link, bytes.len(), &sha).is_err());
        }
    }
}

// The two historical callers stop after different prefixes of the same
// original recipe. Complete that sealed recipe before its unchanged inverse.
pub(super) fn complete_receiver_followup(supplied: &str) -> String {
    const SOURCE: &str = "src/source_reflows/semantic_receivers.rs";
    const AFTER: &str = "17989402984251c9956537bc3d2d8c6a1a6c6252cb1bd188ffd265dc6cdd0a51";
    let supplied_sha = digest(supplied.as_bytes());
    assert!(
        supplied_sha
            == main043_raw_pin("b5bc349c44ec11c094484d7b5d40a534b2f3c66a10ba7b16af9dd08eb8012607")
            || supplied_sha
                == main043_raw_pin(
                    "68c95445cf0a8e9202822098149f74890cfc8807ab352ec071824e8705f4d28f"
                ),
        "closed original followup prefix"
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let original = main043_raw_file(
        &root.join("docs/evidence/source-reader-followups-20261016/sources.json"),
        152502,
        "08d9a996cbf1c0d2582d3ae1175398c46666e28ba47f65f8fd69fdebc3c05115",
    )
    .unwrap();
    let rows: serde_json::Value = serde_json::from_str(&original).unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 3);
    let matched = rows
        .iter()
        .filter(|row| row["path"] == SOURCE)
        .collect::<Vec<_>>();
    assert_eq!(matched.len(), 1);
    let row = matched[0];
    assert_eq!(row["archive"], "0.before.txt");
    let before_sha = row["sha256"].as_str().unwrap();
    assert_eq!(
        before_sha,
        "ac6850b7a3e872343f7ce3f8ad96d6f286b8d68ba0fcf998bf766982a4500abc"
    );
    let mut restored = main043_raw_file(
        &root.join("docs/evidence/source-reader-followups-20261016/0.before.txt"),
        4205,
        before_sha,
    )
    .unwrap();
    let edits = row["edits"].as_array().unwrap();
    assert_eq!(edits.len(), 11);
    let mut admitted = false;
    for (index, edit) in edits.iter().enumerate() {
        let old = edit["old"].as_str().unwrap();
        assert!(!old.is_empty());
        assert_eq!(restored.matches(old).count(), 1);
        restored = restored.replacen(old, edit["new"].as_str().unwrap(), 1);
        if index < 2 && restored == supplied {
            admitted = true;
        }
    }
    assert!(admitted, "complete original prefix bytes");
    assert_eq!(digest(restored.as_bytes()), main043_raw_pin(AFTER));
    let acquired = super::main_e177_final_inputs::registered_file(SOURCE, AFTER, OUTPUT_CAP)
        .expect("complete original followup frame from authenticated current source");
    assert_eq!(acquired, restored);
    acquired
}

#[test]
fn complete_receiver_followup_keeps_both_original_prefixes_and_refuses_changed_inputs() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let rows: serde_json::Value = serde_json::from_str(
        &main043_raw_file(
            &root.join("docs/evidence/source-reader-followups-20261016/sources.json"),
            152502,
            "08d9a996cbf1c0d2582d3ae1175398c46666e28ba47f65f8fd69fdebc3c05115",
        )
        .unwrap(),
    )
    .unwrap();
    let row = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["path"] == "src/source_reflows/semantic_receivers.rs")
        .unwrap();
    let after = main043_raw_file(
        &root.join("docs/evidence/ordinary-replay-routes-v1/src__source_reflows__semantic_receivers.rs.before.txt"),
        26937,
        "17989402984251c9956537bc3d2d8c6a1a6c6252cb1bd188ffd265dc6cdd0a51",
    ).unwrap();
    for index in [0, 1] {
        let prefix = row["edits"][index]["new"].as_str().unwrap();
        assert_eq!(complete_receiver_followup(prefix), after);
        assert!(std::panic::catch_unwind(|| complete_receiver_followup(
            &(prefix.to_owned() + "\n")
        ))
        .is_err());
        let changed = prefix.replacen(&prefix[..1], "!", 1);
        assert!(std::panic::catch_unwind(|| complete_receiver_followup(&changed)).is_err());
    }
    assert!(std::panic::catch_unwind(|| complete_receiver_followup("foreign source")).is_err());
}
