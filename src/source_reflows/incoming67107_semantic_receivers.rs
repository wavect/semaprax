//! Exact custody for later receiver changes, separate from CI reformatting.
//! These snapshots are used only by historical source assertions. Live source
//! inventories continue to include the current modules and their new children.
use super::*;

const PROOFS: &str = include_str!("../../docs/evidence/main67107-production-composition-20261017/receiver-main12.json");
const LIVE_AUDITS: &str =
    include_str!("../../docs/evidence/live-source-audit-custody-v1/inverse.json");
static PROOF_RECORDS: OnceLock<Vec<serde_json::Value>> = OnceLock::new();

// A later test receiver has its own semantic inverse before the unchanged
// formatting registry. Keep all original twelve plus two custody proofs intact.
fn followups() -> &'static [serde_json::Value] {
    static RECORDS: OnceLock<Vec<serde_json::Value>> = OnceLock::new();
    RECORDS.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../docs/evidence/source-reader-followups-20261016/allocation.json"
        ))
        .unwrap()
    })
}

// Physical setup successors have their own complete-input custody before the
// frozen formatting input. They do not edit or replace any formatting repair,
// original twelve-plus-two receiver proof, or allocation followup.
fn ordinary_setup() -> &'static [serde_json::Value] {
    static RECORDS: OnceLock<Vec<serde_json::Value>> = OnceLock::new();
    RECORDS.get_or_init(|| {
        serde_json::from_str(
            &std::fs::read_to_string(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("docs/evidence/ordinary-replay-setup-v1/format-inputs.json"),
            )
            .unwrap(),
        )
        .unwrap()
    })
}

// Other promoted helper inputs also retain their complete historical view.
// Read evidence at runtime so these archived modules are not embedded in each
// compiler test binary.
fn ordinary_helpers() -> &'static [serde_json::Value] {
    static RECORDS: OnceLock<Vec<serde_json::Value>> = OnceLock::new();
    RECORDS.get_or_init(|| {
        serde_json::from_str(
            &std::fs::read_to_string(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("docs/evidence/ordinary-replay-setup-v1/helper-inputs.json"),
            )
            .unwrap(),
        )
        .unwrap()
    })
}

fn proofs() -> &'static [serde_json::Value] {
    PROOF_RECORDS.get_or_init(|| {
        let mut original: Vec<serde_json::Value> = serde_json::from_str(PROOFS).unwrap();
        let additional: Vec<serde_json::Value> = serde_json::from_str(LIVE_AUDITS).unwrap();
        assert_eq!(original.len(), 12);
        assert_eq!(additional.len(), 2);
        original.extend(additional);
        original
    })
}

fn digest(source: &str) -> String {
    format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(source.as_bytes()))
    )
}

pub(super) fn invert_ordinary(proof: &serde_json::Value, current: &str) -> String {
    let path = proof["path"].as_str().unwrap();
    assert_eq!(
        digest(current),
        proof["after_sha256"],
        "receiver input drifted: {path}"
    );
    let mut restored = current.to_owned();
    for edit in proof["inverse"].as_array().unwrap() {
        let after = edit["after"].as_str().unwrap();
        assert_eq!(
            restored.matches(after).count(),
            1,
            "nonunique receiver inverse: {path}"
        );
        restored = restored.replacen(after, edit["before"].as_str().unwrap(), 1);
    }
    assert_eq!(
        digest(&restored),
        proof["before_sha256"],
        "receiver predecessor drifted: {path}"
    );
    restored
}

fn receiver_input(path: &str, current: &str) -> String {
    let domain = match path {
        "src/cleanup_plan/replay/selected_entry/tests/session_errors/source.rs" => "liveSessionMAIN",
        "src/hir/nominal_payload/legacy/source_tests.rs" => "liveNominalMAIN",
        _ => return current.to_owned(),
    };
    crate::source_reflows_merge::main67107_inputs::project(domain, current)
}
fn invert(proof: &serde_json::Value, current: &str) -> String {
    let view = receiver_input(proof["path"].as_str().unwrap(), current);
    invert_ordinary(proof, &view)
}

pub(super) fn predecessor(path: &str, current: &str) -> Option<String> {
    if let Some(proof) = ordinary_setup()
        .iter()
        .chain(ordinary_helpers())
        .find(|proof| proof["path"] == path)
    {
        return Some(invert(proof, current));
    }
    if let Some(proof) = followups().iter().find(|proof| proof["path"] == path) {
        return Some(invert(proof, current));
    }
    let proof = proofs().iter().find(|proof| proof["path"] == path)?;
    Some(invert(proof, current))
}

pub(super) fn contains(path: &str) -> bool {
    proofs()
        .iter()
        .chain(followups())
        .chain(ordinary_setup())
        .chain(ordinary_helpers())
        .any(|proof| proof["path"] == path)
}

#[test]
fn ordinary_helper_views_authenticate_every_complete_input() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(ordinary_helpers().len(), 20);
    let mut paths = std::collections::BTreeSet::new();
    for proof in ordinary_helpers() {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        assert!(!proofs()
            .iter()
            .chain(followups())
            .chain(ordinary_setup())
            .any(|old| old["path"] == path));
        let current = std::fs::read_to_string(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            std::fs::read_to_string(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(contains(path));
        assert_eq!(predecessor(path, &current), Some(before));
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
        let mut forged = proof.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| invert(&forged, &current)).is_err());
    }
}

#[test]
fn ordinary_setup_complete_sources_and_original_legacy_constructor_remain_exact() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let evidence = root.join("docs/evidence/ordinary-replay-setup-v1");
    let rows: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(evidence.join("sources.json")).unwrap())
            .unwrap();
    assert_eq!(rows.len(), 34);
    let mut paths = std::collections::BTreeSet::new();
    for row in &rows {
        let path = row["path"].as_str().unwrap();
        assert!(paths.insert(path));
        let current = std::fs::read_to_string(root.join(path)).unwrap();
        let before = invert(row, &current);
        assert_eq!(
            before,
            std::fs::read_to_string(evidence.join(row["archive"].as_str().unwrap())).unwrap()
        );
        assert!(std::panic::catch_unwind(|| invert(row, &(current.clone() + "\n"))).is_err());
        if path == "src/cleanup_plan/replay/skeleton_work.rs" {
            let start = before.find("        Ok(Self {\n").unwrap();
            let end = before[start..]
                .find("\n    pub(super) fn checked_with_index(")
                .unwrap()
                + start;
            let legacy = &before[start..end];
            assert_eq!(current.matches(legacy).count(), 1);
        }
    }
}

#[test]
fn ordinary_setup_bridges_preserve_every_complete_original_format_input() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(ordinary_setup().len(), 6);
    let mut paths = std::collections::BTreeSet::new();
    for proof in ordinary_setup() {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        assert!(!proofs()
            .iter()
            .chain(followups())
            .any(|old| old["path"] == path));
        let original_format = records()
            .iter()
            .find(|record| record["path"] == path)
            .unwrap();
        assert_eq!(proof["before_sha256"], original_format["after_sha256"]);
        let current = std::fs::read_to_string(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            std::fs::read_to_string(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert_eq!(
            digest(&restore(original_format, &before)),
            original_format["before_sha256"]
        );
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
    }
}

#[test]
fn every_receiver_inverse_preserves_the_entire_reflow_input_and_archive() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut paths = std::collections::BTreeSet::new();
    assert_eq!(proofs().len(), 14);
    for proof in proofs() {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        let repair = records().iter().find(|repair| repair["path"] == path);
        if let Some(repair) = repair {
            assert_eq!(proof["before_sha256"], repair["after_sha256"]);
        }
        let current = std::fs::read_to_string(root.join(path)).unwrap();
        let restored = invert(proof, &current);
        let source_audits: Vec<serde_json::Value> = serde_json::from_str(LIVE_AUDITS).unwrap();
        let archive_root = if source_audits.iter().any(|audit| audit["path"] == path) {
            "docs/evidence/live-source-audit-custody-v1"
        } else {
            "docs/evidence/receiver-source-custody-v1"
        };
        let archive = root
            .join(archive_root)
            .join(proof["archive"].as_str().unwrap());
        assert_eq!(restored, std::fs::read_to_string(archive).unwrap());
        let current = receiver_input(path, &current);
        assert_ne!(current, restored);
        // Execute the unchanged formatting inverse and its whole-file digest.
        if let Some(repair) = repair {
            assert_ne!(restore(repair, &restored), restored);
        }
    }
    assert!(predecessor("src/source_reflows.rs", "unregistered source").is_none());
}

#[test]
fn receiver_mutation_and_forged_predecessor_fail_closed() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for proof in proofs() {
        let current = std::fs::read_to_string(root.join(proof["path"].as_str().unwrap())).unwrap();
        let changed = format!("{current} ");
        assert!(std::panic::catch_unwind(|| invert(proof, &changed)).is_err());
        let mut forged = proof.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| invert(&forged, &current)).is_err());
    }
}

#[test]
fn allocation_followup_preserves_the_complete_original_format_input() {
    assert_eq!(followups().len(), 1);
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for proof in followups() {
        let path = proof["path"].as_str().unwrap();
        assert!(!proofs().iter().any(|original| original["path"] == path));
        let current = std::fs::read_to_string(root.join(path)).unwrap();
        let restored = invert(proof, &current);
        let archive = root
            .join("docs/evidence/source-reader-followups-20261016")
            .join(proof["archive"].as_str().unwrap());
        assert_eq!(restored, std::fs::read_to_string(archive).unwrap());
        let format = records()
            .iter()
            .find(|record| record["path"] == path)
            .unwrap();
        assert_eq!(proof["before_sha256"], format["after_sha256"]);
        assert_ne!(restore(format, &restored), restored);
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
        let mut forged = proof.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| invert(&forged, &current)).is_err());
    }
}

fn followup_input(path: &str, current: &str) -> String {
    let domain = match path {
        "src/source_reflows/semantic_receivers.rs" => "followupReceiverMAIN",
        "src/workspace_graph/identity_resolution.rs" => "followupIdentityMAIN",
        "src/workspace_graph/expected_projection/import_stub/core_proofs/tests.rs" => "followupCoreMAIN",
        _ => panic!("foreign MAIN followup source"),
    };
    crate::source_reflows_merge::main67107_inputs::project(domain, current)
}

#[test]
fn complete_followup_receivers_keep_all_original_source_bytes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let evidence = root.join("docs/evidence/source-reader-followups-20261016");
    let records: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(evidence.join("sources.json")).unwrap())
            .unwrap();
    for record in records.as_array().unwrap() {
        let original =
            std::fs::read_to_string(evidence.join(record["archive"].as_str().unwrap())).unwrap();
        assert_eq!(digest(&original), record["sha256"]);
        let current = std::fs::read_to_string(root.join(record["path"].as_str().unwrap())).unwrap();
        let current = followup_input(record["path"].as_str().unwrap(), &current);
        let restore = |source: &str| -> Option<String> {
            let mut restored = source.to_owned();
            for edit in record["edits"].as_array().unwrap().iter().rev() {
                let new = edit["new"].as_str().unwrap();
                if restored.matches(new).count() != 1 {
                    return None;
                }
                restored = restored.replacen(new, edit["old"].as_str().unwrap(), 1);
            }
            Some(restored)
        };
        assert_eq!(restore(&current).as_deref(), Some(original.as_str()));
        assert_ne!(
            restore(&(current.clone() + "\n")).as_deref(),
            Some(original.as_str())
        );
        assert_ne!(
            restore(&current.replacen(&current[..1], "!", 1)).as_deref(),
            Some(original.as_str())
        );
    }
}
