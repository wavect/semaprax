//! Exact source-only custody domains for the composed MAIN receiver.
use sha2::{Digest, Sha256};
fn digest(text: &str) -> String { format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))) }
fn records() -> serde_json::Value {
    let data = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main67107-fronts-6e45-20261017/stages.json"));
    assert!(data.len() <= 1_048_576);
    assert_eq!(digest(data), "bfbacf2777c310ee34ad6acd34352a9c58777ec11d39f2649be584711d72f120");
    let rows: serde_json::Value = serde_json::from_str(data).unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 9);
    rows
}
fn restore(rows: &serde_json::Value, domain: &str, current: &str) -> String {
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.iter().filter(|r| r["domain"] == domain).count(), 1);
    let row = rows.iter().find(|r| r["domain"] == domain).unwrap();
    let source = match domain {
        "canonical870" | "canonicalMAIN" => "src/cleanup_plan/replay/factored/canonical_shared.rs",
        "testsMAIN" => "src/cleanup_plan/replay/factored/canonical_shared/tests.rs",
        "ownedGrowth246fd230" => "src/workspace_graph/expected_projection/borrowed_imports/typed/alias/owned_growth_tests.rs",
        "followupReceiverMAIN" => "src/source_reflows/semantic_receivers.rs",
        "followupIdentityMAIN" => "src/workspace_graph/identity_resolution.rs",
        "followupCoreMAIN" => "src/workspace_graph/expected_projection/import_stub/core_proofs/tests.rs",
        "liveSessionMAIN" => "src/cleanup_plan/replay/selected_entry/tests/session_errors/source.rs",
        "liveNominalMAIN" => "src/hir/nominal_payload/legacy/source_tests.rs",
        _ => panic!("foreign MAIN source domain"),
    };
    assert_eq!(row["source"], source);
    assert_eq!(digest(current), row["current_sha256"]);
    assert_eq!(current, row["current"].as_str().unwrap());
    let edits = row["inverse"].as_array().unwrap();
    assert!(!edits.is_empty() && edits.len() <= 512);
    let mut lines: Vec<&str> = current.split_inclusive('\n').collect();
    let mut last = lines.len();
    for edit in edits.iter().rev() {
        let a = edit["start"].as_u64().unwrap() as usize;
        let z = edit["end"].as_u64().unwrap() as usize;
        assert!(a <= z && z <= last && z <= lines.len());
        assert_eq!(lines[a..z].concat(), edit["current"].as_str().unwrap());
        lines.splice(a..z, edit["previous"].as_str().unwrap().split_inclusive('\n'));
        last = a;
    }
    let before = lines.concat();
    assert_eq!(digest(&before), row["previous_sha256"]);
    assert_eq!(before.len() as u64, row["previous_length"].as_u64().unwrap());
    before
}
pub(crate) fn project(domain: &str, current: &str) -> String { restore(&records(), domain, current) }
#[test]
fn complete_main_domains_and_both_unit_namespaces_are_preserved() {
    let rows = records();
    for row in rows.as_array().unwrap() {
        let domain = row["domain"].as_str().unwrap();
        let raw = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(row["source"].as_str().unwrap())).unwrap();
        let old = restore(&rows, domain, &raw);
        let archived = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/evidence/main67107-fronts-6e45-20261017").join(row["archive"].as_str().unwrap())).unwrap();
        assert_eq!(old, archived);
    }
    let root = include_str!("../cleanup_plan/replay/factored/canonical_shared.rs");
    let mut original = project("canonicalMAIN", root);
    let inverse: serde_json::Value = serde_json::from_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main67107-production-composition-20261017/unit-main10.json"))).unwrap();
    assert_eq!(inverse.as_array().unwrap().len(), 10);
    for change in inverse.as_array().unwrap().iter().rev() {
        let a = change[0].as_u64().unwrap() as usize; let now = change[1].as_str().unwrap();
        assert_eq!(original.get(a..a + now.len()), Some(now));
        original.replace_range(a..a + now.len(), change[2].as_str().unwrap());
    }
    assert_eq!(original, include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/unit-weight-closure-v1/canonical_shared.before.rs.txt")));
    let old: serde_json::Value = serde_json::from_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main67107-production-composition-20261017/unit-ours8.json"))).unwrap();
    assert_eq!(old.as_array().unwrap().len(), 8);
    assert_eq!(&inverse.as_array().unwrap()[..8], old.as_array().unwrap().as_slice());
}
#[test]
fn foreign_truncated_duplicate_reordered_and_forged_main_domains_fail_closed() {
    let rows = records();
    for row in rows.as_array().unwrap() {
        let domain = row["domain"].as_str().unwrap(); let raw = row["current"].as_str().unwrap();
        for bad in [String::new(), format!("{raw} "), format!("{raw}{raw}"), raw[..raw.len()-1].to_owned(), "foreign".to_owned()] {
            assert!(std::panic::catch_unwind(|| restore(&rows, domain, &bad)).is_err());
        }
        let mut forged = rows.clone();
        let record = forged.as_array_mut().unwrap().iter_mut().find(|r| r["domain"] == domain).unwrap();
        record["previous_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| restore(&forged, domain, raw)).is_err());
        let mut duplicate = rows.clone(); duplicate.as_array_mut().unwrap().push(row.clone());
        assert!(std::panic::catch_unwind(|| restore(&duplicate, domain, raw)).is_err());
        let mut reordered = rows.clone();
        let record = reordered.as_array_mut().unwrap().iter_mut().find(|r| r["domain"] == domain).unwrap();
        if record["inverse"].as_array().unwrap().len() > 1 {
            record["inverse"].as_array_mut().unwrap().reverse();
            assert!(std::panic::catch_unwind(|| restore(&reordered, domain, raw)).is_err());
        }
    }
    assert!(std::panic::catch_unwind(|| project("foreign", "foreign")).is_err());
}


// Distinct from followupReceiverMAIN: the old043 self-source proof expects
// stage457d, not frozen MAIN68c9. Only historical source equality uses this.
fn main043_receiver_restore(row: &serde_json::Value, source: &str, current: &str) -> String {
    assert_eq!(row.as_object().unwrap().len(), 7);
    assert_eq!(source, "src/source_reflows/semantic_receivers.rs");
    assert_eq!(row["source"], source);
    assert!(current.len() <= 65_536);
    assert_eq!(digest(current), row["current_sha256"]);
    assert_eq!(current, row["current"].as_str().unwrap());
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/evidence/main043-semantic-acquisition-20261017");
    assert_eq!(current, std::fs::read_to_string(root.join("current.rs.txt")).unwrap());
    let previous = std::fs::read_to_string(root.join("prior-stage.rs.txt")).unwrap();
    assert_eq!(digest(&previous), row["previous_sha256"]);
    let edits = row["inverse"].as_array().unwrap();
    assert!(!edits.is_empty() && edits.len() <= 32);
    let mut lines: Vec<&str> = current.split_inclusive('\n').collect();
    let mut last = lines.len();
    for edit in edits.iter().rev() {
        assert_eq!(edit.as_object().unwrap().len(), 4);
        let a = usize::try_from(edit["start"].as_u64().unwrap()).unwrap();
        let z = usize::try_from(edit["end"].as_u64().unwrap()).unwrap();
        assert!(a <= z && z <= last && z <= lines.len());
        assert_eq!(lines[a..z].concat(), edit["current"].as_str().unwrap());
        lines.splice(a..z, edit["previous"].as_str().unwrap().split_inclusive('\n'));
        last = a;
    }
    let restored = lines.concat();
    assert_eq!(digest(&restored), row["previous_sha256"]);
    assert_eq!(restored, row["previous"].as_str().unwrap());
    assert_eq!(restored, previous);
    restored
}
fn main043_receiver_record() -> serde_json::Value {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let raw = std::fs::read_to_string(root.join("docs/evidence/main043-semantic-acquisition-20261017/current-to-043-stage.json")).unwrap();
    assert!(raw.len() <= 131_072);
    assert_eq!(digest(&raw), "63bd9f8a4f8d19369a78798a41278e5b96ab0732361598656884efd6af159ff9");
    serde_json::from_str(&raw).unwrap()
}
#[test]
fn main043_receiver_complete_stage_and_original_historical_inverse_are_retained() {
    let source = "src/source_reflows/semantic_receivers.rs";
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let raw = std::fs::read_to_string(root.join(source)).unwrap();
    let row = main043_receiver_record();
    let current = main043_receiver_restore(&row, source, &raw);
    let packet = root.join("docs/evidence/main043-independent-custody-20261016/semantic-receiver-historical-view");
    let old = std::fs::read_to_string(packet.join("current-to-main043.json")).unwrap();
    assert_eq!(digest(&old), "1aa0092ae0b2503a0fc398efe2dd3454f9940d052f6c3e9cf5861c57e9d05b5d");
    let old: serde_json::Value = serde_json::from_str(&old).unwrap();
    // Original whole-stage assertions and line inverse, independently checked.
    let historical = |old: &serde_json::Value| {
    assert_eq!(old["source"], source);
    assert_eq!(digest(&current), old["current_sha256"]);
    assert_eq!(current, old["current"].as_str().unwrap());
    assert!(current.len() <= 65_536);
    let edits = old["inverse"].as_array().unwrap();
    assert!(edits.len() <= 32);
    let mut lines: Vec<&str> = current.split_inclusive('\n').collect();
    let mut last = lines.len();
    for edit in edits.iter().rev() {
        let a = usize::try_from(edit["start"].as_u64().unwrap()).unwrap();
        let z = usize::try_from(edit["end"].as_u64().unwrap()).unwrap();
        assert!(a <= z && z <= last && z <= lines.len());
        assert_eq!(lines[a..z].concat(), edit["current"].as_str().unwrap());
        lines.splice(a..z, edit["previous"].as_str().unwrap().split_inclusive('\n'));
        last = a;
    }
    let restored = lines.concat();
    assert_eq!(digest(&restored), old["historical_sha256"]);
    restored
    };
    let restored = historical(&old);
    let mut forged_old = old.clone(); forged_old["historical_sha256"] = serde_json::Value::String("0".repeat(64));
    assert!(std::panic::catch_unwind(|| historical(&forged_old)).is_err());
    assert_eq!(digest(&restored), "651d174d04fad97da5c08ad215a869fd395a7d691d4a78dc901a99e814fc8252");
    assert_eq!(restored, std::fs::read_to_string(packet.join("main043-semantic_receivers.rs.txt")).unwrap());
    for bad in [String::new(), format!("{raw} "), format!("{raw}{raw}"), current, "foreign".to_owned()] {
        assert!(std::panic::catch_unwind(|| main043_receiver_restore(&row, source, &bad)).is_err());
    }
    assert!(std::panic::catch_unwind(|| main043_receiver_restore(&row, "foreign", &raw)).is_err());
    let mut forged = row.clone(); forged["previous_sha256"] = serde_json::Value::String("0".repeat(64));
    assert!(std::panic::catch_unwind(|| main043_receiver_restore(&forged, source, &raw)).is_err());
    let mut altered = row.clone(); altered["inverse"][0]["previous"] = serde_json::Value::String("foreign".to_owned());
    assert!(std::panic::catch_unwind(|| main043_receiver_restore(&altered, source, &raw)).is_err());
}
