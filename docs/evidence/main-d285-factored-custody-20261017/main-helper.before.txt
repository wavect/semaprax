//! Complete file custody assertions belong to the test source-reader layer.
mod receivers {
    use sha2::{Digest, Sha256};
    macro_rules! include_str {
        ("tests.rs") => {
            ::std::include_str!("../cleanup_plan/replay/factored/canonical_shared/tests.rs")
        };
    }
    pub(crate) fn complete_predecessors_and_new_target_sources_are_bound() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let evidence = root.join("docs/evidence/factored-flat-targets-v1");
        let proof: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(evidence.join("sources.json")).unwrap())
                .unwrap();
        let digest = |text: &str| {
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
            )
        };
        for record in proof["predecessors"].as_array().unwrap() {
            let before =
                std::fs::read_to_string(evidence.join(record["archive"].as_str().unwrap()))
                    .unwrap();
            let current =
                std::fs::read_to_string(root.join(record["path"].as_str().unwrap())).unwrap();
            assert_eq!(digest(&before), record["before_sha256"]);
            assert_eq!(digest(&current), record["after_sha256"]);
            let restore = |input: &str| -> Option<String> {
                let mut output = input.to_owned();
                for edit in record["inverse"].as_array().unwrap() {
                    let after = edit["after"].as_str().unwrap();
                    if output.matches(after).count() != 1 {
                        return None;
                    }
                    output = output.replacen(after, edit["before"].as_str().unwrap(), 1);
                }
                Some(output)
            };
            assert_eq!(restore(&current).as_deref(), Some(before.as_str()));
            assert_ne!(
                restore(&(current.clone() + "\n")).as_deref(),
                Some(before.as_str())
            );
            let mut changed = current.clone();
            changed.replace_range(..1, "!");
            assert_ne!(restore(&changed).as_deref(), Some(before.as_str()));
        }
        for record in proof["new_files"].as_array().unwrap() {
            let current =
                std::fs::read_to_string(root.join(record["path"].as_str().unwrap())).unwrap();
            assert_eq!(digest(&current), record["sha256"]);
            assert_ne!(digest(&(current + "\n")), record["sha256"]);
        }
    }
    pub(crate) fn complete_outer_label_sources_and_original_producer_remain_exact() {
        use sha2::{Digest, Sha256};
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let evidence = root.join("docs/evidence/factored-flat-labels-v1");
        let proof: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(evidence.join("sources.json")).unwrap())
                .unwrap();
        let digest = |text: &str| {
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
            )
        };
        assert_eq!(proof["predecessors"].as_array().unwrap().len(), 4);
        for record in proof["predecessors"].as_array().unwrap() {
            let before =
                std::fs::read_to_string(evidence.join(record["archive"].as_str().unwrap()))
                    .unwrap();
            let current =
                std::fs::read_to_string(root.join(record["path"].as_str().unwrap())).unwrap();
            assert_eq!(digest(&before), record["before_sha256"]);
            assert_eq!(digest(&current), record["after_sha256"]);
            let restore = |input: &str| -> Option<String> {
                let mut output = input.to_owned();
                for edit in record["inverse"].as_array().unwrap() {
                    let after = edit["after"].as_str().unwrap();
                    if output.matches(after).count() != 1 {
                        return None;
                    }
                    output = output.replacen(after, edit["before"].as_str().unwrap(), 1);
                }
                Some(output)
            };
            assert_eq!(restore(&current).as_deref(), Some(before.as_str()));
            assert_ne!(
                restore(&(current.clone() + "\n")).as_deref(),
                Some(before.as_str())
            );
            let mut changed = current.clone();
            changed.replace_range(..1, "!");
            assert_ne!(restore(&changed).as_deref(), Some(before.as_str()));
        }
        for record in proof["new_files"].as_array().unwrap() {
            let current =
                std::fs::read_to_string(root.join(record["path"].as_str().unwrap())).unwrap();
            assert_eq!(digest(&current), record["sha256"]);
            assert_ne!(digest(&(current + "\n")), record["sha256"]);
        }
        let before = std::fs::read_to_string(
            evidence.join("src__cleanup_plan__replay__factored__canonical_shared.rs.before.txt"),
        )
        .unwrap();
        let start = before
            .find("        let mut successors = BTreeMap::<Label, targets::Targets>::new();\n")
            .unwrap();
        let end = start
            + before[start..]
                .find("\n    }\n\n    pub(super) fn id(")
                .unwrap();
        let old_body = before[start..end].replace("self.automaton", "automaton");
        assert_eq!(include_str!("tests.rs").matches(&old_body).count(), 1);
    }
}
pub(crate) use receivers::{
    complete_outer_label_sources_and_original_producer_remain_exact,
    complete_predecessors_and_new_target_sources_are_bound,
};

#[test]
fn complete_factored_custody_assertions_remain_executable_and_byte_exact() {
    use sha2::{Digest, Sha256};
    receivers::complete_predecessors_and_new_target_sources_are_bound();
    let original =
        include_str!("../../docs/evidence/factored-flat-targets-v1/custody-function.before.txt");
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
        ),
        "5054407bbf53475c5d9af764cf2b94018609ef6f13ee4e07e7987ae9d79ec116"
    );
    let relocated = original.replacen("    fn ", "    pub(crate) fn ", 1);
    assert_eq!(
        include_str!("factored_sources.rs")
            .matches(&relocated)
            .count(),
        1
    );
    receivers::complete_outer_label_sources_and_original_producer_remain_exact();
    let original =
        include_str!("../../docs/evidence/factored-flat-labels-v1/custody-function.before.txt");
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
        ),
        "591d2884914ff7a1d43ad796f27f4fca6a96de19e82f373e7d1eeaaa4af8ea42"
    );
    let relocated = original.replacen("    fn ", "    pub(crate) fn ", 1);
    assert_eq!(
        include_str!("factored_sources.rs")
            .matches(&relocated)
            .count(),
        1
    );
}
