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
pub(super) mod custody;
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

#[path = "browser_75b_controls.rs"]
mod browser_75b_controls;
#[path = "historical_main_75b.rs"]
mod main_75b;

// Actual authenticated67b graph wires compose both independent receiver trees.
#[path = "actual_67b.rs"]
mod actual_67b;
pub(super) fn current67b_digest(graph: &str) -> String {
    actual_67b::digest(graph)
}
pub(super) fn current67b_drift_controls(graph: &str) {
    actual_67b::drift_controls(graph);
}

#[path = "next_actual.rs"]
mod next_actual;
pub(super) fn next_actual_digest(graph: &str) -> String { next_actual::digest(graph) }
pub(super) fn next_actual_drift_controls(graph: &str) { next_actual::drift_controls(graph); }

// Independent incoming hosted524 receiver and all its older full-wire controls.
#[path = "historical_main_7f8.rs"]
mod main_7f8;
pub(super) fn main7f8_digest(graph: &str) -> String {
    main_7f8::digest(graph)
}

// Independent incoming MAIN671 receiver; its complete prior module is retained.
#[path = "before_671.rs"]
mod before_671;
mod incoming_main671 {
//! Whole-wire bridge from the hosted 67107c7c5 capture to every older control.
use super::digest_without_checked_value_cursor_budget;
use sha2::{Digest, Sha256};

use super::before_671;

const CURRENT: [&str; 2] = [
    include_str!("../../../docs/evidence/browser-graph-pins-67107c7c5/current-hosted-calculator-baseline.json"),
    include_str!("../../../docs/evidence/browser-graph-pins-67107c7c5/current-hosted-calculator-renamed.json"),
];
const PREVIOUS: [&str; 2] = [
    include_str!("../../../docs/evidence/browser-graph-pins-524a80bc9/current-hosted-calculator-baseline.json"),
    include_str!("../../../docs/evidence/browser-graph-pins-524a80bc9/current-hosted-calculator-renamed.json"),
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
    assert_eq!(parsed["budget"]["used_builder_bytes"], 2_927_728);
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
    assert_eq!(prior["budget"]["used_builder_bytes"], 2_917_432);
    let field = "\"used_builder_bytes\":2927728";
    assert_eq!(graph.matches(field).count(), 1);
    let restored = graph.replacen(field, "\"used_builder_bytes\":2917432", 1);
    let field = format!("\"graph_digest\":{}", parsed["graph_digest"]);
    assert_eq!(restored.matches(&field).count(), 1);
    let restored = restored.replacen(
        &field,
        &format!("\"graph_digest\":{}", prior["graph_digest"]),
        1,
    );
    assert_eq!(restored, previous, "every prior hosted graph byte");
    before_671::digest(&restored)
}

#[test]
fn hosted_wires_keep_current_pins_and_every_complete_previous_receiver() {
    let original = [
        "sha256:489f02ed298c2d35433d6c9f811ccc26c50ce90117b3976a862b40aff11129b8",
        "sha256:ccb0ec81a00f3890ec2fe87af8d0f801970a1a397eeb55fe115e83dab02188ba",
    ];
    let runner = include_str!("../../../docs/evidence/browser-current-both-retained-754f-20261017/main671-runner.before.mjs");
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
        "../../../docs/evidence/browser-graph-pins-67107c7c5/historical_builder.before.rs.txt"
    );
    let runner = include_str!(
        "../../../docs/evidence/browser-graph-pins-67107c7c5/test-fixtures.before.mjs"
    );
    for (source, expected) in [
        (
            original,
            "8651f7dd678016d5ce6d617e249a8cd566fffdb65652e2f1ddf9a9724dbcb747",
        ),
        (
            runner,
            "439aac5c76fc97e037bf2e30273c99d842357c64925babbffd7b2007edd486ec",
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
    let current = include_str!("before_671.rs");
    let new = concat!(
        "include_str!(\n",
        "        \"../../../docs/evidence/browser-graph-pins-67107c7c5/test-fixtures.before.mjs\"\n",
        "    )"
    );
    let old = "include_str!(\"../../../platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs\")";
    assert_eq!(current.matches(new).count(), 1);
    assert_eq!(current.replacen(new, old, 1), original);
}
}

// Current raw-pin admission followed by BOTH complete retained browser trees.
// No particular future builder bill is assumed: only the actual self-bound
// bill/digest may differ from either independent complete predecessor.
fn current_browser_with_runner(graph: &str, role: &str, runner: &str) -> String {
    let index = match role { "baseline" => 0, "renamed" => 1, _ => panic!("foreign browser fixture role") };
    let parsed: serde_json::Value = serde_json::from_str(graph).unwrap();
    assert_eq!(graph_digest(graph), parsed["graph_digest"], "complete actual browser self digest");
    // Existing Workspace Semantic Graph production maximum (workspace_graph.rs).
    let bill = parsed["budget"]["used_builder_bytes"].as_u64().unwrap();
    assert!(bill > 0 && bill <= 67_108_864, "actual browser bill stays within the unchanged production cap");
    let marker = format!("name: \"project-{role}\",");
    assert_eq!(runner.matches(&marker).count(), 1, "one raw shipping fixture role");
    let record = runner.split_once(&marker).unwrap().1.split_once("\n  },").unwrap().0;
    for (pin, field) in [("project_revision", "project_revision"), ("workspace_revision", "workspace_revision"), ("project_graph_digest", "graph_digest")] {
        let value = parsed[field].as_str().unwrap();
        assert!(value.starts_with("sha256:") && value.len() == 71);
        let expected = format!("{pin}: \"{value}\"");
        assert_eq!(record.matches(&expected).count(), 1, "actual graph must match its raw shipping role pin");
        assert_eq!(record.matches(&format!("{pin}:")).count(), 1);
    }
    let main671 = [
        include_str!("../../../docs/evidence/browser-graph-pins-67107c7c5/current-hosted-calculator-baseline.json"),
        include_str!("../../../docs/evidence/browser-graph-pins-67107c7c5/current-hosted-calculator-renamed.json"),
    ];
    let next_actual = [
        include_str!("../../../docs/evidence/browser-next-actual-218a9-20261016/actual-browser/calculator-baseline.json"),
        include_str!("../../../docs/evidence/browser-next-actual-218a9-20261016/actual-browser/calculator-renamed.json"),
    ];
    let restore = |previous: &str| {
        let previous = previous.strip_suffix('\n').unwrap();
        let prior: serde_json::Value = serde_json::from_str(previous).unwrap();
        assert_eq!(graph_digest(previous), prior["graph_digest"], "independent complete predecessor self digest");
        let current_field = format!("\"used_builder_bytes\":{bill}");
        assert_eq!(graph.matches(&current_field).count(), 1);
        let restored = graph.replacen(&current_field, &format!("\"used_builder_bytes\":{}", prior["budget"]["used_builder_bytes"].as_u64().unwrap()), 1);
        let digest_field = format!("\"graph_digest\":{}", parsed["graph_digest"]);
        assert_eq!(restored.matches(&digest_field).count(), 1);
        let restored = restored.replacen(&digest_field, &format!("\"graph_digest\":{}", prior["graph_digest"]), 1);
        assert_eq!(restored, previous, "only the actual bill and independently verified digest may differ");
        restored
    };
    let main = restore(main671[index]);
    let ours = restore(next_actual[index]);
    let main_result = incoming_main671::digest(&main);
    let ours_result = next_actual_digest(&ours);
    next_actual_drift_controls(&ours);
    assert_eq!(main_result, ours_result, "both complete retained browser trees");
    main_result
}
mod retained_current_entry {
use super::current_browser_with_runner;
pub(super) fn current_browser_digest(graph: &str, role: &str) -> String {
    current_browser_with_runner(graph, role, include_str!("../../../platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs"))
}
}
pub(super) fn current_browser_drift_controls(graph: &str, role: &str) {
    for (before, after) in [("\"used_call_sites\":10", "\"used_call_sites\":11")] {
        assert_eq!(graph.matches(before).count(), 1);
        let changed = graph.replacen(before, after, 1);
        let parsed: serde_json::Value = serde_json::from_str(graph).unwrap();
        let field = format!("\"graph_digest\":{}", parsed["graph_digest"]);
        let changed = changed.replacen(&field, &format!("\"graph_digest\":\"{}\"", graph_digest(&changed)), 1);
        assert!(std::panic::catch_unwind(|| current_browser_digest(&changed, role)).is_err(), "rehashed drift must refuse current raw pins");
    }
}
#[test]
fn current_browser_bridge_keeps_both_complete_trees_and_refuses_rehashed_foreign_and_wrong_role() {
    // Explicit historical control for the new bridge algorithm. The separate
    // live producer owner admits actual current graphs against the RAW runner.
    let runner = include_str!("../../../docs/evidence/browser-current-both-retained-754f-20261017/main671-runner.before.mjs");
    assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(runner.as_bytes()))), "11cfaa78b2fb7df873e2d2f6b5b66c9a06ac33cdb5448cf12bb5ce6a1f216b47");
    for (index, role) in ["baseline", "renamed"].into_iter().enumerate() {
        let graph = [
            include_str!("../../../docs/evidence/browser-graph-pins-67107c7c5/current-hosted-calculator-baseline.json"),
            include_str!("../../../docs/evidence/browser-graph-pins-67107c7c5/current-hosted-calculator-renamed.json"),
        ][index].strip_suffix('\n').unwrap();
        let expected = ["sha256:489f02ed298c2d35433d6c9f811ccc26c50ce90117b3976a862b40aff11129b8", "sha256:ccb0ec81a00f3890ec2fe87af8d0f801970a1a397eeb55fe115e83dab02188ba"][index];
        assert_eq!(current_browser_with_runner(graph, role, runner), expected);
        for wrong in ["foreign", if role == "baseline" { "renamed" } else { "baseline" }] {
            assert!(std::panic::catch_unwind(|| current_browser_with_runner(graph, wrong, runner)).is_err());
        }
        let field = "\"used_call_sites\":10";
        assert_eq!(graph.matches(field).count(), 1);
        let changed = graph.replacen(field, "\"used_call_sites\":11", 1);
        assert!(std::panic::catch_unwind(|| current_browser_with_runner(&changed, role, runner)).is_err());
        let parsed: serde_json::Value = serde_json::from_str(graph).unwrap();
        let old_digest = parsed["graph_digest"].as_str().unwrap();
        let new_digest = graph_digest(&changed);
        let changed = changed.replacen(&format!("\"graph_digest\":{}", parsed["graph_digest"]), &format!("\"graph_digest\":\"{new_digest}\""), 1);
        for invalid in ["0", "67108865", "true"] {
            let field = "\"used_builder_bytes\":2927728";
            assert_eq!(graph.matches(field).count(), 1);
            let invalid_graph = graph.replacen(field, &format!("\"used_builder_bytes\":{invalid}"), 1);
            let invalid_digest = graph_digest(&invalid_graph);
            let invalid_graph = invalid_graph.replacen(&format!("\"graph_digest\":{}", parsed["graph_digest"]), &format!("\"graph_digest\":\"{invalid_digest}\""), 1);
            let forged_runner = runner.replacen(old_digest, &invalid_digest, 1);
            assert!(std::panic::catch_unwind(|| current_browser_with_runner(&invalid_graph, role, &forged_runner)).is_err());
        }
        // Even forging the role's raw pin cannot authorize any other byte drift.
        let forged_runner = runner.replacen(old_digest, &new_digest, 1);
        assert!(std::panic::catch_unwind(|| current_browser_with_runner(&changed, role, &forged_runner)).is_err());
    }
}

// Independent complete incoming MAIN7d34 receiver. The two finite acquisition
// edits bind its sibling and immutable hosted runner, preserving all old guards.
#[path = "before_7d34.rs"]
mod before_7d34;
mod incoming_main7d34 {
//! Whole-wire bridge from the hosted 7d34f1269 capture to every older control.
use super::digest_without_checked_value_cursor_budget;
use sha2::{Digest, Sha256};

use super::before_7d34;

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
    let runner = include_str!("../../../docs/evidence/main-d285-browser-integration-20261017/main7d34-runner.before.mjs");
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
    let new = concat!(
        "include_str!(\n",
        "        \"../../../docs/evidence/browser-graph-pins-7d34f1269/test-fixtures.before.mjs\"\n",
        "    )"
    );
    let old = "include_str!(\"../../../platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs\")";
    assert_eq!(current.matches(new).count(), 1);
    assert_eq!(current.replacen(new, old, 1), original);
}
}

fn current_browser_all_with_runner(graph: &str, role: &str, runner: &str) -> String {
    // The unchanged full old bridge authenticates self-digest, original cap,
    // actual raw role pins and both complete 671/next_actual predecessor trees.
    let expected = current_browser_with_runner(graph, role, runner);
    let index = match role { "baseline" => 0, "renamed" => 1, _ => unreachable!() };
    let previous = [
        include_str!("../../../docs/evidence/browser-graph-pins-7d34f1269/current-hosted-calculator-baseline.json"),
        include_str!("../../../docs/evidence/browser-graph-pins-7d34f1269/current-hosted-calculator-renamed.json"),
    ][index].strip_suffix('\n').unwrap();
    let parsed: serde_json::Value = serde_json::from_str(graph).unwrap();
    let prior: serde_json::Value = serde_json::from_str(previous).unwrap();
    assert_eq!(graph_digest(previous), prior["graph_digest"], "complete MAIN7d34 self digest");
    let bill = parsed["budget"]["used_builder_bytes"].as_u64().unwrap();
    let bill_field = format!("\"used_builder_bytes\":{bill}");
    assert_eq!(graph.matches(&bill_field).count(), 1);
    let restored = graph.replacen(&bill_field,
        &format!("\"used_builder_bytes\":{}", prior["budget"]["used_builder_bytes"].as_u64().unwrap()), 1);
    let digest_field = format!("\"graph_digest\":{}", parsed["graph_digest"]);
    assert_eq!(restored.matches(&digest_field).count(), 1);
    let restored = restored.replacen(&digest_field, &format!("\"graph_digest\":{}", prior["graph_digest"]), 1);
    assert_eq!(restored, previous, "every complete independent MAIN7d34 byte");
    assert_eq!(incoming_main7d34::digest(&restored), expected, "all complete browser receiver trees");
    expected
}

pub(super) fn current_browser_digest(graph: &str, role: &str) -> String {
    // Preserve the complete previous entry point and its raw shipping acquisition.
    let expected = retained_current_entry::current_browser_digest(graph, role);
    let current = current_browser_all_with_runner(graph, role,
        include_str!("../../../platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs"));
    assert_eq!(current, expected);
    current
}

#[test]
fn main7d34_browser_bridge_preserves_three_complete_trees_and_raw_role_refusals() {
    // Historical MAIN7d34 capture control, not a current integration producer.
    let runner = include_str!("../../../docs/evidence/main-d285-browser-integration-20261017/main7d34-runner.before.mjs");
    for (index, role) in ["baseline", "renamed"].into_iter().enumerate() {
        let graph = [
            include_str!("../../../docs/evidence/browser-graph-pins-7d34f1269/current-hosted-calculator-baseline.json"),
            include_str!("../../../docs/evidence/browser-graph-pins-7d34f1269/current-hosted-calculator-renamed.json"),
        ][index].strip_suffix('\n').unwrap();
        let expected = ["sha256:489f02ed298c2d35433d6c9f811ccc26c50ce90117b3976a862b40aff11129b8", "sha256:ccb0ec81a00f3890ec2fe87af8d0f801970a1a397eeb55fe115e83dab02188ba"][index];
        assert_eq!(current_browser_all_with_runner(graph, role, runner), expected);
        for wrong in ["foreign", if role == "baseline" { "renamed" } else { "baseline" }] {
            assert!(std::panic::catch_unwind(|| current_browser_all_with_runner(graph, wrong, runner)).is_err());
        }
        let before = "\"used_call_sites\":10";
        assert_eq!(graph.matches(before).count(), 1);
        let changed = graph.replacen(before, "\"used_call_sites\":11", 1);
        let parsed: serde_json::Value = serde_json::from_str(graph).unwrap();
        let digest = graph_digest(&changed);
        let changed = changed.replacen(&format!("\"graph_digest\":{}", parsed["graph_digest"]),
            &format!("\"graph_digest\":\"{digest}\""), 1);
        let forged_runner = runner.replacen(parsed["graph_digest"].as_str().unwrap(), &digest, 1);
        assert!(std::panic::catch_unwind(|| current_browser_all_with_runner(&changed, role, &forged_runner)).is_err());
    }
}
