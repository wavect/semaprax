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
    ("src/cleanup_plan/replay.rs") => { include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/future831-f247-final-source-binding-20261017/incoming/e9d061abd4812771d5357a77182d490d96a5f0768f2c302f29b6dafe34f8c1e1.txt")) };
    ("docs/evidence/compact-staged-observations-v1/src__cleanup_plan__replay.rs.before.txt") => { include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/future831-f247-final-source-binding-20261017/incoming/ee6582dfe9654e0d405cd103834e5ceb501c87c60d4b676ccde829fb4801329f.txt")) };
    ("src/cleanup_plan/replay/branch_clone/tests.rs") => { include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/future831-f247-final-source-binding-20261017/incoming/1f2b5f306c51e9eba09c1133b47eec2f13ad168121533b98ee4f77fcf4bb5321.txt")) };
    ("docs/evidence/compact-staged-observations-v1/src__cleanup_plan__replay__branch_clone__tests.rs.before.txt") => { include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/future831-f247-final-source-binding-20261017/incoming/e9c4c72bcfad51ee7936c3e4b302c4440ea6862c4b771bcfc3202885b63ebbb7.txt")) };
    ("src/cleanup_plan/replay/factored.rs") => { include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/future831-f247-final-source-binding-20261017/incoming/7fe20afb729d15e4ab903bcc741c119f269b748fad0365d6271d609c64b5f94a.txt")) };
    ("docs/evidence/compact-staged-observations-v1/src__cleanup_plan__replay__factored.rs.before.txt") => { include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/future831-f247-final-source-binding-20261017/incoming/ca11b94661cd07264ef2b6da61f2dca28ac70aac3ffc4a362395c12e780beb7c.txt")) };
    ("src/cleanup_plan/replay/factored/borrowed_canonical/dictionary/keys.rs") => { include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/future831-f247-final-source-binding-20261017/incoming/01b3e408874930f2aad015d20cf8317fc1507183f92aae740d11985b99d223e1.txt")) };
    ("docs/evidence/compact-staged-observations-v1/src__cleanup_plan__replay__factored__borrowed_canonical__dictionary__keys.rs.before.txt") => { include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/future831-f247-final-source-binding-20261017/incoming/a37b4da6cef36a9249dd30031bab14a710c32cea7b259159983978f390529643.txt")) };
    ("src/cleanup_plan/replay/factored/borrowed_canonical/tests.rs") => { include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/future831-f247-final-source-binding-20261017/incoming/c9c42329ff2e2ee3802de78f0da6484b42a7c58ae6636532024c4e955b743033.txt")) };
    ("docs/evidence/compact-staged-observations-v1/src__cleanup_plan__replay__factored__borrowed_canonical__tests.rs.before.txt") => { include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/future831-f247-final-source-binding-20261017/incoming/603d7c7bfcbc1d6163b7395f048b7421665539806d7643cc0d9f38b61e48bfc6.txt")) };
    ("tests/hir_module_boundaries.rs") => { include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/future831-f247-final-source-binding-20261017/incoming/2a8c92d82704c56d18e15da4912e839be52e96114dbd9d2562fdb67cce303577.txt")) };
    ("docs/evidence/compact-staged-observations-v1/tests__hir_module_boundaries.rs.before.txt") => { include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/future831-f247-final-source-binding-20261017/incoming/0fe29b4e9a4c1aea7e6ff38c608e4bc3f8b82c4c3c3a4484f35cf7f2490842bb.txt")) };
    ("tests/workspace/semantic_graph/source_inventory.rs") => { include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/future831-f247-final-source-binding-20261017/incoming/75534a389c06ec3f4e28e6504303bf155b51062b46e5a76e2c80a255a5e56aa9.txt")) };
    ("docs/evidence/compact-staged-observations-v1/tests__workspace__semantic_graph__source_inventory.rs.before.txt") => { include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/future831-f247-final-source-binding-20261017/incoming/401c904b0941c4dc7e4677936366d23375d7503944692061ee024f9278d40cb1.txt")) };
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

fn packet() -> &'static str {
    static PACKET: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    PACKET.get_or_init(|| f247_read("docs/evidence/compact-staged-observations-v1/inverse.json")).as_str()
}
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
    let authenticated = crate::source_reflows_merge::main_c883_inputs::f247_expected(
        "src/cleanup_plan/replay.rs", current,
    );
    let current = authenticated.as_str();
    let records: serde_json::Value = serde_json::from_str(packet()).unwrap();
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
    authenticate_compiled_inputs();
    let records: serde_json::Value = serde_json::from_str(packet()).unwrap();
    assert_eq!(records.as_array().unwrap().len(), 7);
    for (index, record) in records.as_array().unwrap().iter().enumerate() {
        assert_eq!(record["before_sha256"], ORIGINAL_FILES[index]);
        let (path, current, archive_path, archive) = FILES[index];
        assert_eq!(record["path"], path);
        assert_eq!(record["archive"], archive_path);
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
    authenticate_compiled_inputs();
    let pieces: serde_json::Value = serde_json::from_str(&f247_read("docs/evidence/compact-staged-observations-v1/original-pieces.json"))
    .unwrap();
    assert_eq!(pieces.as_array().unwrap().len(), 7);
    for (index, piece) in pieces.as_array().unwrap().iter().enumerate() {
        assert_eq!(piece["before_sha256"], ORIGINAL_PIECES[index]);
        let old = piece["body"].as_str().unwrap();
        assert_eq!(digest(old), piece["before_sha256"]);
        let current = match piece["compiled_path"].as_str().unwrap() {
            "src/cleanup_plan/replay/compact_stage/before.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/future831-f247-final-source-binding-20261017/incoming/4d10cbe619d26918551f006a29055551fbd0e1020387ac881848aa8d6f9719f6.txt")),
            "src/cleanup_plan/replay/compact_stage/lexical_before.rs" => {
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/future831-f247-final-source-binding-20261017/incoming/ce639daa60fcb96e43d19d48d9c9f0e5e8eb22b34488604c48e12e55056bd889.txt"))
            }
            _ => panic!("unregistered original producer input"),
        };
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

fn f247_read(path: &str) -> String {
    crate::source_reflows_merge::main_c883_inputs::read_f247(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path),
    ).unwrap()
}
fn authenticate_compiled_inputs() {
    assert_eq!(f247_read("src/cleanup_plan/replay.rs"), source!("src/cleanup_plan/replay.rs"));
    assert_eq!(f247_read("docs/evidence/compact-staged-observations-v1/src__cleanup_plan__replay.rs.before.txt"), source!("docs/evidence/compact-staged-observations-v1/src__cleanup_plan__replay.rs.before.txt"));
    assert_eq!(f247_read("src/cleanup_plan/replay/branch_clone/tests.rs"), source!("src/cleanup_plan/replay/branch_clone/tests.rs"));
    assert_eq!(f247_read("docs/evidence/compact-staged-observations-v1/src__cleanup_plan__replay__branch_clone__tests.rs.before.txt"), source!("docs/evidence/compact-staged-observations-v1/src__cleanup_plan__replay__branch_clone__tests.rs.before.txt"));
    assert_eq!(f247_read("src/cleanup_plan/replay/factored.rs"), source!("src/cleanup_plan/replay/factored.rs"));
    assert_eq!(f247_read("docs/evidence/compact-staged-observations-v1/src__cleanup_plan__replay__factored.rs.before.txt"), source!("docs/evidence/compact-staged-observations-v1/src__cleanup_plan__replay__factored.rs.before.txt"));
    assert_eq!(f247_read("src/cleanup_plan/replay/factored/borrowed_canonical/dictionary/keys.rs"), source!("src/cleanup_plan/replay/factored/borrowed_canonical/dictionary/keys.rs"));
    assert_eq!(f247_read("docs/evidence/compact-staged-observations-v1/src__cleanup_plan__replay__factored__borrowed_canonical__dictionary__keys.rs.before.txt"), source!("docs/evidence/compact-staged-observations-v1/src__cleanup_plan__replay__factored__borrowed_canonical__dictionary__keys.rs.before.txt"));
    assert_eq!(f247_read("src/cleanup_plan/replay/factored/borrowed_canonical/tests.rs"), source!("src/cleanup_plan/replay/factored/borrowed_canonical/tests.rs"));
    assert_eq!(f247_read("docs/evidence/compact-staged-observations-v1/src__cleanup_plan__replay__factored__borrowed_canonical__tests.rs.before.txt"), source!("docs/evidence/compact-staged-observations-v1/src__cleanup_plan__replay__factored__borrowed_canonical__tests.rs.before.txt"));
    assert_eq!(f247_read("tests/hir_module_boundaries.rs"), source!("tests/hir_module_boundaries.rs"));
    assert_eq!(f247_read("docs/evidence/compact-staged-observations-v1/tests__hir_module_boundaries.rs.before.txt"), source!("docs/evidence/compact-staged-observations-v1/tests__hir_module_boundaries.rs.before.txt"));
    assert_eq!(f247_read("tests/workspace/semantic_graph/source_inventory.rs"), source!("tests/workspace/semantic_graph/source_inventory.rs"));
    assert_eq!(f247_read("docs/evidence/compact-staged-observations-v1/tests__workspace__semantic_graph__source_inventory.rs.before.txt"), source!("docs/evidence/compact-staged-observations-v1/tests__workspace__semantic_graph__source_inventory.rs.before.txt"));
    assert_eq!(f247_read("src/cleanup_plan/replay/compact_stage/before.rs"), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/future831-f247-final-source-binding-20261017/incoming/4d10cbe619d26918551f006a29055551fbd0e1020387ac881848aa8d6f9719f6.txt")));
    assert_eq!(f247_read("src/cleanup_plan/replay/compact_stage/lexical_before.rs"), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/future831-f247-final-source-binding-20261017/incoming/ce639daa60fcb96e43d19d48d9c9f0e5e8eb22b34488604c48e12e55056bd889.txt")));
    assert_eq!(before_root(FILES[0].1), FILES[0].3);
}
