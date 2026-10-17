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
