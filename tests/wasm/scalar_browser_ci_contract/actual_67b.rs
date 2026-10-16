//! Actual67b graph captures; derived restorations grant no producer authority.
const ACTUAL: [&str; 2] = [
    include_str!("../../../docs/evidence/browser-67b1b-successor-20261016/actual-browser/calculator-baseline.json"),
    include_str!("../../../docs/evidence/browser-67b1b-successor-20261016/actual-browser/calculator-renamed.json"),
];
const ACTUAL83CA: [&str; 2] = [
    include_str!("../../../docs/evidence/actual83ca-pin-successor-20261016/actual-browser/calculator-baseline.json"),
    include_str!("../../../docs/evidence/actual83ca-pin-successor-20261016/actual-browser/calculator-renamed.json"),
];
const MAIN75B: [&str; 2] = [
    include_str!("../../../docs/evidence/browser-graph-pins-75b314f70/current-hosted-calculator-baseline.json"),
    include_str!("../../../docs/evidence/browser-graph-pins-75b314f70/current-hosted-calculator-renamed.json"),
];
fn restore(graph: &str, previous: &str) -> String {
    let parsed: serde_json::Value = serde_json::from_str(graph).unwrap();
    let prior: serde_json::Value = serde_json::from_str(previous).unwrap();
    assert_eq!(super::graph_digest(previous), prior["graph_digest"]);
    let bill = "\"used_builder_bytes\":3468381";
    assert_eq!(graph.matches(bill).count(), 1);
    let restored = graph.replacen(
        bill,
        &format!(
            "\"used_builder_bytes\":{}",
            prior["budget"]["used_builder_bytes"]
        ),
        1,
    );
    let field = format!("\"graph_digest\":{}", parsed["graph_digest"]);
    assert_eq!(restored.matches(&field).count(), 1);
    let restored = restored.replacen(
        &field,
        &format!("\"graph_digest\":{}", prior["graph_digest"]),
        1,
    );
    assert_eq!(
        restored, previous,
        "every independently retained browser byte"
    );
    restored
}
pub(super) fn digest(graph: &str) -> String {
    let parsed: serde_json::Value = serde_json::from_str(graph).unwrap();
    assert_eq!(super::graph_digest(graph), parsed["graph_digest"]);
    assert_eq!(parsed["budget"]["used_builder_bytes"], 3_468_381);
    let index = ACTUAL
        .iter()
        .position(|wire| {
            serde_json::from_str::<serde_json::Value>(wire).unwrap()["workspace_revision"]
                == parsed["workspace_revision"]
        })
        .expect("exact actual67b baseline or renamed fixture");
    assert_eq!(
        graph,
        ACTUAL[index].strip_suffix('\n').unwrap(),
        "complete actual67b graph"
    );
    let actual83ca = restore(graph, ACTUAL83CA[index].strip_suffix('\n').unwrap());
    let main75b = restore(graph, MAIN75B[index].strip_suffix('\n').unwrap());
    let result = super::current83ca_digest(&actual83ca);
    super::current83ca_drift_controls(&actual83ca);
    assert_eq!(
        result,
        super::main_75b::digest(&main75b),
        "both full independent receiver trees"
    );
    result
}
pub(super) fn drift_controls(graph: &str) {
    for (before, after) in [
        ("\"used_call_sites\":10", "\"used_call_sites\":11"),
        (
            "\"used_builder_bytes\":3468381",
            "\"used_builder_bytes\":3468382",
        ),
    ] {
        assert_eq!(graph.matches(before).count(), 1);
        let changed = graph.replacen(before, after, 1);
        let parsed: serde_json::Value = serde_json::from_str(graph).unwrap();
        let field = format!("\"graph_digest\":{}", parsed["graph_digest"]);
        assert_eq!(changed.matches(&field).count(), 1);
        let changed = changed.replacen(
            &field,
            &format!("\"graph_digest\":\"{}\"", super::graph_digest(&changed)),
            1,
        );
        assert!(
            std::panic::catch_unwind(|| digest(&changed)).is_err(),
            "rehashed drift must refuse"
        );
    }
}
#[test]
fn actual67b_wires_run_both_complete_independent_receiver_trees() {
    let runner = include_str!("../../../platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs");
    let runner = super::custody::next_actual_input("platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs", runner);
    let expected = [
        "sha256:489f02ed298c2d35433d6c9f811ccc26c50ce90117b3976a862b40aff11129b8",
        "sha256:ccb0ec81a00f3890ec2fe87af8d0f801970a1a397eeb55fe115e83dab02188ba",
    ];
    for (wire, expected) in ACTUAL.into_iter().zip(expected) {
        let graph = wire.strip_suffix('\n').unwrap();
        assert_eq!(digest(graph), expected);
        assert!(runner.contains(&super::graph_digest(graph)));
        drift_controls(graph);
    }
}
#[test]
fn actual67b_source_inverse_preserves_every_complete_old_guard_input() {
    let rows: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/evidence/browser-67b1b-successor-20261016/current-to-prior.json"
    ))
    .unwrap();
    for row in rows.as_array().unwrap() {
        let source = row["source"].as_str().unwrap();
        let current =
            std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(source))
                .unwrap();
        let previous = super::custody::current67b_input(source, &current);
        assert_eq!(previous, row["edits"][0]["old"].as_str().unwrap());
    }
}
