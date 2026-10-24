//! Exact new-source acquisition before every unchanged historical raw guard.
use sha2::{Digest, Sha256};
fn digest(text: &str) -> String {
    format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
    )
}
const PACKET_LENGTH: usize = 784669; // sealed by the source binding recipe
const PACKET_SHA256: &str = "a5d995cad4d5ecff6ea81eb310580a9a2caaf63150718d4992726cb99b877278";
fn records() -> serde_json::Value {
    let packet = super::runtime_json::additional(
        super::runtime_json::AdditionalFixture::OrdinaryPlanHistory,
    );
    assert!(
        packet.len() <= 2 * 1024 * 1024,
        "fixed custody packet exceeds cap"
    );
    assert_eq!(packet.len(), PACKET_LENGTH, "whole custody packet length");
    assert_eq!(
        digest(&packet),
        PACKET_SHA256,
        "whole custody packet digest"
    );
    serde_json::from_str(&packet).unwrap()
}
const CURRENT: [(&str, &str); 6] = [
    (
        "src/cleanup_plan/replay.rs",
        include_str!("../cleanup_plan/replay.rs"),
    ),
    (
        "src/source_reflows_merge.rs",
        include_str!("../source_reflows_merge.rs"),
    ),
    (
        "src/source_reflows_merge/main67107_inputs.rs",
        include_str!("main67107_inputs.rs"),
    ),
    (
        "tests/hir_module_boundaries.rs",
        include_str!("../../tests/hir_module_boundaries.rs"),
    ),
    (
        "tests/workspace/semantic_graph/source_inventory.rs",
        include_str!("../../tests/workspace/semantic_graph/source_inventory.rs"),
    ),
    (
        "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs",
        include_str!(
            "../../crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs"
        ),
    ),
];
fn restore(row: &serde_json::Value, current: &str) -> String {
    assert_eq!(
        digest(current),
        row["after_sha256"],
        "whole ordinary plan source input"
    );
    let mut result = current.to_owned();
    for edit in row["inverse"].as_array().unwrap() {
        let new = edit["after"].as_str().unwrap();
        assert_eq!(
            result.matches(new).count(),
            1,
            "unique complete source inverse"
        );
        result = result.replacen(new, edit["before"].as_str().unwrap(), 1);
    }
    assert_eq!(digest(&result), row["before_sha256"]);
    assert_eq!(result, row["before"].as_str().unwrap());
    result
}
pub(super) fn acquire(source: &str, current: &str) -> String {
    if !matches!(
        source,
        "src/cleanup_plan/replay.rs"
            | "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs"
            | "src/source_reflows_merge.rs"
            | "src/source_reflows_merge/main67107_inputs.rs"
            | "tests/hir_module_boundaries.rs"
            | "tests/workspace/semantic_graph/source_inventory.rs"
    ) {
        return current.to_owned();
    }
    let rows = records();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 6);
    let matching: Vec<_> = rows.iter().filter(|row| row["path"] == source).collect();
    assert!(matching.len() <= 1);
    let Some(row) = matching.first() else {
        return current.to_owned();
    };
    // Only this exact successor is acquired here. Other historical inputs keep
    // their unchanged expected-input owner and strict downstream byte guard.
    if digest(current) != row["after_sha256"] {
        return current.to_owned();
    }
    restore(row, current)
}
#[test]
fn complete_originals_inverse_and_foreign_inputs_remain_independent() {
    let rows = records();
    for row in rows.as_array().unwrap() {
        let path = row["path"].as_str().unwrap();
        let current = CURRENT.iter().find(|(name, _)| *name == path).unwrap().1;
        assert!(current.len() <= 2 * 1024 * 1024);
        // Acquire the complete integrated physical owner before exercising the
        // unchanged complete legacy assertions below on its exact old stage.
        let current = legacy_checkpoint(path, row["before_sha256"].as_str().unwrap(), current);
        let before = restore(row, &current);
        assert_eq!(acquire(path, &current), before);
        assert_eq!(acquire(path, &before), before);
        let bad = format!("{current} ");
        assert!(std::panic::catch_unwind(|| restore(row, &bad)).is_err());
        let mut forged = row.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| restore(&forged, &current)).is_err());
        assert_eq!(acquire("foreign", &current), current);
    }
}

// Distinct expected namespace for the finite integrated six-source successor.
// The original packet, restore and acquire above remain independent controls.
const INTEGRATED_LENGTH: usize = 1539271;
const INTEGRATED_SHA256: &str = "da99e3b9599f64e879ba43646aef4b69f927f566b25007c96cd6d0c932a815bd";
fn integrated_records() -> serde_json::Value {
    let integrated = super::runtime_json::additional(
        super::runtime_json::AdditionalFixture::OrdinaryPlanIntegrated,
    );
    assert!(integrated.len() <= 2 * 1024 * 1024);
    assert_eq!(integrated.len(), INTEGRATED_LENGTH);
    assert_eq!(digest(&integrated), INTEGRATED_SHA256);
    let rows: serde_json::Value = serde_json::from_str(&integrated).unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 6);
    rows
}
fn edge(row: &serde_json::Value, text: &str, name: &str) -> String {
    let edge = &row[name];
    assert_eq!(
        digest(text),
        edge["after_sha256"],
        "whole integrated source edge"
    );
    let mut restored = text.to_owned();
    let mut last = restored.len();
    for edit in edge["inverse"].as_array().unwrap() {
        let start = edit["start_byte"].as_u64().unwrap() as usize;
        let after = edit["after"].as_str().unwrap();
        let end = start
            .checked_add(after.len())
            .expect("bounded inverse endpoint");
        assert!(end <= last);
        assert_eq!(restored.get(start..end), Some(after));
        restored.replace_range(start..end, edit["before"].as_str().unwrap());
        last = start;
    }
    assert_eq!(digest(&restored), edge["before_sha256"]);
    restored
}
fn legacy_checkpoint(source: &str, expected: &str, current: &str) -> String {
    let rows = integrated_records();
    let matching = rows
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["path"] == source)
        .collect::<Vec<_>>();
    assert_eq!(matching.len(), 1, "finite integrated source owner");
    let row = matching[0];
    assert_eq!(
        row["expected_sha256"], expected,
        "explicit original expected namespace"
    );
    let (acquired_physical, acquired_current) = final_ingress_checkpoint(source, expected, current);
    let current = acquired_current.as_str();
    let physical = acquired_physical.as_str();
    assert!(physical.len() <= 2 * 1024 * 1024);
    assert_eq!(
        physical.len() as u64,
        row["current_length"].as_u64().unwrap()
    );
    assert_eq!(
        digest(physical),
        row["current_sha256"],
        "whole actual integrated source"
    );
    let checkpoint = edge(row, physical, "current_to_e24");
    assert_eq!(checkpoint, row["e24"].as_str().unwrap());
    let legacy = edge(row, &checkpoint, "e24_to_legacy");
    assert_eq!(legacy, row["legacy"].as_str().unwrap());
    let old = records();
    let old = old
        .as_array()
        .unwrap()
        .iter()
        .find(|old| old["path"] == source)
        .unwrap();
    assert_eq!(digest(&legacy), old["after_sha256"]);
    assert_eq!(row["expected_sha256"], old["before_sha256"]);
    assert!(
        current == physical
            || current == checkpoint
            || current == legacy
            || current == old["before"].as_str().unwrap(),
        "foreign integrated source stage"
    );
    legacy
}
pub(super) fn acquire_expected(source: &str, expected: &str, current: &str) -> String {
    if !matches!(
        source,
        "src/cleanup_plan/replay.rs"
            | "src/source_reflows_merge.rs"
            | "src/source_reflows_merge/main67107_inputs.rs"
            | "tests/hir_module_boundaries.rs"
            | "tests/workspace/semantic_graph/source_inventory.rs"
            | "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs"
    ) {
        return current.to_owned();
    }
    let old = records();
    let matching = old
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["path"] == source)
        .collect::<Vec<_>>();
    assert!(matching.len() <= 1);
    let Some(row) = matching.first() else {
        return current.to_owned();
    };
    // A current/raw or older expected namespace is never normalized here.
    if row["before_sha256"] != expected {
        return current.to_owned();
    }
    let legacy = legacy_checkpoint(source, expected, current);
    restore(row, &legacy)
}
#[test]
fn finite_integrated_expected_namespace_authenticates_every_complete_stage() {
    let rows = integrated_records();
    let old = records();
    for row in rows.as_array().unwrap() {
        let source = row["path"].as_str().unwrap();
        let expected = row["expected_sha256"].as_str().unwrap();
        let physical = CURRENT.iter().find(|(path, _)| *path == source).unwrap().1;
        let original = old
            .as_array()
            .unwrap()
            .iter()
            .find(|old| old["path"] == source)
            .unwrap()["before"]
            .as_str()
            .unwrap();
        for stage in [
            physical,
            row["e24"].as_str().unwrap(),
            row["legacy"].as_str().unwrap(),
            original,
        ] {
            assert_eq!(acquire_expected(source, expected, stage), original);
        }
        assert_eq!(
            acquire_expected(source, &digest(physical), physical),
            physical
        );
        assert_eq!(
            acquire_expected(source, "older selected namespace", "older source stage"),
            "older source stage"
        );
        for bad in [
            String::new(),
            format!("{physical} "),
            format!("{physical}{physical}"),
            "foreign".to_owned(),
        ] {
            assert!(std::panic::catch_unwind(|| acquire_expected(source, expected, &bad)).is_err());
        }
    }
    assert_eq!(
        acquire_expected("foreign", "foreign", "unchanged"),
        "unchanged"
    );
}

// Separate FINAL -> sealed787 edge. Existing integrated/legacy packets are
// immutable; only an exact original expected namespace reaches this reader.
const FINAL_INGRESS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/evidence/ordinary-plan-final-ingress-20261017/current-inputs.json"
));
const FINAL_INGRESS_LENGTH: usize = 455009;
const FINAL_INGRESS_SHA256: &str =
    "3d61f09b3235194aba756af2ab9ec48ef7d24d77a5dcdba3190d104a306477fe";
const FINAL_INGRESS_CAP: usize = 2 * 1024 * 1024;
fn final_ingress_same_file(a: &std::fs::Metadata, b: &std::fs::Metadata) -> bool {
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
fn final_ingress_file(path: &std::path::Path, length: usize, expected: &str) -> String {
    use std::io::Read;
    assert!(length <= FINAL_INGRESS_CAP);
    let first = std::fs::symlink_metadata(path).unwrap();
    assert!(first.is_file() && !first.file_type().is_symlink());
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
        panic!("unsupported bounded physical source reader");
    }
    let mut file = options.open(path).unwrap();
    let opened = file.metadata().unwrap();
    assert!(final_ingress_same_file(&first, &opened));
    assert_eq!(opened.len(), u64::try_from(length).unwrap());
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length).unwrap();
    bytes.resize(length, 0);
    file.read_exact(&mut bytes).unwrap();
    assert_eq!(file.read(&mut [0u8; 1]).unwrap(), 0);
    let after = file.metadata().unwrap();
    let last = std::fs::symlink_metadata(path).unwrap();
    assert!(
        !last.file_type().is_symlink()
            && final_ingress_same_file(&opened, &after)
            && final_ingress_same_file(&after, &last)
    );
    let text = String::from_utf8(bytes).unwrap();
    assert_eq!(digest(&text), expected);
    text
}
// A separate newest->FINAL phase. The sealed FINAL, integrated and legacy
// records below remain unchanged and replay their original inverses.
fn latest_ingress_records() -> serde_json::Value {
    let data = include_str!(
        "../../docs/evidence/source-coverage-254-20261024/latest-ordinary-inputs.json"
    );
    assert!(data.len() <= FINAL_INGRESS_CAP);
    assert_eq!(data.len(), 499221);
    assert_eq!(
        digest(data),
        "575f816ae3634fcef8f5b2da7f1516a5e4d6a7723273871fe0722aec215861a8"
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        final_ingress_file(
            &root.join("docs/evidence/source-coverage-254-20261024/latest-ordinary-inputs.json"),
            data.len(),
            "575f816ae3634fcef8f5b2da7f1516a5e4d6a7723273871fe0722aec215861a8"
        ),
        data
    );
    let result: serde_json::Value = serde_json::from_str(data).unwrap();
    assert_eq!(
        result["schema"],
        "semaprax.ordinary-final-ingress-latest.v1"
    );
    assert_eq!(result["rows"].as_array().unwrap().len(), CURRENT.len());
    result
}

fn final_ingress_checkpoint(source: &str, expected: &str, current: &str) -> (String, String) {
    assert!(FINAL_INGRESS.len() <= FINAL_INGRESS_CAP);
    assert_eq!(FINAL_INGRESS.len(), FINAL_INGRESS_LENGTH);
    assert_eq!(digest(FINAL_INGRESS), FINAL_INGRESS_SHA256);
    let rows: serde_json::Value = serde_json::from_str(FINAL_INGRESS).unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), CURRENT.len());
    let frozen = integrated_records();
    let frozen = frozen.as_array().unwrap();
    let old = records();
    let old = old.as_array().unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut selected = None;
    let latest_packet = latest_ingress_records();
    let latest_rows = latest_packet["rows"].as_array().unwrap();
    for ((((row, frozen), old), (path, compiled)), newest) in rows
        .iter()
        .zip(frozen)
        .zip(old)
        .zip(CURRENT)
        .zip(latest_rows)
    {
        assert_eq!(row["path"], path);
        assert_eq!(frozen["path"], path);
        assert_eq!(old["path"], path);
        assert_eq!(row["expected_sha256"], old["before_sha256"]);
        assert_eq!(row["checkpoint_sha256"], frozen["current_sha256"]);
        assert_eq!(row["checkpoint_length"], frozen["current_length"]);
        let length = usize::try_from(row["current_length"].as_u64().unwrap()).unwrap();
        assert_eq!(newest["path"], path);
        assert_eq!(newest["previous_length"], row["current_length"]);
        assert_eq!(newest["previous_sha256"], row["current_sha256"]);
        let latest_length = usize::try_from(newest["current_length"].as_u64().unwrap()).unwrap();
        let latest_sha = newest["current_sha256"].as_str().unwrap();
        let latest = final_ingress_file(&root.join(path), latest_length, latest_sha);
        assert_eq!(
            latest, compiled,
            "actual complete compiled and physical final source"
        );
        let latest_archive = newest["current_archive"].as_str().unwrap();
        assert_eq!(latest_archive, format!("e/{latest_sha}"));
        assert_eq!(
            latest,
            final_ingress_file(&root.join(latest_archive), latest_length, latest_sha)
        );
        let mut physical = latest.clone();
        let mut last_latest = physical.len();
        let latest_inverse = newest["inverse"].as_array().unwrap();
        assert!(latest_inverse.len() <= 512);
        for edit in latest_inverse {
            let start = usize::try_from(edit["start_byte"].as_u64().unwrap()).unwrap();
            let after = edit["after"].as_str().unwrap();
            let end = start
                .checked_add(after.len())
                .expect("bounded newest inverse endpoint");
            assert!(end <= last_latest);
            assert_eq!(physical.get(start..end), Some(after));
            physical.replace_range(start..end, edit["before"].as_str().unwrap());
            last_latest = start;
        }
        assert_eq!(physical.len(), length);
        assert_eq!(digest(&physical), row["current_sha256"]);
        let retained = final_ingress_file(
            &root.join(row["current_archive"].as_str().unwrap()),
            length,
            row["current_sha256"].as_str().unwrap(),
        );
        assert_eq!(physical, retained);
        let checkpoint = final_ingress_file(
            &root.join(row["checkpoint_archive"].as_str().unwrap()),
            usize::try_from(row["checkpoint_length"].as_u64().unwrap()).unwrap(),
            row["checkpoint_sha256"].as_str().unwrap(),
        );
        let mut restored = physical.clone();
        let mut last = restored.len();
        let inverse = row["inverse"].as_array().unwrap();
        assert!(inverse.len() <= 512);
        for edit in inverse {
            let start = usize::try_from(edit["start_byte"].as_u64().unwrap()).unwrap();
            let after = edit["after"].as_str().unwrap();
            let end = start
                .checked_add(after.len())
                .expect("bounded final inverse endpoint");
            assert!(end <= last);
            assert_eq!(restored.get(start..end), Some(after));
            restored.replace_range(start..end, edit["before"].as_str().unwrap());
            last = start;
        }
        assert_eq!(restored, checkpoint, "complete final to frozen787 inverse");
        if path == source {
            assert_eq!(
                row["expected_sha256"], expected,
                "exact original c874 expected namespace"
            );
            assert!(
                current == latest
                    || current == physical
                    || current == checkpoint
                    || current == frozen["e24"].as_str().unwrap()
                    || current == frozen["legacy"].as_str().unwrap()
                    || current == old["before"].as_str().unwrap(),
                "foreign final ingress stage"
            );
            let supplied = if current == latest || current == physical {
                checkpoint.clone()
            } else {
                current.to_owned()
            };
            selected = Some((checkpoint, supplied));
        }
    }
    selected.expect("six literal final source owners only")
}
#[test]
fn final_ingress_six_physical_owners_preserve_frozen_stages_and_expected_dispatch() {
    let frozen = integrated_records();
    let old = records();
    for ((row, old), (source, physical)) in frozen
        .as_array()
        .unwrap()
        .iter()
        .zip(old.as_array().unwrap())
        .zip(CURRENT)
    {
        let expected = old["before_sha256"].as_str().unwrap();
        let (checkpoint, supplied) = final_ingress_checkpoint(source, expected, physical);
        assert_eq!(checkpoint, supplied);
        assert_eq!(digest(&checkpoint), row["current_sha256"]);
        for staged in [
            checkpoint.as_str(),
            row["e24"].as_str().unwrap(),
            row["legacy"].as_str().unwrap(),
            old["before"].as_str().unwrap(),
        ] {
            let (again, supplied) = final_ingress_checkpoint(source, expected, staged);
            assert_eq!(again, checkpoint);
            assert_eq!(supplied, staged);
            assert_eq!(
                acquire_expected(source, expected, staged),
                old["before"].as_str().unwrap()
            );
        }
        assert_eq!(
            acquire_expected(source, &digest(physical), physical),
            physical
        );
        assert_eq!(
            acquire_expected(source, "older expected namespace", "older stage"),
            "older stage"
        );
        for bad in [String::new(), format!("{physical} "), "foreign".to_owned()] {
            assert!(
                std::panic::catch_unwind(|| final_ingress_checkpoint(source, expected, &bad))
                    .is_err()
            );
        }
    }
}

// Exact root-only custody entry. It shares the complete six-owner FINAL
// authenticator; the caller supplies actual compiled root and flat old owner.
pub(super) fn original_root_checkpoint(current: &str, compiled_before: &str) -> String {
    assert_eq!(CURRENT[0].0, "src/cleanup_plan/replay.rs");
    assert_eq!(
        current, CURRENT[0].1,
        "custody entry requires actual compiled root"
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let actual_before = final_ingress_file(
        &root.join("src/cleanup_plan/replay/ordinary_plan_history/before.rs"),
        27915,
        "9e07f08634f9a79e9d38955da089790acf951521362e46ba23b073e67e9ba3a5",
    );
    assert_eq!(
        actual_before, compiled_before,
        "complete current compiled flat predecessor"
    );
    let legacy = records();
    assert_eq!(legacy[0]["path"], "src/cleanup_plan/replay.rs");
    let (checkpoint, _) = final_ingress_checkpoint(
        "src/cleanup_plan/replay.rs",
        legacy[0]["before_sha256"].as_str().unwrap(),
        current,
    );
    checkpoint
}
