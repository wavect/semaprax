use super::*;

#[test]
fn inline_status_receipts_keep_all_complete_inputs_and_original_validation_loops() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let proofs: Vec<serde_json::Value> = serde_json::from_str(
        &std::fs::read_to_string(root.join("docs/evidence/status-inline-receipts-v1/sources.json"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(proofs.len(), 7);
    let mut paths = std::collections::BTreeSet::new();
    for proof in &proofs {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        let current = std::fs::read_to_string(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            std::fs::read_to_string(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
        let mut forged = proof.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| invert(&forged, &current)).is_err());
    }
}
