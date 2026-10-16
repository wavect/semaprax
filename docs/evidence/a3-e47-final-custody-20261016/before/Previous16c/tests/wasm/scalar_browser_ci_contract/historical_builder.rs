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
const CURRENT: [&str; 2] = [
    include_str!("../../../docs/evidence/browser-graph-pins-35cc-20261016/actual-capture/calculator-baseline.json"),
    include_str!("../../../docs/evidence/browser-graph-pins-35cc-20261016/actual-capture/calculator-renamed.json"),
];

/// Restore only the actual new bill and self-digest, authenticate every other
/// byte against the complete previous capture, then run the original bridge.
pub(super) fn current_digest(graph: &str) -> String {
    let parsed: serde_json::Value = serde_json::from_str(graph).unwrap();
    assert_eq!(
        graph_digest(graph),
        parsed["graph_digest"].as_str().unwrap()
    );
    assert_eq!(parsed["budget"]["used_builder_bytes"], 4_032_239);
    let previous = CAPTURED
        .iter()
        .map(|wire| wire.strip_suffix('\n').unwrap())
        .find(|wire| {
            serde_json::from_str::<serde_json::Value>(wire).unwrap()["workspace_revision"]
                == parsed["workspace_revision"]
        })
        .expect("complete previous baseline or renamed browser wire");
    let prior: serde_json::Value = serde_json::from_str(previous).unwrap();
    assert_eq!(
        graph_digest(previous),
        prior["graph_digest"].as_str().unwrap()
    );
    assert_eq!(prior["budget"]["used_builder_bytes"], 3_930_276);
    let field = "\"used_builder_bytes\":4032239";
    assert_eq!(graph.matches(field).count(), 1);
    let restored = graph.replacen(field, "\"used_builder_bytes\":3930276", 1);
    let field = format!("\"graph_digest\":{}", parsed["graph_digest"]);
    assert_eq!(restored.matches(&field).count(), 1);
    let restored = restored.replacen(
        &field,
        &format!("\"graph_digest\":{}", prior["graph_digest"]),
        1,
    );
    assert_eq!(
        restored, previous,
        "complete immediate previous browser wire"
    );
    digest(&restored)
}

fn prior_runner(current: &str) -> String {
    let mut previous = current.to_owned();
    let note = "// Actual local 35cc compiler records include the complete current builder bill.\n// The Rust browser contract retains every complete historical wire and cursor\n// control; docs/evidence/browser-graph-pins-35cc-20261016 binds this capture.";
    assert_eq!(previous.matches(note).count(), 1);
    previous = previous.replacen(note, "// Exact hosted compiler records from 268cb0523, including the complete current\n// builder bill. The Rust browser contract retains full historical wire and\n// cursor controls; docs/evidence/browser-graph-pins-268cb0523 binds this capture.", 1);
    assert_eq!(
        previous
            .matches("sha256:82b22b0f2c49b594045bef876d491bb182df03a734b34b9a7430464e0c38f347")
            .count(),
        1
    );
    previous = previous.replacen(
        "sha256:82b22b0f2c49b594045bef876d491bb182df03a734b34b9a7430464e0c38f347",
        "sha256:dcd4f20c52e9b8cc52ac7695f4ef7f0668a4bb5d7c735434ede9e0c0361ee363",
        1,
    );
    assert_eq!(
        previous
            .matches("sha256:d47fc1d22cb152e19f9f99e64ff2d675babb111a75c9d4aaac639d032729abcf")
            .count(),
        1
    );
    previous = previous.replacen(
        "sha256:d47fc1d22cb152e19f9f99e64ff2d675babb111a75c9d4aaac639d032729abcf",
        "sha256:322b05af9bab447a12c7f9f2cda2ef3f7b2959ced4b68af04a716aa54b127122",
        1,
    );
    previous
}

fn prior_contract(current: &str) -> String {
    let current = custody::parent_for_ours(current);
    let current_call =
        "previous_digests.push(historical_builder::current_digest(revision.semantic_graph()));";
    assert_eq!(current.matches(current_call).count(), 1);
    current.replacen(
        current_call,
        "previous_digests.push(historical_builder::digest(revision.semantic_graph()));",
        1,
    )
}

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
    let runner = custody::runner_for_ours(runner);
    for (wire, expected) in CURRENT.into_iter().zip(original) {
        let graph = wire.strip_suffix('\n').unwrap();
        assert_eq!(current_digest(graph), expected);
        assert!(runner.contains(&graph_digest(graph)));
    }
    let runner = prior_runner(&runner);
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
        assert!(std::panic::catch_unwind(|| current_digest(&changed)).is_err());
    }
    for captured in ACTUAL47 {
        let graph = captured.strip_suffix('\n').unwrap();
        let parsed: serde_json::Value = serde_json::from_str(graph).unwrap();
        let sites = parsed["budget"]["used_call_sites"].as_u64().unwrap();
        let field = format!("\"used_call_sites\":{sites}");
        assert_eq!(graph.matches(&field).count(), 1);
        let changed = graph.replacen(&field, &format!("\"used_call_sites\":{}", sites + 1), 1);
        let changed = changed.replacen(
            &format!("\"graph_digest\":{}", parsed["graph_digest"]),
            &format!("\"graph_digest\":\"{}\"", graph_digest(&changed)),
            1,
        );
        assert!(std::panic::catch_unwind(|| final_digest(&changed)).is_err());
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
    let current = prior_contract(current);
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

// Independent actual MAIN2600 capture and its complete previous receiver.
use super::digest_without_checked_value_cursor_budget;
#[path = "browser_custody.rs"]
mod custody;
#[path = "historical_main2600.rs"]
mod main2600;
pub(super) fn main_digest(graph: &str) -> String {
    main2600::digest(graph)
}

// Actual source-bound47f capture; every older digest receiver stays independent.
const ACTUAL47: [&str; 2] = [
    include_str!("../../../docs/evidence/actual47f-pin-fixture-successor-20261016/actual-browser/calculator-baseline.json"),
    include_str!("../../../docs/evidence/actual47f-pin-fixture-successor-20261016/actual-browser/calculator-renamed.json"),
];
const OBSERVED364D: [&str; 2] = [
    include_str!("../../../docs/evidence/actual47f-pin-fixture-successor-20261016/observed364d/calculator-baseline.json"),
    include_str!("../../../docs/evidence/actual47f-pin-fixture-successor-20261016/observed364d/calculator-renamed.json"),
];
const OBSERVED2D: [&str; 2] = [
    include_str!("../../../docs/evidence/actual47f-pin-fixture-successor-20261016/observed2d4b/calculator-baseline.json"),
    include_str!("../../../docs/evidence/actual47f-pin-fixture-successor-20261016/observed2d4b/calculator-renamed.json"),
];
const OBSERVED8BB: [&str; 2] = [
    include_str!("../../../docs/evidence/actual47f-pin-fixture-successor-20261016/observed8bb/calculator-baseline.json"),
    include_str!("../../../docs/evidence/actual47f-pin-fixture-successor-20261016/observed8bb/calculator-renamed.json"),
];
const MAIN2600: [&str; 2] = [
    include_str!("../../../docs/evidence/browser-graph-pins-2600e49eb/current-hosted-calculator-baseline.json"),
    include_str!("../../../docs/evidence/browser-graph-pins-2600e49eb/current-hosted-calculator-renamed.json"),
];

pub(super) fn final_digest(graph: &str) -> String {
    let parsed: serde_json::Value = serde_json::from_str(graph).unwrap();
    assert_eq!(graph_digest(graph), parsed["graph_digest"]);
    assert_eq!(parsed["budget"]["used_builder_bytes"], 3477789);
    let index = ACTUAL47
        .iter()
        .position(|wire| {
            let value: serde_json::Value = serde_json::from_str(wire).unwrap();
            value["workspace_revision"] == parsed["workspace_revision"]
        })
        .expect("exact actual baseline or renamed graph");
    assert_eq!(
        graph,
        ACTUAL47[index].strip_suffix('\n').unwrap(),
        "complete actual47f wire"
    );
    let actual = format!(
        "\"used_builder_bytes\":{}",
        parsed["budget"]["used_builder_bytes"]
    );
    let digest_field = format!("\"graph_digest\":{}", parsed["graph_digest"]);
    let mut main = None;
    for (ordinal, previous) in [
        MAIN2600[index],
        CURRENT[index],
        OBSERVED364D[index],
        OBSERVED2D[index],
        OBSERVED8BB[index],
    ]
    .into_iter()
    .enumerate()
    {
        let previous = previous.strip_suffix('\n').unwrap();
        let prior: serde_json::Value = serde_json::from_str(previous).unwrap();
        assert_eq!(graph_digest(previous), prior["graph_digest"]);
        assert_eq!(graph.matches(&actual).count(), 1);
        let restored = graph.replacen(
            &actual,
            &format!(
                "\"used_builder_bytes\":{}",
                prior["budget"]["used_builder_bytes"]
            ),
            1,
        );
        assert_eq!(restored.matches(&digest_field).count(), 1);
        let restored = restored.replacen(
            &digest_field,
            &format!("\"graph_digest\":{}", prior["graph_digest"]),
            1,
        );
        assert_eq!(restored, previous, "entire independently retained wire");
        if ordinal == 0 {
            main = Some(restored);
        }
    }
    main_digest(&main.unwrap())
}

// Complete independent actual Main c451 receiver and all older Main controls.
#[path = "historical_main_c451.rs"]
mod main_c451;
pub(super) fn main_c451_digest(graph: &str) -> String {
    main_c451::digest(graph)
}

// Fresh actual83ca wires; both complete independently retained receivers run.
#[path = "actual_83ca.rs"]
mod actual_83ca;
pub(super) fn current83ca_digest(graph: &str) -> String {
    actual_83ca::digest(graph)
}
pub(super) fn current83ca_drift_controls(graph: &str) {
    actual_83ca::drift_controls(graph);
}

#[path = "historical_main_f82.rs"]
mod main_f82;
