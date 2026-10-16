//! Whole-wire bridge from the hosted c11a63509 capture to every older control.
use super::digest_without_checked_value_cursor_budget;
use sha2::{Digest, Sha256};

#[path = "before_c11.rs"]
mod before_c11;

const CURRENT: [&str; 2] = [
    include_str!("../../../docs/evidence/browser-graph-pins-c11a63509/current-hosted-calculator-baseline.json"),
    include_str!("../../../docs/evidence/browser-graph-pins-c11a63509/current-hosted-calculator-renamed.json"),
];
const PREVIOUS: [&str; 2] = [
    include_str!("../../../docs/evidence/browser-graph-pins-c451086ce/current-hosted-calculator-baseline.json"),
    include_str!("../../../docs/evidence/browser-graph-pins-c451086ce/current-hosted-calculator-renamed.json"),
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
    assert_eq!(parsed["budget"]["used_builder_bytes"], 2_967_742);
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
    assert_eq!(prior["budget"]["used_builder_bytes"], 2_971_194);
    let field = "\"used_builder_bytes\":2967742";
    assert_eq!(graph.matches(field).count(), 1);
    let restored = graph.replacen(field, "\"used_builder_bytes\":2971194", 1);
    let field = format!("\"graph_digest\":{}", parsed["graph_digest"]);
    assert_eq!(restored.matches(&field).count(), 1);
    let restored = restored.replacen(
        &field,
        &format!("\"graph_digest\":{}", prior["graph_digest"]),
        1,
    );
    assert_eq!(restored, previous, "every prior hosted graph byte");
    before_c11::digest(&restored)
}

#[test]
fn hosted_wires_keep_current_pins_and_every_complete_previous_receiver() {
    let original = [
        "sha256:489f02ed298c2d35433d6c9f811ccc26c50ce90117b3976a862b40aff11129b8",
        "sha256:ccb0ec81a00f3890ec2fe87af8d0f801970a1a397eeb55fe115e83dab02188ba",
    ];
    let runner =
        include_str!("../../../docs/evidence/main65-private-merge-20261016/main-runner.mjs.txt");
    let runner = c11_runner_input(runner);
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
        "../../../docs/evidence/browser-graph-pins-c11a63509/historical_builder.before.rs.txt"
    );
    let runner = include_str!(
        "../../../docs/evidence/browser-graph-pins-c11a63509/test-fixtures.before.mjs"
    );
    for (source, expected) in [
        (
            original,
            "fbdc934cb47d6031aefd0ecc2945a575f4e74050b8e9b927e9cf7907c452ff7c",
        ),
        (
            runner,
            "e2dcb8f6a31a753891aeb04fa5269a28c555ec686a7945a8c5080bd54d7178a3",
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
    let current = include_str!("before_c11.rs");
    let current = c11_before_input(current);
    let new = concat!(
        "include_str!(\n",
        "        \"../../../docs/evidence/browser-graph-pins-c11a63509/test-fixtures.before.mjs\"\n",
        "    )"
    );
    let old = "include_str!(\"../../../platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs\")";
    assert_eq!(current.matches(new).count(), 1);
    assert_eq!(current.replacen(new, old, 1), original);
}

pub(super) fn c11_before2600_input(current: &str) -> String {
    assert_eq!(
        format!(
            "{:x}",
            semaprax::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
        ),
        "30cc9238ae44dfd6988023f1296eb5c7c32b084ed164b64a1c194e4c5a2b758b"
    );
    let mut restored = current.to_owned();
    assert_eq!(restored.matches("include_str!(\"../../../docs/evidence/main65-private-merge-20261016/main-browser-parent.rs.txt\")").count(), 1);
    restored = restored.replacen("include_str!(\"../../../docs/evidence/main65-private-merge-20261016/main-browser-parent.rs.txt\")", "include_str!(\"../scalar_browser_ci_contract.rs\")", 1);
    assert_eq!(
        format!(
            "{:x}",
            semaprax::digest_hex::LowerHex(Sha256::digest(restored.as_bytes()))
        ),
        "e9a110ae30efac1012deed4c9ac4750dbc4f82e4cb698f1b33508872d968ca26"
    );
    restored
}

pub(super) fn c11_before451_input(current: &str) -> String {
    assert_eq!(
        format!(
            "{:x}",
            semaprax::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
        ),
        "1e593e61a5617473cdf1faf1cf8eb009090d0bead4cbd81a8f849b0b1205723a"
    );
    let mut restored = current.to_owned();
    assert_eq!(
        restored.matches("#[path = \"c11_before_2600.rs\"]").count(),
        1
    );
    restored = restored.replacen(
        "#[path = \"c11_before_2600.rs\"]",
        "#[path = \"before_2600.rs\"]",
        1,
    );
    assert_eq!(restored.matches("let current = include_str!(\"c11_before_2600.rs\");\n    let current = super::super::c11_before2600_input(current);").count(), 1);
    restored = restored.replacen("let current = include_str!(\"c11_before_2600.rs\");\n    let current = super::super::c11_before2600_input(current);", "let current = include_str!(\"before_2600.rs\");", 1);
    assert_eq!(
        format!(
            "{:x}",
            semaprax::digest_hex::LowerHex(Sha256::digest(restored.as_bytes()))
        ),
        "c2a37b465fbeba1cae400ea2316fce996b8561bd5523f73100c0b1904f967fc2"
    );
    restored
}

pub(super) fn c11_before_input(current: &str) -> String {
    assert_eq!(
        format!(
            "{:x}",
            semaprax::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
        ),
        "16386e03fc572dddc250937eb1b4fa606d5a2edc2abc7c70003254f2e0b36585"
    );
    let mut restored = current.to_owned();
    assert_eq!(
        restored.matches("#[path = \"before_c451_c11.rs\"]").count(),
        1
    );
    restored = restored.replacen(
        "#[path = \"before_c451_c11.rs\"]",
        "#[path = \"before_c451.rs\"]",
        1,
    );
    assert_eq!(restored.matches("let current = include_str!(\"before_c451_c11.rs\");\n    let current = super::c11_before451_input(current);").count(), 1);
    restored = restored.replacen("let current = include_str!(\"before_c451_c11.rs\");\n    let current = super::c11_before451_input(current);", "let current = include_str!(\"before_c451.rs\");", 1);
    assert_eq!(
        format!(
            "{:x}",
            semaprax::digest_hex::LowerHex(Sha256::digest(restored.as_bytes()))
        ),
        "11759a5634608fd672855964935d2e484d909934f771faf41177eaca0caaf863"
    );
    restored
}

fn c11_runner_input(runner: &str) -> &str {
    assert_eq!(
        format!(
            "{:x}",
            semaprax::digest_hex::LowerHex(Sha256::digest(runner.as_bytes()))
        ),
        "d7b15f2bdc93ee2fdbc118baddadfb6f888991b94550d1c048cb01d159ce6e02"
    );
    runner
}
