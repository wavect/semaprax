//! Complete-input and complete-predecessor checks for this private representation.
use sha2::{Digest, Sha256};

const ORIGINAL_FILES: [&str; 7] = [
    "ee6582dfe9654e0d405cd103834e5ceb501c87c60d4b676ccde829fb4801329f",
    "e9c4c72bcfad51ee7936c3e4b302c4440ea6862c4b771bcfc3202885b63ebbb7",
    "ca11b94661cd07264ef2b6da61f2dca28ac70aac3ffc4a362395c12e780beb7c",
    "a37b4da6cef36a9249dd30031bab14a710c32cea7b259159983978f390529643",
    "603d7c7bfcbc1d6163b7395f048b7421665539806d7643cc0d9f38b61e48bfc6",
    "0fe29b4e9a4c1aea7e6ff38c608e4bc3f8b82c4c3c3a4484f35cf7f2490842bb",
    "401c904b0941c4dc7e4677936366d23375d7503944692061ee024f9278d40cb1",
];
const ORIGINAL_PIECES: [&str; 7] = [
    "1c3e01317ed79996b443dfce899ca94939202d86079086adfbe0c36b6c5f265e",
    "de4442b11d64eef0e1a61c79cb9b549975e4e47a7ff8a83dab3aa0ee37e4c0b3",
    "19ba460b3183b3e0bf50a7eba8b00ea3eb3985fa9bcb01dda9c0fb6374a6de2e",
    "80a1ceef3baca52945b8f897cc30573c911e9250e4a4e978b96e1615b8f885cd",
    "9ded3679c3b4777b01ea97bb13cb88c77fb96db296579d69e9a942ff93318b58",
    "fc722eea9f308a5ccae38965e33eeee30f426173c6fea9cb8a364238049ee2e8",
    "450f9163ddb216c72ebb0f193eea2732869762b45eb9466867241f041a015674",
];

// Every input is fixed at compile time; the check needs no runtime authority.
macro_rules! source {
    ($path:literal) => {
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/", $path))
    };
}
const FILES: [(&str, &str, &str, &str); 7] = [
    (
        "src/cleanup_plan/replay.rs",
        source!("src/cleanup_plan/replay.rs"),
        "src__cleanup_plan__replay.rs.before.txt",
        source!("docs/evidence/compact-staged-observations-v1/src__cleanup_plan__replay.rs.before.txt"),
    ),
    (
        "src/cleanup_plan/replay/branch_clone/tests.rs",
        source!("src/cleanup_plan/replay/branch_clone/tests.rs"),
        "src__cleanup_plan__replay__branch_clone__tests.rs.before.txt",
        source!("docs/evidence/compact-staged-observations-v1/src__cleanup_plan__replay__branch_clone__tests.rs.before.txt"),
    ),
    (
        "src/cleanup_plan/replay/factored.rs",
        source!("src/cleanup_plan/replay/factored.rs"),
        "src__cleanup_plan__replay__factored.rs.before.txt",
        source!("docs/evidence/compact-staged-observations-v1/src__cleanup_plan__replay__factored.rs.before.txt"),
    ),
    (
        "src/cleanup_plan/replay/factored/borrowed_canonical/dictionary/keys.rs",
        source!("src/cleanup_plan/replay/factored/borrowed_canonical/dictionary/keys.rs"),
        "src__cleanup_plan__replay__factored__borrowed_canonical__dictionary__keys.rs.before.txt",
        source!("docs/evidence/compact-staged-observations-v1/src__cleanup_plan__replay__factored__borrowed_canonical__dictionary__keys.rs.before.txt"),
    ),
    (
        "src/cleanup_plan/replay/factored/borrowed_canonical/tests.rs",
        source!("src/cleanup_plan/replay/factored/borrowed_canonical/tests.rs"),
        "src__cleanup_plan__replay__factored__borrowed_canonical__tests.rs.before.txt",
        source!("docs/evidence/compact-staged-observations-v1/src__cleanup_plan__replay__factored__borrowed_canonical__tests.rs.before.txt"),
    ),
    (
        "tests/hir_module_boundaries.rs",
        source!("tests/hir_module_boundaries.rs"),
        "tests__hir_module_boundaries.rs.before.txt",
        source!("docs/evidence/compact-staged-observations-v1/tests__hir_module_boundaries.rs.before.txt"),
    ),
    (
        "tests/workspace/semantic_graph/source_inventory.rs",
        source!("tests/workspace/semantic_graph/source_inventory.rs"),
        "tests__workspace__semantic_graph__source_inventory.rs.before.txt",
        source!("docs/evidence/compact-staged-observations-v1/tests__workspace__semantic_graph__source_inventory.rs.before.txt"),
    ),
];

const PACKET: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/evidence/compact-staged-observations-v1/inverse.json"
));
fn digest(text: &str) -> String {
    format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
    )
}
fn invert(record: &serde_json::Value, current: &str) -> String {
    assert_eq!(
        digest(current),
        record["after_sha256"],
        "{}",
        record["path"]
    );
    let mut restored = current.to_owned();
    for edit in record["inverse"].as_array().unwrap() {
        let after = edit["after"].as_str().unwrap();
        assert_eq!(restored.matches(after).count(), 1, "{}", record["path"]);
        restored = restored.replacen(after, edit["before"].as_str().unwrap(), 1);
    }
    assert_eq!(
        digest(&restored),
        record["before_sha256"],
        "{}",
        record["path"]
    );
    restored
}
pub(in crate::cleanup_plan::replay) fn before_root(current: &str) -> String {
    let joined = crate::source_reflows_merge::main_b80_input("src/cleanup_plan/replay.rs", current);
    let current = joined.as_str();
    let records: serde_json::Value = serde_json::from_str(PACKET).unwrap();
    let record = records
        .as_array()
        .unwrap()
        .iter()
        .find(|record| record["path"] == "src/cleanup_plan/replay.rs")
        .unwrap();
    assert_eq!(record["before_sha256"], ORIGINAL_FILES[0]);
    invert(record, current)
}

#[test]
fn all_seven_live_receivers_reconstruct_every_predecessor_byte() {
    let records: serde_json::Value = serde_json::from_str(PACKET).unwrap();
    assert_eq!(records.as_array().unwrap().len(), 7);
    for (index, record) in records.as_array().unwrap().iter().enumerate() {
        assert_eq!(record["before_sha256"], ORIGINAL_FILES[index]);
        let (path, current, archive_path, archive) = FILES[index];
        assert_eq!(record["path"], path);
        assert_eq!(record["archive"], archive_path);
        let current = crate::source_reflows_merge::main_b80_input(path, current);
        let current = current.as_str();
        let restored = invert(record, current);
        assert_eq!(restored, archive);
        let mut changed = current.to_owned();
        changed.push(' ');
        assert!(std::panic::catch_unwind(|| invert(record, &changed)).is_err());
        let mut forged = record.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| invert(&forged, current)).is_err());
    }
}

#[test]
fn original_enumerator_enum_header_traits_and_lexical_helper_stay_complete() {
    let pieces: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/compact-staged-observations-v1/original-pieces.json"
    )))
    .unwrap();
    assert_eq!(pieces.as_array().unwrap().len(), 7);
    for (index, piece) in pieces.as_array().unwrap().iter().enumerate() {
        assert_eq!(piece["before_sha256"], ORIGINAL_PIECES[index]);
        let old = piece["body"].as_str().unwrap();
        assert_eq!(digest(old), piece["before_sha256"]);
        let path = piece["compiled_path"].as_str().unwrap();
        let current = match path {
            "src/cleanup_plan/replay/compact_stage/before.rs" => include_str!("before.rs"),
            "src/cleanup_plan/replay/compact_stage/lexical_before.rs" => {
                include_str!("lexical_before.rs")
            }
            _ => panic!("unregistered original producer input"),
        };
        let current = crate::source_reflows_merge::main_b80_input(path, current);
        let current = current.as_str();
        let compiled = piece["compiled_body"].as_str().unwrap();
        assert_eq!(current.matches(compiled).count(), 1, "{}", piece["name"]);
        let mut restored = compiled.to_owned();
        for edit in piece["inverse"].as_array().unwrap() {
            let after = edit["after"].as_str().unwrap();
            assert_eq!(restored.matches(after).count(), 1);
            restored = restored.replacen(after, edit["before"].as_str().unwrap(), 1);
        }
        assert_eq!(restored, old);
    }
}
