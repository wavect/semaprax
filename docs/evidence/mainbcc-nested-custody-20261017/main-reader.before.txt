//! File custody lives in the test source-reader layer, outside core HIR.
mod ci_scope {
    pub(crate) fn nested_scope_correction_retains_complete_original_hostile_and_loan_controls() {
        use sha2::{Digest, Sha256};
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let evidence = root.join("docs/evidence/nested-byte-traversal-780");
        let record: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(evidence.join("ci-scope-original.json")).unwrap(),
        )
        .unwrap();
        let original =
            std::fs::read_to_string(evidence.join(record["archive"].as_str().unwrap())).unwrap();
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
            ),
            record["sha256"].as_str().unwrap()
        );
        let current = std::fs::read_to_string(root.join(record["path"].as_str().unwrap())).unwrap();
        let restore = |source: &str| -> Option<String> {
            let mut result = source.to_owned();
            for edit in record["edits"].as_array().unwrap().iter().rev() {
                let new = edit["new"].as_str().unwrap();
                if result.matches(new).count() != 1 {
                    return None;
                }
                result = result.replacen(new, edit["old"].as_str().unwrap(), 1);
            }
            Some(result)
        };
        assert_eq!(restore(&current).as_deref(), Some(original.as_str()));
        assert_ne!(
            restore(&(current.clone() + "\n")).as_deref(),
            Some(original.as_str())
        );
        let start = original.find("    let freezes = sum\n").unwrap();
        let end = start
            + original[start..]
                .find("    for id in outer.shape.frozen {")
                .unwrap();
        assert_eq!(current.matches(&original[start..end]).count(), 1);
        let tail_end = end + original[end..].find("\n#[test]").unwrap();
        assert_eq!(current.matches(&original[end..tail_end]).count(), 1);
        let tampered = current.replacen(
            "after_inner.live_after.contains",
            "after_inner.live_before.contains",
            1,
        );
        assert_ne!(restore(&tampered).as_deref(), Some(original.as_str()));
        assert!(current.contains("let scope_metrics = ci_scope::empty_nested_budget_metrics();"));
        assert!(current.contains("assert_eq!(metrics, scope_metrics);"));
        assert!(current.contains("assert_eq!(metrics.allocated - scope_metrics.allocated, 0);"));
    }
}
pub(crate) use ci_scope::nested_scope_correction_retains_complete_original_hostile_and_loan_controls;

#[test]
fn complete_nested_custody_assertions_remain_executable_and_byte_exact() {
    use sha2::{Digest, Sha256};
    ci_scope::nested_scope_correction_retains_complete_original_hostile_and_loan_controls();
    let original = include_str!(
        "../../docs/evidence/nested-byte-traversal-780/ci-scope-custody-function.before.txt"
    );
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
        ),
        "e42c74a514afe34fef7c9b1fd0bc17368f59dce64d2447d4b0580bf73756064e"
    );
    let relocated = original.replacen("    fn ", "    pub(crate) fn ", 1);
    assert_eq!(
        include_str!("nested_scope.rs").matches(&relocated).count(),
        1
    );
}
