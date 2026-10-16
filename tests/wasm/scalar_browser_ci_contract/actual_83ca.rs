//! Actual source83ca browser wires, with independent Main and actual47 receivers.
const ACTUAL: [&str; 2] = [
    include_str!("../../../docs/evidence/actual83ca-pin-successor-20261016/actual-browser/calculator-baseline.json"),
    include_str!("../../../docs/evidence/actual83ca-pin-successor-20261016/actual-browser/calculator-renamed.json"),
];
const MAIN: [&str; 2] = [
    include_str!("../../../docs/evidence/browser-graph-pins-c451086ce/current-hosted-calculator-baseline.json"),
    include_str!("../../../docs/evidence/browser-graph-pins-c451086ce/current-hosted-calculator-renamed.json"),
];
const OLD47: [&str; 2] = [
    include_str!("../../../docs/evidence/actual47f-pin-fixture-successor-20261016/actual-browser/calculator-baseline.json"),
    include_str!("../../../docs/evidence/actual47f-pin-fixture-successor-20261016/actual-browser/calculator-renamed.json"),
];
fn restore(graph: &str, previous: &str) -> String {
    let parsed: serde_json::Value = serde_json::from_str(graph).unwrap();
    let prior: serde_json::Value = serde_json::from_str(previous).unwrap();
    assert_eq!(super::graph_digest(previous), prior["graph_digest"]);
    let field = "\"used_builder_bytes\":3442219";
    assert_eq!(graph.matches(field).count(), 1);
    let restored = graph.replacen(
        field,
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
        "complete independent previous browser wire"
    );
    restored
}
pub(super) fn digest(graph: &str) -> String {
    let parsed: serde_json::Value = serde_json::from_str(graph).unwrap();
    assert_eq!(super::graph_digest(graph), parsed["graph_digest"]);
    assert_eq!(parsed["budget"]["used_builder_bytes"], 3_442_219);
    let index = ACTUAL
        .iter()
        .position(|wire| {
            serde_json::from_str::<serde_json::Value>(wire).unwrap()["workspace_revision"]
                == parsed["workspace_revision"]
        })
        .expect("exact actual83ca baseline or renamed fixture");
    assert_eq!(
        graph,
        ACTUAL[index].strip_suffix('\n').unwrap(),
        "complete actual83ca graph"
    );
    let main = restore(graph, MAIN[index].strip_suffix('\n').unwrap());
    let old47 = restore(graph, OLD47[index].strip_suffix('\n').unwrap());
    let result = super::main_c451_digest(&main);
    assert_eq!(
        result,
        super::final_digest(&old47),
        "both independent complete receivers"
    );
    result
}
pub(super) fn drift_controls(graph: &str) {
    for (before, after) in [
        ("\"used_call_sites\":10", "\"used_call_sites\":11"),
        (
            "\"used_builder_bytes\":3442219",
            "\"used_builder_bytes\":3442220",
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
