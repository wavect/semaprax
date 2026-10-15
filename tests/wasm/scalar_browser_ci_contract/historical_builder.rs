//! Exact whole-wire bridge to independently retained historical controls.
use sha2::{Digest, Sha256};

const HISTORICAL: [&str; 2] = [
    include_str!(
        "../../../docs/evidence/browser-graph-pins-841efe3e2/historical-calculator-baseline.json"
    ),
    include_str!(
        "../../../docs/evidence/browser-graph-pins-841efe3e2/historical-calculator-renamed.json"
    ),
];
const CAPTURED: [&str; 2] = [
    include_str!("../../../docs/evidence/browser-graph-pins-17104e20c/current-hosted-calculator-baseline.json"),
    include_str!("../../../docs/evidence/browser-graph-pins-17104e20c/current-hosted-calculator-renamed.json"),
];
fn hash(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    )
}
fn graph_digest(graph: &str) -> String {
    let parsed: serde_json::Value = serde_json::from_str(graph).unwrap();
    let field = format!(",\"graph_digest\":{}", parsed["graph_digest"]);
    assert_eq!(graph.matches(&field).count(), 1);
    hash(
        b"semaprax.project-semantic-graph.artifact-digest.v1\0",
        graph.replacen(&field, "", 1).as_bytes(),
    )
}
/// Test-only reconstruction; the actual pin assertion still uses the entire
/// current graph with its current bill. Every other byte must match the full
/// prior fixture before the complete original cursor helper is invoked.
pub(super) fn digest(graph: &str) -> String {
    let parsed: serde_json::Value = serde_json::from_str(graph).unwrap();
    assert_eq!(
        graph_digest(graph),
        parsed["graph_digest"].as_str().unwrap()
    );
    assert_eq!(parsed["budget"]["used_builder_bytes"], 3_930_276);
    let historical = HISTORICAL
        .iter()
        .map(|text| text.strip_suffix('\n').unwrap())
        .find(|text| {
            serde_json::from_str::<serde_json::Value>(text).unwrap()["workspace_revision"]
                == parsed["workspace_revision"]
        })
        .expect("exact retained baseline or renamed fixture");
    let previous: serde_json::Value = serde_json::from_str(historical).unwrap();
    assert_eq!(previous["budget"]["used_builder_bytes"], 1_214_409);
    assert_eq!(
        graph_digest(historical),
        previous["graph_digest"].as_str().unwrap()
    );
    let field = "\"used_builder_bytes\":3930276";
    assert_eq!(graph.matches(field).count(), 1);
    let restored = graph.replacen(field, "\"used_builder_bytes\":1214409", 1);
    let current_digest = format!("\"graph_digest\":{}", parsed["graph_digest"]);
    assert_eq!(restored.matches(&current_digest).count(), 1);
    let restored = restored.replacen(
        &current_digest,
        &format!("\"graph_digest\":{}", previous["graph_digest"]),
        1,
    );
    assert_eq!(restored, historical, "complete historical graph bytes");
    super::digest_without_checked_value_cursor_budget(&restored)
}
#[test]
fn current_hosted_wires_replay_complete_historical_cursor_controls() {
    let original = [
        "sha256:489f02ed298c2d35433d6c9f811ccc26c50ce90117b3976a862b40aff11129b8",
        "sha256:ccb0ec81a00f3890ec2fe87af8d0f801970a1a397eeb55fe115e83dab02188ba",
    ];
    let runner = include_str!("../../../platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs");
    for (captured, expected) in CAPTURED.into_iter().zip(original) {
        let graph = captured.strip_suffix('\n').unwrap();
        assert_eq!(digest(graph), expected);
        assert!(runner.contains(&graph_digest(graph)));
    }
}
#[test]
fn current_hosted_wires_reproduce_every_prior_byte_after_only_bill_and_digest_restoration() {
    let previous = [
        include_str!("../../../docs/evidence/browser-graph-pins-268cb0523/current-hosted-calculator-baseline.json"),
        include_str!("../../../docs/evidence/browser-graph-pins-268cb0523/current-hosted-calculator-renamed.json"),
    ];
    for (captured, previous) in CAPTURED.into_iter().zip(previous) {
        let graph = captured.strip_suffix('\n').unwrap();
        let previous = previous.strip_suffix('\n').unwrap();
        let parsed: serde_json::Value = serde_json::from_str(graph).unwrap();
        let prior: serde_json::Value = serde_json::from_str(previous).unwrap();
        assert_eq!(graph_digest(graph), parsed["graph_digest"]);
        assert_eq!(graph_digest(previous), prior["graph_digest"]);
        assert_eq!(parsed["budget"]["used_builder_bytes"], 3_930_276);
        assert_eq!(prior["budget"]["used_builder_bytes"], 4_393_980);
        let field = "\"used_builder_bytes\":3930276";
        assert_eq!(graph.matches(field).count(), 1);
        let restored = graph.replacen(field, "\"used_builder_bytes\":4393980", 1);
        let field = format!("\"graph_digest\":{}", parsed["graph_digest"]);
        assert_eq!(restored.matches(&field).count(), 1);
        let restored = restored.replacen(
            &field,
            &format!("\"graph_digest\":{}", prior["graph_digest"]),
            1,
        );
        assert_eq!(restored, previous, "complete prior hosted graph bytes");
    }
}
#[test]
fn a_rehashed_non_builder_budget_mutation_cannot_pass_the_historical_bridge() {
    for captured in CAPTURED {
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
fn complete_prior_browser_contract_has_only_registration_and_call_adapter_changes() {
    let original = include_str!("../../../docs/evidence/browser-graph-pins-841efe3e2/scalar_browser_ci_contract.before.rs.txt");
    assert_eq!(
        format!(
            "{:x}",
            semaprax::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
        ),
        "ea17287522800683e8b257ecdf379866d26bf891dc2b4b34e49c5ee82f206de3"
    );
    let current = include_str!("../scalar_browser_ci_contract.rs");
    let changes = [
        ("#[path = \"scalar_browser_ci_contract/historical_builder.rs\"]\nmod historical_builder;\n\n", ""),
        ("previous_digests.push(historical_builder::digest(revision.semantic_graph()));", "previous_digests.push(digest_without_checked_value_cursor_budget(\n                    revision.semantic_graph(),\n                ));"),
    ];
    let mut restored = current.to_owned();
    for (new, old) in changes {
        assert_eq!(restored.matches(new).count(), 1);
        restored = restored.replacen(new, old, 1);
    }
    assert_eq!(restored, original);
}
