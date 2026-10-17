//! Whole-wire bridge from complete hosted 3e66 captures; later-head CI is required.
use super::digest_without_checked_value_cursor_budget;
use sha2::{Digest, Sha256};
#[path = "before_3e66.rs"]
mod before_3e66;
const CURRENT:[&str;2]=[
 include_str!("../../../docs/evidence/browser-graph-pins-3e66c0116/current-hosted-calculator-baseline.json"),
 include_str!("../../../docs/evidence/browser-graph-pins-3e66c0116/current-hosted-calculator-renamed.json"),
];
const PREVIOUS:[&str;2]=[
 include_str!("../../../docs/evidence/browser-graph-pins-21abcda9b/current-hosted-calculator-baseline.json"),
 include_str!("../../../docs/evidence/browser-graph-pins-21abcda9b/current-hosted-calculator-renamed.json"),
];

fn graph_digest(graph: &str) -> String {
    let parsed: serde_json::Value = serde_json::from_str(graph).unwrap();
    let field = format!(",\"graph_digest\":{}", parsed["graph_digest"]);
    assert_eq!(graph.matches(&field).count(), 1);
    let payload = graph.replacen(&field, "", 1);
    let mut hash = Sha256::new();
    hash.update(b"semaprax.project-semantic-graph.artifact-digest.v1\0");
    hash.update((payload.len() as u64).to_le_bytes());
    hash.update(payload.as_bytes());
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hash.finalize())
    )
}

pub(super) fn digest(graph: &str) -> String {
    let parsed: serde_json::Value = serde_json::from_str(graph).unwrap();
    assert_eq!(graph_digest(graph), parsed["graph_digest"]);
    assert_eq!(parsed["budget"]["used_builder_bytes"], 2_697_151);
    let previous = PREVIOUS
        .iter()
        .map(|text| text.strip_suffix('\n').unwrap())
        .find(|text| {
            serde_json::from_str::<serde_json::Value>(text).unwrap()["workspace_revision"]
                == parsed["workspace_revision"]
        })
        .expect("exact retained baseline or renamed fixture");
    let prior: serde_json::Value = serde_json::from_str(previous).unwrap();
    assert_eq!(graph_digest(previous), prior["graph_digest"]);
    assert_eq!(prior["budget"]["used_builder_bytes"], 2_691_397);
    let field = "\"used_builder_bytes\":2697151";
    assert_eq!(graph.matches(field).count(), 1);
    let restored = graph.replacen(field, "\"used_builder_bytes\":2691397", 1);
    let field = format!("\"graph_digest\":{}", parsed["graph_digest"]);
    assert_eq!(restored.matches(&field).count(), 1);
    let restored = restored.replacen(
        &field,
        &format!("\"graph_digest\":{}", prior["graph_digest"]),
        1,
    );
    assert_eq!(restored, previous, "every prior hosted graph byte");
    before_3e66::digest(&restored)
}

#[test]
fn hosted_wires_keep_current_pins_and_every_complete_previous_receiver() {
    let original = [
        "sha256:489f02ed298c2d35433d6c9f811ccc26c50ce90117b3976a862b40aff11129b8",
        "sha256:ccb0ec81a00f3890ec2fe87af8d0f801970a1a397eeb55fe115e83dab02188ba",
    ];
    let runner = include_str!("../../../platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs");
    for (captured, expected) in CURRENT.into_iter().zip(original) {
        let graph = captured.strip_suffix('\n').unwrap();
        assert_eq!(digest(graph), expected);
        assert!(runner.contains(&graph_digest(graph)));
    }
}

#[test]
fn a_rehashed_non_builder_change_still_fails_the_complete_bridge() {
    for captured in CURRENT {
        let graph = captured.strip_suffix('\n').unwrap();
        let field = "\"used_call_sites\":10";
        assert_eq!(graph.matches(field).count(), 1);
        let changed = graph.replacen(field, "\"used_call_sites\":11", 1);
        let parsed: serde_json::Value = serde_json::from_str(graph).unwrap();
        let changed = changed.replacen(
            &format!("\"graph_digest\":{}", parsed["graph_digest"]),
            &format!("\"graph_digest\":\"{}\"", graph_digest(&changed)),
            1,
        );
        assert!(std::panic::catch_unwind(|| digest(&changed)).is_err());
    }
}

#[test]
fn every_original_bridge_byte_is_retained_except_its_frozen_runner_include() {
    let original=include_str!("../../../docs/evidence/browser-graph-pins-3e66c0116/tests__wasm__scalar_browser_ci_contract__historical_builder.rs.before.txt");
    let runner=include_str!("../../../docs/evidence/browser-graph-pins-3e66c0116/platform-tests__wasm-scalar-browser-v1__test-fixtures.mjs.before.txt");
    for (source, expected) in [
        (
            original,
            "bb1c19263bf1680e6f58f2711b9bf2bb5c03fbe2a62d34d05d457ca68eb2633c",
        ),
        (
            runner,
            "9bb8345d9ce598c20f3dd8ee3879a1de46ae6dd2fc22c5b3bb9cb9e7801ececd",
        ),
    ] {
        assert_eq!(
            format!(
                "{:x}",
                semaprax::digest_hex::LowerHex(Sha256::digest(source.as_bytes()))
            ),
            expected
        );
    }
    let current = include_str!("before_3e66.rs");
    let new="include_str!(\n        \"../../../docs/evidence/browser-graph-pins-3e66c0116/platform-tests__wasm-scalar-browser-v1__test-fixtures.mjs.before.txt\"\n    )";
    let old = "include_str!(\"../../../platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs\")";
    assert_eq!(current.matches(new).count(), 1);
    assert_eq!(current.replacen(new, old, 1), original);
}

fn restore_source(proof: &serde_json::Value, current: &str) -> String {
    assert_eq!(
        format!(
            "{:x}",
            semaprax::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
        ),
        proof["after_sha256"]
    );
    let mut before = current.to_owned();
    for edit in proof["inverse"].as_array().unwrap() {
        let after = edit["after"].as_str().unwrap();
        assert_eq!(before.matches(after).count(), 1);
        before = before.replacen(after, edit["before"].as_str().unwrap(), 1);
    }
    assert_eq!(
        format!(
            "{:x}",
            semaprax::digest_hex::LowerHex(Sha256::digest(before.as_bytes()))
        ),
        proof["before_sha256"]
    );
    before
}
#[test]
fn all_current_sources_and_three_complete_predecessors_are_authenticated() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let proofs: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../../../docs/evidence/browser-graph-pins-3e66c0116/sources.json"
    ))
    .unwrap();
    assert_eq!(proofs.len(), 3);
    let mut paths = std::collections::BTreeSet::new();
    for proof in &proofs {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        let current = std::fs::read_to_string(root.join(path)).unwrap();
        let before = restore_source(proof, &current);
        assert_eq!(
            before,
            std::fs::read_to_string(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(
            std::panic::catch_unwind(|| restore_source(proof, &(current.clone() + "\n"))).is_err()
        );
        let mut forged = proof.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| restore_source(&forged, &current)).is_err());
    }
}
