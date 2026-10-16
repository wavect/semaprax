//! Exact custody for later receiver changes, separate from CI reformatting.
//! These snapshots are used only by historical source assertions. Live source
//! inventories continue to include the current modules and their new children.
use super::*;

const PROOFS: &str = include_str!(
    "../../docs/evidence/main-f1-prefix-lexical-subsumption-20261016/receiver-inverse12.json"
);
static PROOF_RECORDS: OnceLock<serde_json::Value> = OnceLock::new();

fn proofs() -> &'static [serde_json::Value] {
    PROOF_RECORDS
        .get_or_init(|| serde_json::from_str(PROOFS).unwrap())
        .as_array()
        .unwrap()
}

fn digest(source: &str) -> String {
    format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(source.as_bytes()))
    )
}

fn invert(proof: &serde_json::Value, current: &str) -> String {
    let path = proof["path"].as_str().unwrap();
    let main = crate::source_reflows_merge::main_f1_input(path, current);
    let current = main.as_str();
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
    assert_eq!(proofs().len(), 12);
    for proof in proofs() {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        let repair = records().iter().find(|repair| repair["path"] == path);
        if let Some(repair) = repair {
            assert_eq!(proof["before_sha256"], repair["after_sha256"]);
        }
        let current = std::fs::read_to_string(root.join(path)).unwrap();
        let restored = invert(proof, &current);
        let archive = root
            .join("docs/evidence/receiver-source-custody-v1")
            .join(proof["archive"].as_str().unwrap());
        assert_eq!(restored, std::fs::read_to_string(archive).unwrap());
        if path == "src/hir/expr_nodes.rs" {
            assert_eq!(restored, include_str!("../../docs/evidence/receiver-source-custody-v1/src__hir__expr_nodes.rs.before.txt"), "complete compiled previous expression model");
        }
        assert_ne!(current, restored);
        // Execute the unchanged formatting inverse and its whole-file digest.
        if let Some(repair) = repair {
            assert_ne!(restore_formatting(repair, &restored), restored);
        }
    }
    assert!(predecessor("src/source_reflows.rs", "unregistered source").is_none());
    verify_main24(false);
}

#[test]
fn receiver_mutation_and_forged_predecessor_fail_closed() {
    verify_main24(true);
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

// Independent complete Main24 tables; neither predecessor is fed to another raw front.
fn invert_main24(proof: &serde_json::Value, current: &str) -> String {
    let main = crate::source_reflows_merge::main24_input(proof["path"].as_str().unwrap(), current);
    assert_eq!(
        digest(&main),
        proof["after_sha256"],
        "complete Main24 current input"
    );
    let mut restored = main;
    for edit in proof["inverse"].as_array().unwrap() {
        let after = edit["after"].as_str().unwrap();
        assert_eq!(restored.matches(after).count(), 1, "unique Main24 inverse");
        restored = restored.replacen(after, edit["before"].as_str().unwrap(), 1);
    }
    assert_eq!(
        digest(&restored),
        proof["before_sha256"],
        "complete Main24 predecessor"
    );
    restored
}
fn verify_main24(hostile: bool) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for compact in [false, true] {
        let raw = crate::source_reflows_merge::main24_proofs(compact);
        let records: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let records = records.as_array().unwrap();
        assert_eq!(records.len(), if compact { 7 } else { 12 });
        let mut paths = std::collections::BTreeSet::new();
        for proof in records {
            let path = proof["path"].as_str().unwrap();
            assert!(paths.insert(path));
            let current = std::fs::read_to_string(root.join(path)).unwrap();
            if hostile {
                assert!(
                    std::panic::catch_unwind(|| invert_main24(proof, &format!("{current} ")))
                        .is_err()
                );
                let mut forged = proof.clone();
                forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
                assert!(std::panic::catch_unwind(|| invert_main24(&forged, &current)).is_err());
            } else {
                let restored = invert_main24(proof, &current);
                let packet = if compact {
                    "compact-staged-observations-v1"
                } else {
                    "receiver-source-custody-v1"
                };
                let archive = root
                    .join("docs/evidence")
                    .join(packet)
                    .join(proof["archive"].as_str().unwrap());
                assert_eq!(restored, std::fs::read_to_string(archive).unwrap());
                if path == "src/hir/expr_nodes.rs" {
                    assert_eq!(restored, include_str!("../../docs/evidence/receiver-source-custody-v1/src__hir__expr_nodes.rs.before.txt"), "complete compiled prior expression model");
                }
            }
        }
    }
}
