//! Authenticated test-only repository data. No compiler or budget authority.
//! Runtime files are required at the exact compile-time repository location.
#![cfg(test)]
use sha2::{Digest, Sha256};
use std::io::Read;
#[path = "runtime_json/sealed_io.rs"]
mod sealed_io;
pub(super) use sealed_io::{additional, AdditionalFixture};
use sealed_io::{digest, read_body, read_fixture, Failure, Spec, OUTPUT_CAP};

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
    // Bind both live channels before selecting this immutable RAW checkpoint.
    let live = main043_raw_file(
        std::path::Path::new(path),
        compiled.len(),
        &format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(compiled.as_bytes()))
        ),
    )
    .unwrap();
    assert_eq!(
        live, compiled,
        "complete latest physical and compiled RAW source"
    );
    let compiled_view = crate::source_reflows_merge::main_e177_final_inputs::value_direct(
        source, expected, compiled,
    );
    let physical =
        crate::source_reflows_merge::main_e177_final_inputs::value_direct(source, expected, &live);
    let compiled = compiled_view.as_str();
    assert_eq!(
        physical.len(),
        length,
        "complete historical RAW checkpoint length"
    );
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(physical.as_bytes()))
        ),
        expected
    );

    let held = main043_raw_file(std::path::Path::new(archive), length, expected).unwrap();
    assert_eq!(physical, compiled, "complete current compiled RAW source");
    assert_eq!(physical, held, "complete current RAW source archive");
    physical
}

// This first step binds the complete latest RAW channels before replaying
// the unchanged successor and checkpoint inverses below. It is not a logical
// source projection: the complete physical/compiled current image is required.
// The three unchanged ExitDirectory subjects were already explicit inputs
// of its original ten-owner audit. Keep their full compiled images available
// without opening the older RAW checkpoint subject registry.
fn main043_current_subject(source: &str) -> (&'static str, &'static str) {
    match source {
        "src/cleanup_plan/replay/expression_facts/receiving/remaining.rs" => (
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/cleanup_plan/replay/expression_facts/receiving/remaining.rs"
            ),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/cleanup_plan/replay/expression_facts/receiving/remaining.rs"
            )),
        ),
        "src/cleanup_plan/replay/expression_facts/receiving/remaining/selected.rs" => (
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/cleanup_plan/replay/expression_facts/receiving/remaining/selected.rs"
            ),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/cleanup_plan/replay/expression_facts/receiving/remaining/selected.rs"
            )),
        ),
        "src/cleanup_plan/replay/expression_facts/receiving/remaining/tests.rs" => (
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/cleanup_plan/replay/expression_facts/receiving/remaining/tests.rs"
            ),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/cleanup_plan/replay/expression_facts/receiving/remaining/tests.rs"
            )),
        ),
        _ => {
            let (path, compiled, _) = main043_raw_subject(source);
            (path, compiled)
        }
    }
}

fn main043_before_successors(
    source: &str,
    raw: &str,
    key: &str,
    outer: &mut Vec<serde_json::Value>,
) -> String {
    if !matches!(source,
        "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs"
        | "src/cleanup_plan/replay.rs" | "tests/hir_module_boundaries.rs"
        | "tests/workspace/semantic_graph/source_inventory.rs"
        | "src/cleanup_plan/replay/expression_facts/receiving/remaining/tests.rs") {
        return raw.to_owned();
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let data = main043_raw_file(
        &root.join("docs/evidence/source-coverage-254-20261024/latest-to-successors.json"),
        286968,
        "7746c771eaf205e67df886e43f027083ec1e61b4dae4380d16c3476e303f6734",
    )
    .unwrap();
    assert_eq!(
        data,
        include_str!("../../docs/evidence/source-coverage-254-20261024/latest-to-successors.json")
    );
    let spec: serde_json::Value = serde_json::from_str(&data).unwrap();
    assert_eq!(
        spec["schema"],
        "semaprax.main043.raw-checkpoint-successor.v1"
    );
    let rows = spec["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 5);
    let matching: Vec<_> = rows.iter().filter(|row| row["source"] == source).collect();
    assert_eq!(matching.len(), 1);
    let row = matching[0];
    let (path, compiled) = main043_current_subject(source);
    assert_eq!(
        raw, compiled,
        "complete latest supplied/compiled RAW source"
    );
    let physical = main043_raw_file(
        std::path::Path::new(path),
        usize::try_from(row["length"].as_u64().unwrap()).unwrap(),
        row["sha256"].as_str().unwrap(),
    )
    .unwrap();
    assert_eq!(
        physical, raw,
        "complete latest physical/compiled RAW source"
    );
    let previous_sha = row["checkpoint_sha256"].as_str().unwrap();
    let archive = row["archive"].as_str().unwrap();
    assert_eq!(archive, format!("e/{previous_sha}"));
    let previous = main043_raw_file(
        &root.join(archive),
        usize::try_from(row["checkpoint_length"].as_u64().unwrap()).unwrap(),
        previous_sha,
    )
    .unwrap();
    assert!(matches!(key, "after" | "new"));
    let mut restored = physical;
    for pair in row["inverse"].as_array().unwrap() {
        let after = pair[0].as_str().unwrap();
        let before = pair[1].as_str().unwrap();
        assert!(!after.is_empty());
        assert_eq!(
            restored.matches(after).count(),
            1,
            "unique latest RAW inverse"
        );
        restored = restored.replacen(after, before, 1);
        let mut edit = serde_json::Map::new();
        edit.insert(key.to_owned(), after.into());
        edit.insert("before".to_owned(), before.into());
        outer.push(edit.into());
    }
    assert_eq!(restored, previous, "whole immutable RAW checkpoint inverse");
    previous
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
    let checkpoints = main043_raw_file(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("docs/evidence/raw-checkpoints-20261021/checkpoint-inverses.json"),
        424887,
        "c0137c8c2a4f16e11ca4a385d57f764bae804b5170db0ae8a882bb7868546c41",
    )
    .unwrap();
    let checkpoints: serde_json::Value = serde_json::from_str(&checkpoints).unwrap();
    assert_eq!(checkpoints.as_array().unwrap().len(), 5);
    let successor = main043_raw_file(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("docs/evidence/ci-small-fixes-20261021/inventory-successor.json"),
        66998,
        "d64759367b95039add5633647e94066ae973c057584239b43a7701c09a7aee6a",
    )
    .unwrap();
    let successor: serde_json::Value = serde_json::from_str(&successor).unwrap();
    let replay_successor = main043_raw_file(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("docs/evidence/compact-byte-errors-20261021/replay-successor.json"),
        5646,
        "2788ad869b721f06ff00d2b7cb36b8e23dd9d86bd4f500ef7ee9dd4c33f1314c",
    )
    .unwrap();
    let replay_successor: serde_json::Value = serde_json::from_str(&replay_successor).unwrap();
    let successors = [successor, replay_successor];
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
        let (_, live, _) = main043_raw_subject(source);
        let mut live = main043_before_successors(source, live, key, &mut outer);
        for successor in successors.iter().filter(|row| row["source"] == source) {
            assert_eq!(live.len() as u64, successor["length"]);
            assert_eq!(
                format!(
                    "{:x}",
                    crate::digest_hex::LowerHex(Sha256::digest(live.as_bytes()))
                ),
                successor["sha256"]
            );
            for pair in successor["inverse"].as_array().unwrap() {
                let after = pair[0].as_str().unwrap();
                let before = pair[1].as_str().unwrap();
                assert!(!after.is_empty());
                assert_eq!(live.matches(after).count(), 1);
                live = live.replacen(after, before, 1);
                let mut edit = serde_json::Map::new();
                edit.insert(key.to_owned(), after.into());
                edit.insert("before".to_owned(), before.into());
                outer.push(serde_json::Value::Object(edit));
            }
            assert_eq!(live.len() as u64, successor["checkpoint_length"]);
            assert_eq!(
                format!(
                    "{:x}",
                    crate::digest_hex::LowerHex(Sha256::digest(live.as_bytes()))
                ),
                successor["checkpoint_sha256"]
            );
        }
        if live != current {
            let matches = checkpoints
                .as_array()
                .unwrap()
                .iter()
                .filter(|row| row["source"] == source)
                .collect::<Vec<_>>();
            assert_eq!(matches.len(), 1, "unique latest RAW checkpoint inverse");
            let checkpoint = matches[0];
            assert_eq!(live.len() as u64, checkpoint["length"]);
            assert_eq!(
                format!(
                    "{:x}",
                    crate::digest_hex::LowerHex(Sha256::digest(live.as_bytes()))
                ),
                checkpoint["sha256"]
            );
            assert_eq!(current.len() as u64, checkpoint["checkpoint_length"]);
            assert_eq!(
                format!(
                    "{:x}",
                    crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
                ),
                checkpoint["checkpoint_sha256"]
            );
            let mut restored = live.to_owned();
            for pair in checkpoint["inverse"].as_array().unwrap() {
                let after = pair[0].as_str().unwrap();
                let before = pair[1].as_str().unwrap();
                assert!(!after.is_empty());
                assert_eq!(restored.matches(after).count(), 1);
                restored = restored.replacen(after, before, 1);
                let mut edit = serde_json::Map::new();
                edit.insert(key.to_owned(), after.into());
                edit.insert("before".to_owned(), before.into());
                outer.push(serde_json::Value::Object(edit));
            }
            assert_eq!(restored, current, "complete latest-to-checkpoint inverse");
        }
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
        if name == "ExitDirectory" {
            // Replay every unchanged intermediate inverse before composing its
            // exact source-to-checkpoint step. The original rows remain below;
            // repeated full intermediate images need not inflate the wire data.
            let (path, compiled, _) = main043_raw_subject(source);
            let latest = main043_raw_file(
                std::path::Path::new(path),
                compiled.len(),
                &format!(
                    "{:x}",
                    crate::digest_hex::LowerHex(Sha256::digest(compiled.as_bytes()))
                ),
            )
            .unwrap();
            assert_eq!(latest, compiled, "fresh complete RAW composition source");
            let mut replayed = latest.clone();
            for edit in &outer {
                let after = edit[key].as_str().unwrap();
                assert!(!after.is_empty());
                assert_eq!(
                    replayed.matches(after).count(),
                    1,
                    "unique complete RAW composition"
                );
                replayed = replayed.replacen(after, edit["before"].as_str().unwrap(), 1);
            }
            assert_eq!(
                replayed, expected,
                "all original intermediate RAW steps executed"
            );
            let mut composed = serde_json::Map::new();
            composed.insert(key.to_owned(), latest.into());
            composed.insert("before".to_owned(), expected.into());
            let composed = vec![serde_json::Value::Object(composed)];
            if serde_json::to_string(&composed).unwrap().len()
                < serde_json::to_string(&outer).unwrap().len()
            {
                outer = composed;
            }
        }
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
    if name == "ExitDirectory" {
        // This original subject had no old outer front. Bind its complete
        // latest source, then retain and execute every unchanged old edit.
        let source = "src/cleanup_plan/replay/expression_facts/receiving/remaining/tests.rs";
        let index = rows
            .as_array()
            .unwrap()
            .iter()
            .position(|row| row["source"] == source)
            .unwrap();
        let row = &mut rows.as_array_mut().unwrap()[index];
        let old_steps = row["edits"].as_array().unwrap().clone();
        let (_, raw) = main043_current_subject(source);
        let mut outer = Vec::new();
        let mut restored = main043_before_successors(source, raw, key, &mut outer);
        for edit in old_steps.iter().rev() {
            let after = edit[key].as_str().unwrap();
            assert!(!after.is_empty());
            assert_eq!(restored.matches(after).count(), 1);
            restored = restored.replacen(after, edit["before"].as_str().unwrap(), 1);
        }
        assert_eq!(
            digest(restored.as_bytes()),
            main043_raw_pin(row["sha256"].as_str().unwrap())
        );
        row["edits"]
            .as_array_mut()
            .unwrap()
            .extend(outer.into_iter().rev());
        let mut original = row.clone();
        original["edits"] = old_steps.into();
        assert_eq!(
            original, before[index],
            "complete original unfronted RAW row"
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
    fn exit_directory_recipe_replays_every_complete_latest_raw_owner() {
        let metadata = main043_raw_metadata(super::super::main043_custody::Fixture::ExitDirectory);
        let rows: serde_json::Value = serde_json::from_str(&metadata).unwrap();
        for row in rows.as_array().unwrap() {
            let source = row["source"].as_str().unwrap();
            let (_, raw) = main043_current_subject(source);
            let mut restored = raw.to_owned();
            for (index, edit) in row["edits"].as_array().unwrap().iter().rev().enumerate() {
                let after = edit["after"].as_str().unwrap();
                assert_eq!(
                    restored.matches(after).count(),
                    1,
                    "complete RAW owner {source}, inverse step {index}, bytes {}",
                    after.len()
                );
                restored = restored.replacen(after, edit["before"].as_str().unwrap(), 1);
            }
            assert_eq!(
                digest(restored.as_bytes()),
                main043_raw_pin(row["sha256"].as_str().unwrap()),
                "{source}"
            );
        }
    }

    #[test]
    fn latest_raw_successor_binds_all_five_complete_live_channels_and_refuses_changed_inputs() {
        let sources = [
            "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs",
            "src/cleanup_plan/replay.rs",
            "tests/hir_module_boundaries.rs",
            "tests/workspace/semantic_graph/source_inventory.rs",
            "src/cleanup_plan/replay/expression_facts/receiving/remaining/tests.rs",
        ];
        for source in sources {
            let (_, current) = main043_current_subject(source);
            let mut edits = Vec::new();
            let previous = main043_before_successors(source, current, "after", &mut edits);
            assert!(!edits.is_empty());
            let mut restored = current.to_owned();
            for edit in &edits {
                let after = edit["after"].as_str().unwrap();
                assert_eq!(restored.matches(after).count(), 1);
                restored = restored.replacen(after, edit["before"].as_str().unwrap(), 1);
            }
            assert_eq!(restored, previous);
            assert_ne!(previous, current);
            for bad in [
                format!("{current}\n"),
                current[..current.len() - 1].to_owned(),
            ] {
                let mut refused = Vec::new();
                assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    main043_before_successors(source, &bad, "after", &mut refused)
                }))
                .is_err());
                assert!(
                    refused.is_empty(),
                    "no history published after live-source refusal"
                );
            }
        }
        let mut edits = Vec::new();
        assert_eq!(
            main043_before_successors("foreign.rs", "foreign", "after", &mut edits),
            "foreign"
        );
        assert!(edits.is_empty());
    }

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
// RUNTIME848_FIXED_RECEIVERS_BEGIN
// Two immutable test-runtime dependencies; no discovery, cache or new reader.
#[derive(Clone, Copy)]
pub(crate) enum ReceiverFixture {
    SelectedCalls,
    Main67107,
    // RUNTIME848_SIX_VARIANTS_BEGIN
    PaidLiveReuse,
    LexicalOperands,
    PrivateLayoutCapacity,
    RegistryScratch,
    ColdCanonical,
    BorrowedGeneric,
    // RUNTIME848_SIX_VARIANTS_END
    // RUNTIME848_THREE_VARIANTS_BEGIN
    RegionEntry,
    DiagnosticWriter,
    SignatureRewrite,
    // RUNTIME848_THREE_VARIANTS_END
}
impl ReceiverFixture {
    // RUNTIME848_THREE_SOURCE_BEGIN
    // This callback receives the complete compiled self-source, not a path or
    // canonical-row request. Authenticate it before the three exact transport
    // reversals and every unchanged original signature/source proof pipeline.
    pub(crate) fn signature_rewrite_source_before(current: &str) -> String {
        let prior = embedded_acquisition_source_before(
            "src/workspace_graph/expected_projection/type_rewrite/receiving.rs",
            current,
        )
        .expect("the signature self-source is a closed embedded consumer");
        let current = prior.as_str();
        assert_eq!(current.len(), 28277);
        assert_eq!(
            digest(current.as_bytes()),
            [
                198, 17, 126, 152, 25, 183, 254, 171, 26, 83, 8, 219, 9, 71, 3, 107, 176, 194, 125,
                45, 139, 98, 251, 184, 50, 47, 197, 221, 238, 199, 127, 102
            ]
        );
        let mut before = current.to_owned();
        for (after, original) in [
            (
                "        let rows: serde_json::Value =\n            serde_json::from_str(&crate::source_reflows_merge::receiver_fixture(\n                crate::source_reflows_merge::ReceiverFixture::SignatureRewrite,\n            ))\n            .unwrap();",
                "        let rows: serde_json::Value = serde_json::from_str(include_str!(concat!(\n            env!(\"CARGO_MANIFEST_DIR\"),\n            \"/docs/evidence/constructor-signature-rewrite-825/source-inverses.json\"\n        )))\n        .unwrap();",
            ),
            (
                "        let current_receiver =\n            crate::source_reflows_merge::ReceiverFixture::signature_rewrite_source_before(\n                include_str!(\"receiving.rs\"),\n            );\n",
                "",
            ),
            (
                "current_receiver.as_str()",
                "include_str!(concat!(\n                    env!(\"CARGO_MANIFEST_DIR\"),\n                    \"/src/workspace_graph/expected_projection/type_rewrite/receiving.rs\"\n                ))",
            ),
        ] {
            assert_eq!(before.matches(after).count(), 1);
            before = before.replacen(after, original, 1);
        }
        assert_eq!(before.len(), 28245);
        assert_eq!(
            digest(before.as_bytes()),
            [
                235, 243, 216, 43, 205, 12, 70, 195, 199, 184, 108, 43, 55, 194, 77, 30, 15, 148,
                46, 227, 254, 221, 174, 133, 214, 58, 237, 35, 120, 109, 199, 24
            ]
        );
        // The whole transport predecessor above is unchanged. Re-enter its
        // exact pre-formatter consumer stage before every original projector.
        let formatted = "            let projected = crate::source_reflows_merge::consumer_input(\n                \"signature825\",\n                row[\"source\"].as_str().unwrap(),\n                current,\n            );";
        let original = "            let projected = crate::source_reflows_merge::consumer_input(\"signature825\", row[\"source\"].as_str().unwrap(), current);";
        assert_eq!(before.matches(formatted).count(), 1);
        before = before.replacen(formatted, original, 1);
        assert_eq!(before.len(), 28182);
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(digest(before.as_bytes()))
            ),
            "b5eb131534d7bbd10d95aa2e8eeeafd3efa7d55fde34b5ab8fa0a24fbf396062"
        );
        before
    }
    // RUNTIME848_THREE_SOURCE_END
    fn spec(self) -> Spec {
        match self {
            // RUNTIME848_SIX_SPECS_BEGIN
            Self::PaidLiveReuse => Spec {
                path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/paid-live-pending-reuse-3c351/whole-source-inverses.json"),
                length: 3576034,
                sha256: [175, 88, 101, 138, 154, 38, 252, 9, 156, 230, 194, 230, 75, 76, 213, 24, 61, 243, 206, 29, 108, 169, 97, 95, 44, 25, 206, 12, 141, 45, 83, 251],
            },
            Self::LexicalOperands => Spec {
                path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/ordinary-lexical-return-operands-832/exact-source-inverses.json"),
                length: 2485760,
                sha256: [203, 244, 183, 123, 0, 154, 182, 125, 92, 251, 176, 112, 8, 30, 56, 131, 132, 45, 202, 131, 180, 69, 127, 109, 176, 70, 214, 65, 36, 223, 177, 243],
            },
            Self::PrivateLayoutCapacity => Spec {
                path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/private-layout-entry-capacity-834/exact-source-inverses.json"),
                length: 2435619,
                sha256: [24, 156, 92, 39, 33, 34, 138, 91, 21, 161, 118, 59, 160, 15, 255, 156, 35, 72, 243, 38, 204, 56, 42, 130, 111, 149, 170, 5, 114, 223, 222, 8],
            },
            Self::RegistryScratch => Spec {
                path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/registry-census-scratch-833/source-inverses.json"),
                length: 1887564,
                sha256: [224, 173, 90, 94, 217, 248, 71, 175, 109, 73, 51, 88, 246, 89, 174, 60, 88, 184, 158, 29, 152, 245, 12, 27, 153, 103, 24, 193, 57, 235, 3, 82],
            },
            Self::ColdCanonical => Spec {
                path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/canonical-comment-comparison-835/source-inverses.json"),
                length: 1821023,
                sha256: [61, 179, 82, 168, 172, 142, 181, 238, 74, 100, 67, 195, 156, 217, 135, 74, 119, 95, 129, 9, 166, 51, 53, 135, 37, 142, 41, 228, 120, 212, 166, 140],
            },
            Self::BorrowedGeneric => Spec {
                path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/borrowed-generic-type-rewrite-141387/whole-source-inverses.json"),
                length: 1498202,
                sha256: [71, 23, 207, 207, 19, 209, 138, 153, 88, 241, 15, 220, 76, 228, 59, 21, 77, 168, 152, 82, 242, 198, 111, 217, 192, 38, 52, 208, 27, 126, 215, 228],
            },
            // RUNTIME848_SIX_SPECS_END
            // RUNTIME848_THREE_SPECS_BEGIN
            Self::RegionEntry => Spec {
                path: concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/region-status-import-view-d4-20261015/runtime-views.json"
                ),
                length: 3841085,
                sha256: [
                    218, 167, 174, 164, 251, 148, 90, 158, 250, 93, 6, 233, 174, 206, 237, 156, 6,
                    92, 144, 192, 122, 218, 66, 73, 60, 119, 244, 243, 41, 189, 21, 199,
                ],
            },
            Self::DiagnosticWriter => Spec {
                path: concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/diagnostic-writer-846/source-inverses.json"
                ),
                length: 2446626,
                sha256: [
                    165, 242, 11, 99, 243, 34, 176, 86, 112, 114, 117, 252, 130, 218, 100, 219,
                    130, 146, 175, 189, 109, 65, 32, 181, 200, 188, 186, 108, 73, 86, 91, 104,
                ],
            },
            Self::SignatureRewrite => Spec {
                path: concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/constructor-signature-rewrite-825/source-inverses.json"
                ),
                length: 2353229,
                sha256: [
                    78, 171, 172, 57, 131, 198, 179, 97, 165, 22, 99, 105, 44, 105, 216, 167, 92,
                    221, 121, 110, 125, 156, 175, 164, 99, 160, 55, 91, 22, 173, 120, 35,
                ],
            },
            // RUNTIME848_THREE_SPECS_END
            Self::SelectedCalls => Spec {
                path: concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/selected-call-refusal-781/exact-source-inverses.json"
                ),
                length: 2614264,
                sha256: [
                    74, 253, 119, 191, 254, 162, 249, 198, 84, 36, 10, 18, 217, 177, 53, 7, 205,
                    95, 186, 130, 189, 110, 193, 196, 36, 139, 88, 217, 220, 125, 245, 209,
                ],
            },
            Self::Main67107 => Spec {
                path: concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/main67107-production-composition-20261017/receiver-main12.json"
                ),
                length: 1193191,
                sha256: [
                    39, 218, 14, 63, 48, 47, 133, 120, 85, 202, 13, 224, 212, 113, 135, 75, 199,
                    127, 92, 195, 76, 251, 33, 125, 206, 26, 103, 209, 20, 222, 136, 154,
                ],
            },
        }
    }
}
pub(crate) fn receiver_fixture(fixture: ReceiverFixture) -> String {
    let spec = fixture.spec();
    read_fixture(std::path::Path::new(spec.path), spec.length, spec.sha256)
        .expect("complete authenticated immutable receiver fixture")
}
// The original selected-call assertion audits its own complete source. Restore
// exactly the two transport edits before applying its unchanged original inverse.
pub(crate) fn receiver_fixture_source_before(source: &str, current: &str) -> String {
    // These fourteen exact entry images belong to the unchanged SelectedCalls
    // inverse packet. Acquire them only after current physical/compiled/archive
    // authentication; the original self-transport inverse below stays intact.
    let expected = match source {
        "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs" => Some("d764e8d6fbd373a2dbb40953baabf688fb4459562760fd5e1545cb9b73b9e6b0"),
        "src/cleanup_plan/replay/lexical_replay.rs" => Some("d0f2b7fcfc3815f0c1207c064d8bcb14d4323dff614d953988b962138ff975d0"),
        "src/cleanup_plan/replay/live_oracle/source_custody/b.rs" => Some("4d534b5433078d8d65ec9f09c7f763614a177cd743aa02465de64c023e509173"),
        "src/cleanup_plan/replay/scalar_replay.rs" => Some("6cbb996ba104a91029e7fb323ad2801c483d6f71b4b19ef10fd097a16f0c764a"),
        "src/cleanup_plan/replay/selected_entry/tests/session_errors.rs" => Some("2b27003287b4e325ed126b4e26f9f933fc039e42f66739242b63759d3d713b05"),
        "src/cleanup_plan/replay/semantic_refusal.rs" => Some("fc1a6f41a225e0936ede78ff9923eb0eea1e93ce5c8e37fbde9709e6ee78c823"),
        "src/cleanup_plan/replay/semantic_refusal/tests.rs" => Some("aca69e92356ddfb4d7bbb774ca647a981cf86d402d9b0dff71108adef777b1ad"),
        "src/cleanup_plan/replay/structural_control.rs" => Some("84c9d537b31a2f24e0f4c977ea0341bf4a4a6a738c299f5e7e35976a4946a037"),
        "src/cleanup_plan/replay/structural_control/blocks.rs" => Some("be348bfc7e713d79dccb11d6022a7371a3419390703aac7e667ae613f591b725"),
        "src/cleanup_plan/replay/structural_control/blocks_before_receipt.rs" => Some("93a5af9fc855967b0fd48573c0dd7ff8075ef63cda3ab8e9a28787d6be47ea04"),
        "src/cleanup_plan/replay/structural_control/calls/custody_tests.rs" => Some("55245329e3a803f31cbdf5488c4068934f1b74ac03cfca84bbf5eb6d93f53dc7"),
        "src/cleanup_plan/replay/structural_control/calls.rs" => Some("17f757fd22a55b7aaaca952c0ab84b7c8d45bf9285fa15a0a4032ecfbdaa5000"),
        "tests/hir_module_boundaries.rs" => Some("055a8f7ddd8b9f5b2d12122154de053a0a6823b75c298580cd4770628c4e8851"),
        "tests/workspace/semantic_graph/source_inventory.rs" => Some("4a1920f6abdff90036fe9afabe687c091b08a38d4f26a28e2e7c7feb678cbd14"),
        _ => None,
    };
    if let Some(expected) = expected {
        let entry = super::main_e177_final_inputs::value_direct(source, expected, current);
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(digest(entry.as_bytes()))
            ),
            expected,
            "complete SelectedCalls entry: {source}"
        );
        return entry;
    }
    if source != "src/cleanup_plan/replay/structural_control/calls/custody_tests/source.rs" {
        return current.to_owned();
    }
    assert_eq!(current.len(), 7873);
    assert_eq!(
        digest(current.as_bytes()),
        [
            22, 25, 246, 163, 6, 93, 58, 87, 222, 52, 59, 43, 203, 215, 114, 78, 99, 244, 200, 168,
            70, 222, 252, 141, 0, 68, 117, 28, 248, 37, 90, 5
        ]
    );
    assert_eq!(current.matches("        crate::source_reflows_merge::receiver_fixture_source_before(\n            \"src/cleanup_plan/replay/structural_control/blocks_before_receipt.rs\",\n            include_str!(\"../../blocks_before_receipt.rs\"),\n        ).replacen(").count(), 1);
    let prior = current.replacen("        crate::source_reflows_merge::receiver_fixture_source_before(\n            \"src/cleanup_plan/replay/structural_control/blocks_before_receipt.rs\",\n            include_str!(\"../../blocks_before_receipt.rs\"),\n        ).replacen(", "        include_str!(\"../../blocks_before_receipt.rs\").replacen(", 1);
    assert_eq!(prior.len(), 7706);
    assert_eq!(
        digest(prior.as_bytes()),
        [
            131, 224, 221, 224, 214, 14, 199, 238, 82, 83, 218, 38, 95, 37, 112, 115, 28, 115, 130,
            137, 146, 83, 41, 253, 48, 76, 196, 9, 53, 64, 39, 52
        ]
    );
    let current = prior.as_str();
    let mut before = current.to_owned();
    assert_eq!(before.matches("    let rows: serde_json::Value =\n        serde_json::from_str(&crate::source_reflows_merge::receiver_fixture(\n            crate::source_reflows_merge::ReceiverFixture::SelectedCalls,\n        ))\n        .unwrap();").count(), 1, "exact runtime fixture transport edit");
    before = before.replacen("    let rows: serde_json::Value =\n        serde_json::from_str(&crate::source_reflows_merge::receiver_fixture(\n            crate::source_reflows_merge::ReceiverFixture::SelectedCalls,\n        ))\n        .unwrap();", "    let rows: serde_json::Value = serde_json::from_str(include_str!(\n        \"../../../../../../docs/evidence/selected-call-refusal-781/exact-source-inverses.json\"\n    ))\n    .unwrap();", 1);
    assert_eq!(before.matches("        let mut current = crate::source_reflows_merge::receiver_fixture_source_before(name, source);").count(), 1, "exact runtime fixture transport edit");
    before = before.replacen("        let mut current = crate::source_reflows_merge::receiver_fixture_source_before(name, source);", "        let mut current = source.to_owned();", 1);
    assert_eq!(
        digest(before.as_bytes()),
        [
            122, 25, 222, 23, 184, 16, 230, 100, 106, 120, 205, 160, 9, 162, 204, 93, 38, 84, 161,
            101, 31, 187, 150, 206, 28, 92, 42, 51, 124, 152, 232, 137
        ],
        "complete original selected-call source"
    );
    before
}

// EMBEDDED848_THREE_CURRENT_BEGIN
// Only the three evaluated acquisitions were restored. Authenticate each whole
// compiled consumer before reconstructing its complete runtime-source stage;
// every original transport inverse and historical assertion still follows.
fn embedded_acquisition_source_before(source: &str, current: &str) -> Option<String> {
    let (length, sha, old_length, old_sha, embedded, runtime): (
        usize,
        [u8; 32],
        usize,
        [u8; 32],
        &str,
        &str,
    ) = match source {
        "src/cleanup_plan/replay/expression_facts/receiving/regions_entry/tests.rs" => (
            30436,
            [
                237, 115, 207, 242, 72, 24, 228, 75, 198, 221, 208, 179, 60, 28, 61, 102, 143, 8,
                11, 1, 245, 253, 222, 0, 14, 31, 63, 135, 113, 217, 167, 142,
            ],
            30430,
            [
                6, 118, 157, 69, 85, 225, 141, 91, 170, 251, 173, 54, 35, 215, 41, 0, 76, 10, 246,
                17, 113, 52, 40, 221, 41, 47, 112, 23, 206, 255, 53, 244,
            ],
            "    let rows: serde_json::Value = serde_json::from_str(include_str!(concat!(\n        env!(\"CARGO_MANIFEST_DIR\"),\n        \"/docs/evidence/region-status-import-view-d4-20261015/runtime-views.json\"\n    )))\n    .unwrap();",
            "    let rows: serde_json::Value =\n        serde_json::from_str(&crate::source_reflows_merge::receiver_fixture(\n            crate::source_reflows_merge::ReceiverFixture::RegionEntry,\n        ))\n        .unwrap();",
        ),
        "src/diagnostic/writer/tests.rs" => (
            12183,
            [
                155, 180, 211, 126, 240, 24, 153, 35, 56, 69, 157, 144, 92, 117, 88, 31, 126, 66,
                20, 135, 80, 136, 13, 252, 139, 246, 134, 243, 251, 103, 100, 240,
            ],
            12196,
            [
                190, 196, 111, 129, 149, 10, 242, 87, 98, 82, 198, 98, 135, 70, 2, 108, 198, 53,
                196, 200, 24, 87, 21, 100, 62, 176, 202, 77, 21, 123, 146, 235,
            ],
            "    let rows: serde_json::Value = serde_json::from_str(include_str!(concat!(\n        env!(\"CARGO_MANIFEST_DIR\"),\n        \"/docs/evidence/diagnostic-writer-846/source-inverses.json\"\n    )))\n    .unwrap();",
            "    let rows: serde_json::Value =\n        serde_json::from_str(&crate::source_reflows_merge::receiver_fixture(\n            crate::source_reflows_merge::ReceiverFixture::DiagnosticWriter,\n        ))\n        .unwrap();",
        ),
        "src/workspace_graph/expected_projection/type_rewrite/receiving.rs" => (
            28276,
            [
                22, 221, 107, 83, 203, 128, 52, 44, 220, 161, 220, 11, 112, 61, 14, 175, 0, 10,
                191, 237, 97, 47, 8, 152, 187, 200, 254, 55, 207, 140, 29, 71,
            ],
            28277,
            [
                198, 17, 126, 152, 25, 183, 254, 171, 26, 83, 8, 219, 9, 71, 3, 107, 176, 194, 125,
                45, 139, 98, 251, 184, 50, 47, 197, 221, 238, 199, 127, 102,
            ],
            "        let rows: serde_json::Value = serde_json::from_str(include_str!(concat!(\n            env!(\"CARGO_MANIFEST_DIR\"),\n            \"/docs/evidence/constructor-signature-rewrite-825/source-inverses.json\"\n        )))\n        .unwrap();",
            "        let rows: serde_json::Value =\n            serde_json::from_str(&crate::source_reflows_merge::receiver_fixture(\n                crate::source_reflows_merge::ReceiverFixture::SignatureRewrite,\n            ))\n            .unwrap();",
        ),
        _ => return None,
    };
    assert_eq!(
        current.len(),
        length,
        "complete embedded consumer extent: {source}"
    );
    assert_eq!(
        digest(current.as_bytes()),
        sha,
        "complete embedded consumer body: {source}"
    );
    assert_eq!(
        current.matches(embedded).count(),
        1,
        "unique acquisition restoration: {source}"
    );
    let before = current.replacen(embedded, runtime, 1);
    assert_eq!(
        before.len(),
        old_length,
        "complete runtime consumer extent: {source}"
    );
    assert_eq!(
        digest(before.as_bytes()),
        old_sha,
        "complete runtime consumer body: {source}"
    );
    Some(before)
}
// EMBEDDED848_THREE_CURRENT_END
