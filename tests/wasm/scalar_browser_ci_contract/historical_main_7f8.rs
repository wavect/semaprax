//! Whole-wire bridge from the hosted 524a80bc9 capture to every older control.
use super::digest_without_checked_value_cursor_budget;
use sha2::{Digest, Sha256};

#[path = "before_524.rs"]
mod before_524;

const CURRENT: [&str; 2] = [
    include_str!("../../../docs/evidence/browser-graph-pins-524a80bc9/current-hosted-calculator-baseline.json"),
    include_str!("../../../docs/evidence/browser-graph-pins-524a80bc9/current-hosted-calculator-renamed.json"),
];
const PREVIOUS: [&str; 2] = [
    include_str!("../../../docs/evidence/browser-graph-pins-75b314f70/current-hosted-calculator-baseline.json"),
    include_str!("../../../docs/evidence/browser-graph-pins-75b314f70/current-hosted-calculator-renamed.json"),
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
    assert_eq!(parsed["budget"]["used_builder_bytes"], 2_917_432);
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
    assert_eq!(prior["budget"]["used_builder_bytes"], 2_954_152);
    let field = "\"used_builder_bytes\":2917432";
    assert_eq!(graph.matches(field).count(), 1);
    let restored = graph.replacen(field, "\"used_builder_bytes\":2954152", 1);
    let field = format!("\"graph_digest\":{}", parsed["graph_digest"]);
    assert_eq!(restored.matches(&field).count(), 1);
    let restored = restored.replacen(
        &field,
        &format!("\"graph_digest\":{}", prior["graph_digest"]),
        1,
    );
    assert_eq!(restored, previous, "every prior hosted graph byte");
    before_524::digest(&restored)
}

#[test]
fn hosted_wires_keep_current_pins_and_every_complete_previous_receiver() {
    let original = [
        "sha256:489f02ed298c2d35433d6c9f811ccc26c50ce90117b3976a862b40aff11129b8",
        "sha256:ccb0ec81a00f3890ec2fe87af8d0f801970a1a397eeb55fe115e83dab02188ba",
    ];
    let runner = include_str!("../../../docs/evidence/main7f8-composition-20261017/runner-main7f8.mjs");
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
        "../../../docs/evidence/browser-graph-pins-524a80bc9/historical_builder.before.rs.txt"
    );
    let runner = include_str!(
        "../../../docs/evidence/browser-graph-pins-524a80bc9/test-fixtures.before.mjs"
    );
    for (source, expected) in [
        (
            original,
            "68cae5ac5707224f8c30ef6f2c9f27c5b93eaed97f25b71dcf98a1c82a0a5c36",
        ),
        (
            runner,
            "f2beafbf9f15748b204f77c90219ed99708929e17b7428d069321f9948594b45",
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
    let current = include_str!("before_524.rs");
    let new = concat!(
        "include_str!(\n",
        "        \"../../../docs/evidence/browser-graph-pins-524a80bc9/test-fixtures.before.mjs\"\n",
        "    )"
    );
    let old = "include_str!(\"../../../platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs\")";
    assert_eq!(current.matches(new).count(), 1);
    assert_eq!(current.replacen(new, old, 1), original);
}

#[test]
fn whole_main7f8_receiver_and_runner_are_authenticated() {
    let original = include_str!("../../../docs/evidence/main7f8-composition-20261017/historical-main7f8.before.rs.txt");
    let runner = include_str!("../../../docs/evidence/main7f8-composition-20261017/runner-main7f8.mjs");
    for (text, expected) in [
        (original, "8651f7dd678016d5ce6d617e249a8cd566fffdb65652e2f1ddf9a9724dbcb747"),
        (runner, "439aac5c76fc97e037bf2e30273c99d842357c64925babbffd7b2007edd486ec"),
    ] {
        assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))), expected);
    }
    let current = include_str!("historical_main_7f8.rs");
    let marker = "\n#[test]\nfn whole_main7f8_receiver_and_runner_are_authenticated() {";
    let end = current.find(marker).unwrap();
    let retained = &current[..end];
    let new = "include_str!(\"../../../docs/evidence/main7f8-composition-20261017/runner-main7f8.mjs\")";
    let old = "include_str!(\"../../../platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs\")";
    assert_eq!(retained.matches(new).count(), 1);
    assert_eq!(retained.replacen(new, old, 1), original);
}
