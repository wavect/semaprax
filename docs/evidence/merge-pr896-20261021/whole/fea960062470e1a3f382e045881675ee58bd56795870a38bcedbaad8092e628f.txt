//! Exact custody for later receiver changes, separate from CI reformatting.
//! These snapshots are used only by historical source assertions. Live source
//! inventories continue to include the current modules and their new children.
use super::*;

static PROOF_RECORDS: OnceLock<Vec<serde_json::Value>> = OnceLock::new();

fn proofs() -> &'static [serde_json::Value] {
    PROOF_RECORDS.get_or_init(|| {
        let mut original: Vec<serde_json::Value> =
            serde_json::from_str(metadata(Fixture::Receivers)).unwrap();
        let additional: Vec<serde_json::Value> =
            serde_json::from_str(metadata(Fixture::LiveAudits)).unwrap();
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

fn invert(proof: &serde_json::Value, current: &str) -> String {
    let path = proof["path"].as_str().unwrap();
    let joined = crate::source_reflows_merge::main043_input(path, current);
    let current = joined.as_str();
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

pub(super) fn predecessor(path: &str, current: &str) -> Option<String> {
    let proof = proofs().iter().find(|proof| proof["path"] == path)?;
    Some(invert(proof, current))
}

pub(super) fn contains(path: &str) -> bool {
    proofs().iter().any(|proof| proof["path"] == path)
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
        let source_audits: Vec<serde_json::Value> =
            serde_json::from_str(metadata(Fixture::LiveAudits)).unwrap();
        let archive_root = if source_audits.iter().any(|audit| audit["path"] == path) {
            "docs/evidence/live-source-audit-custody-v1"
        } else {
            "docs/evidence/receiver-source-custody-v1"
        };
        let archive = root
            .join(archive_root)
            .join(proof["archive"].as_str().unwrap());
        assert_eq!(restored, std::fs::read_to_string(archive).unwrap());
        // This named comparison uses the authenticated Main043 afterimage.
        // A later raw endpoint may itself equal that proof's predecessor.
        let current = receiver_comparison_input(proof, &current);
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
fn complete_registered_helper_membership_is_preserved() {
    for proof in proofs() {
        assert!(contains(proof["path"].as_str().unwrap()));
    }
}

// These two historical audits have raw endpoints equal to their predecessors.
// The existing sealed route authenticates the complete reviewed afterimage;
// the unchanged nonidentity assertion compares that afterimage with its archive.
fn receiver_comparison_input(proof: &serde_json::Value, current: &str) -> String {
    let path = proof["path"].as_str().unwrap();
    if !matches!(
        path,
        "src/cleanup_plan/replay/selected_entry/tests/session_errors/source.rs"
            | "src/hir/nominal_payload/legacy/source_tests.rs"
    ) {
        return current.to_owned();
    }
    let afterimage = crate::source_reflows_merge::main043_input(path, current);
    assert_eq!(
        digest(&afterimage),
        proof["after_sha256"],
        "whole Main043 afterimage"
    );
    afterimage
}

#[test]
fn selected_audit_comparison_uses_the_authenticated_historical_domain() {
    let path = "src/cleanup_plan/replay/selected_entry/tests/session_errors/source.rs";
    let proof = proofs().iter().find(|proof| proof["path"] == path).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let current = std::fs::read_to_string(root.join(path)).unwrap();
    let restored = invert(proof, &current);
    let afterimage = receiver_comparison_input(proof, &current);
    assert_ne!(afterimage, restored);
    assert_eq!(digest(&afterimage), proof["after_sha256"]);
    assert_eq!(digest(&restored), proof["before_sha256"]);
    assert!(
        std::panic::catch_unwind(|| receiver_comparison_input(proof, &format!("{current} ")))
            .is_err()
    );
    let mut forged = proof.clone();
    forged["after_sha256"] = serde_json::Value::String("0".repeat(64));
    assert!(std::panic::catch_unwind(|| receiver_comparison_input(&forged, &current)).is_err());
}

#[test]
fn nominal_audit_comparison_retains_complete_reviewed_afterimage_and_archive() {
    let path = "src/hir/nominal_payload/legacy/source_tests.rs";
    let proof = proofs().iter().find(|proof| proof["path"] == path).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let current = std::fs::read_to_string(root.join(path)).unwrap();
    let archive = std::fs::read_to_string(
        root.join("docs/evidence/live-source-audit-custody-v1")
            .join(proof["archive"].as_str().unwrap()),
    )
    .unwrap();
    assert_eq!(current, archive, "raw current is this exact predecessor");
    let afterimage = receiver_comparison_input(proof, &current);
    assert_eq!(digest(&afterimage), proof["after_sha256"]);
    assert_ne!(afterimage, archive);
    assert_eq!(invert(proof, &current), archive);
    assert_eq!(digest(&archive), proof["before_sha256"]);
    for bad in [
        String::new(),
        format!("{current} "),
        current[..current.len() - 1].to_owned(),
        afterimage.clone(),
    ] {
        assert!(std::panic::catch_unwind(|| receiver_comparison_input(proof, &bad)).is_err());
    }
    let mut forged = proof.clone();
    forged["after_sha256"] = serde_json::Value::String("0".repeat(64));
    assert!(std::panic::catch_unwind(|| receiver_comparison_input(&forged, &current)).is_err());
}
