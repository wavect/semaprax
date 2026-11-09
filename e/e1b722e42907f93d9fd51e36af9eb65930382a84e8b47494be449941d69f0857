//! Whole-wire bridge from the hosted 7d34f1269 capture to every older control.
use super::digest_without_checked_value_cursor_budget;
use sha2::{Digest, Sha256};

#[path = "before_7d34.rs"]
mod before_7d34;

const CURRENT: [&str; 2] = [
    include_str!("../../../docs/evidence/browser-graph-pins-7d34f1269/current-hosted-calculator-baseline.json"),
    include_str!("../../../docs/evidence/browser-graph-pins-7d34f1269/current-hosted-calculator-renamed.json"),
];
const PREVIOUS: [&str; 2] = [
    include_str!("../../../docs/evidence/browser-graph-pins-67107c7c5/current-hosted-calculator-baseline.json"),
    include_str!("../../../docs/evidence/browser-graph-pins-67107c7c5/current-hosted-calculator-renamed.json"),
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
    assert_eq!(parsed["budget"]["used_builder_bytes"], 2_696_104);
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
    assert_eq!(prior["budget"]["used_builder_bytes"], 2_927_728);
    let field = "\"used_builder_bytes\":2696104";
    assert_eq!(graph.matches(field).count(), 1);
    let restored = graph.replacen(field, "\"used_builder_bytes\":2927728", 1);
    let field = format!("\"graph_digest\":{}", parsed["graph_digest"]);
    assert_eq!(restored.matches(&field).count(), 1);
    let restored = restored.replacen(
        &field,
        &format!("\"graph_digest\":{}", prior["graph_digest"]),
        1,
    );
    assert_eq!(restored, previous, "every prior hosted graph byte");
    before_7d34::digest(&restored)
}

#[test]
fn hosted_wires_keep_current_pins_and_every_complete_previous_receiver() {
    let original = [
        "sha256:489f02ed298c2d35433d6c9f811ccc26c50ce90117b3976a862b40aff11129b8",
        "sha256:ccb0ec81a00f3890ec2fe87af8d0f801970a1a397eeb55fe115e83dab02188ba",
    ];
    let runner = include_str!(
        "../../../docs/evidence/browser-graph-pins-21abcda9b/test-fixtures.before.mjs"
    );
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
fn the_complete_old_bridge_changes_only_its_retained_runner_include() {
    let original = include_str!(
        "../../../docs/evidence/browser-graph-pins-7d34f1269/historical_builder.before.rs.txt"
    );
    let runner = include_str!(
        "../../../docs/evidence/browser-graph-pins-7d34f1269/test-fixtures.before.mjs"
    );
    for (source, expected) in [
        (
            original,
            "3a53776194bf754a6516972c06d4cd24833df53ab02eea027dd5d1ba32f0cb63",
        ),
        (
            runner,
            "11cfaa78b2fb7df873e2d2f6b5b66c9a06ac33cdb5448cf12bb5ce6a1f216b47",
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
    let current = include_str!("before_7d34.rs");
    let captured_source_stage = crate::main_e177_final_inputs::value_direct(
        "tests/wasm/scalar_browser_ci_contract/before_7d34.rs",
        "eb43b69f6a9deded48d37ed201bb2a3526ae70bf1f90446dfe57987ff80cc836",
        current,
    );
    let current = captured_source_stage.as_str();
    let new = concat!(
        "include_str!(\n",
        "        \"../../../docs/evidence/browser-graph-pins-7d34f1269/test-fixtures.before.mjs\"\n",
        "    )"
    );
    let old = "include_str!(\"../../../platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs\")";
    assert_eq!(current.matches(new).count(), 1);
    assert_eq!(current.replacen(new, old, 1), original);
}
