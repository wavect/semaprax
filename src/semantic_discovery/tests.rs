use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{json, Value};

use super::*;
use crate::graph::{AgentContextDirection, AgentContextFilter, AgentContextV2Options};

fn write_temp(source: &str) -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "semaprax-semantic-discovery-unit-{}-{}.spx",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::write(&path, source).unwrap();
    path
}

fn forward_options(depth: usize) -> AgentContextV2Options {
    AgentContextV2Options::new(
        depth,
        64 * 1024,
        256,
        [
            AgentContextFilter::Contracts,
            AgentContextFilter::Ownership,
            AgentContextFilter::Effects,
            AgentContextFilter::Types,
        ],
        AgentContextDirection::Forward,
    )
    .expect("options are in bounds")
}

/// Builds the exact v2 document + revision a client would have received for
/// `source`, independent of `compute_context_delta`, so tests can construct
/// an "already acknowledged" base without calling the function under test.
fn base_snapshot(source: &str, symbol: &str, options: &AgentContextV2Options) -> (String, String) {
    let program = crate::parse(source, "fixture.spx").expect("fixture parses");
    let revision = graph::revision(&program);
    let document = graph::agent_context_v2_json(&program, symbol, options)
        .expect("fixture resolves")
        .expect("symbol exists");
    (document, revision)
}

const V1_BASE: &str = "module test.discovery;

@id(\"app.helper\")
fn helper(value: i64) -> i64 { value }

@id(\"app.main\")
fn main() -> i64 { helper(0) }
";

const V2_RENAMED: &str = "module test.discovery;

@id(\"app.helper\")
fn helper_renamed(value: i64) -> i64 { value }

@id(\"app.main\")
fn main() -> i64 { helper_renamed(0) }
";

const V3_EFFECT_CHANGED: &str = "module test.discovery;

permit { process }

@id(\"app.helper\")
fn helper(value: i64) -> i64 uses { process } { value }

@id(\"app.main\")
fn main() -> i64 uses { process } { helper(0) }
";

const V4_ADDED_CALLEE: &str = "module test.discovery;

@id(\"app.helper\")
fn helper(value: i64) -> i64 { value }

@id(\"app.extra\")
fn extra(value: i64) -> i64 { value }

@id(\"app.main\")
fn main() -> i64 { helper(0) + extra(0) }
";

const V5_REMOVED_CALLEE: &str = "module test.discovery;

@id(\"app.helper\")
fn helper(value: i64) -> i64 { value }

@id(\"app.main\")
fn main() -> i64 { 0 }
";

// ---------------------------------------------------------------------
// Discovery manifest
// ---------------------------------------------------------------------

#[test]
fn discovery_manifest_is_compact_and_measures_well_under_its_default_budget() {
    let path = write_temp(V1_BASE);
    let envelope = generate_discovery_manifest(&path, &DiscoveryOptions::default())
        .expect("discovery manifest generates");
    // The measured byte size is the headline compactness evidence for this
    // module: the whole envelope (operation catalog, tool classes, selected
    // target binding and known limitations) fits in a few kilobytes, not a
    // full graph dump.
    assert!(
        envelope.len() < 4096,
        "expected the discovery manifest to stay compact, was {} bytes",
        envelope.len()
    );
    let value: Value = serde_json::from_str(&envelope).expect("valid JSON");
    assert_eq!(value["schema"], DISCOVERY_SCHEMA);
    assert_eq!(
        value["payload"]["selected_target"]["path"],
        path.display().to_string()
    );
    let operations = value["payload"]["operations"]
        .as_array()
        .expect("operations array");
    assert_eq!(operations.len(), SEMANTIC_DISCOVERY_OPERATIONS.len());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn discovery_manifest_generation_is_byte_identical_on_repetition() {
    let path = write_temp(V1_BASE);
    let options = DiscoveryOptions::default();
    let first = generate_discovery_manifest(&path, &options).expect("generates");
    let second = generate_discovery_manifest(&path, &options).expect("generates");
    assert_eq!(first, second, "repeated generation must be byte-identical");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn catalog_is_sorted_by_name_and_covers_every_tool_class() {
    let names: Vec<&str> = SEMANTIC_DISCOVERY_OPERATIONS
        .iter()
        .map(|entry| entry.name)
        .collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(
        names, sorted,
        "operation catalog must be sorted by name bytes"
    );

    let mut classes: Vec<&str> = SEMANTIC_DISCOVERY_OPERATIONS
        .iter()
        .map(|entry| entry.tool_class)
        .collect();
    classes.sort_unstable();
    classes.dedup();
    assert_eq!(
        classes,
        vec![
            "read_only_delta",
            "read_only_help",
            "read_only_query",
            "read_only_report",
            "read_only_schema",
        ]
    );
}

#[test]
fn discovery_manifest_verifies_against_its_exact_source_and_fails_closed_on_drift() {
    let path = write_temp(V1_BASE);
    let envelope = generate_discovery_manifest(&path, &DiscoveryOptions::default())
        .expect("discovery manifest generates");
    verify_discovery_manifest_against_source(&envelope, &path)
        .expect("manifest verifies against its own exact source");

    // Source drift after generation must be reported, not silently accepted:
    // a cached manifest describing a since-changed module is stale
    // capability information.
    std::fs::write(&path, V3_EFFECT_CHANGED).unwrap();
    let outcome = verify_discovery_manifest_against_source(&envelope, &path);
    assert!(
        outcome.is_err(),
        "a manifest bound to the old revision must not verify against drifted source"
    );
    let _ = std::fs::remove_file(&path);
}

// ---------------------------------------------------------------------
// Context delta: end-to-end via compute_context_delta
// ---------------------------------------------------------------------

#[test]
fn unchanged_revision_yields_an_unchanged_outcome_with_no_diff() {
    let path = write_temp(V1_BASE);
    let options = forward_options(1);
    let (base_document, base_revision) = base_snapshot(V1_BASE, "app.main", &options);

    let request = ContextDeltaRequest {
        symbol: "app.main",
        options: &options,
        claimed_base_revision: &base_revision,
        base_document: &base_document,
    };
    let delta = compute_context_delta(&path, &request).expect("delta computes");
    let value: Value = serde_json::from_str(&delta).expect("valid JSON");
    assert_eq!(value["schema"], CONTEXT_DELTA_SCHEMA);
    assert_eq!(value["outcome"], "unchanged");
    assert_eq!(value["revision"], base_revision);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn compute_context_delta_is_deterministic_for_the_same_revision_pair() {
    let path = write_temp(V1_BASE);
    let options = forward_options(1);
    let (base_document, base_revision) = base_snapshot(V1_BASE, "app.main", &options);
    std::fs::write(&path, V4_ADDED_CALLEE).unwrap();

    let request = ContextDeltaRequest {
        symbol: "app.main",
        options: &options,
        claimed_base_revision: &base_revision,
        base_document: &base_document,
    };
    let first = compute_context_delta(&path, &request).expect("delta computes");
    let second = compute_context_delta(&path, &request).expect("delta computes");
    assert_eq!(
        first, second,
        "the same revision pair must produce byte-identical delta output"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_display_rename_changes_only_the_renamed_facts_and_preserves_identity() {
    let path = write_temp(V1_BASE);
    let options = forward_options(1);
    let (base_document, base_revision) = base_snapshot(V1_BASE, "app.main", &options);
    std::fs::write(&path, V2_RENAMED).unwrap();

    let request = ContextDeltaRequest {
        symbol: "app.main",
        options: &options,
        claimed_base_revision: &base_revision,
        base_document: &base_document,
    };
    let delta = compute_context_delta(&path, &request).expect("delta computes");
    let value: Value = serde_json::from_str(&delta).expect("valid JSON");
    assert_eq!(value["outcome"], "delta");
    let diff = &value["diff"];
    assert_eq!(diff["added"].as_array().unwrap().len(), 0);
    assert_eq!(diff["removed"].as_array().unwrap().len(), 0);
    let changed = diff["changed"].as_array().unwrap();
    assert_eq!(
        changed.len(),
        1,
        "only the renamed declaration's facts change"
    );
    assert_eq!(changed[0]["id"], "app.helper");
    assert_eq!(changed[0]["name"], "helper_renamed");
    assert_eq!(
        diff["unchanged_count"], 1,
        "app.main itself is unaffected by its callee's rename"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn an_effect_declaration_change_updates_only_the_relevant_context_facts() {
    let path = write_temp(V1_BASE);
    let options = forward_options(1);
    let (base_document, base_revision) = base_snapshot(V1_BASE, "app.main", &options);
    std::fs::write(&path, V3_EFFECT_CHANGED).unwrap();

    let request = ContextDeltaRequest {
        symbol: "app.main",
        options: &options,
        claimed_base_revision: &base_revision,
        base_document: &base_document,
    };
    let delta = compute_context_delta(&path, &request).expect("delta computes");
    let value: Value = serde_json::from_str(&delta).expect("valid JSON");
    let diff = &value["diff"];
    // This language's typed effects require every transitive caller of an
    // effectful function to also declare that effect (SPX-E102), so a new
    // `uses` on `helper` necessarily changes `main`'s own fact too: the diff
    // must name exactly the two facts that actually differ, no more and no
    // fewer, and must not silently drop `main`'s change to stay "minimal".
    let changed_ids: std::collections::BTreeSet<&str> = diff["changed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|fact| fact["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        changed_ids,
        std::collections::BTreeSet::from(["app.helper", "app.main"])
    );
    assert_eq!(diff["added"].as_array().unwrap().len(), 0);
    assert_eq!(diff["removed"].as_array().unwrap().len(), 0);
    assert_eq!(diff["unchanged_count"], 0);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_new_callee_is_reported_as_added_and_the_caller_as_changed() {
    let path = write_temp(V1_BASE);
    let options = forward_options(1);
    let (base_document, base_revision) = base_snapshot(V1_BASE, "app.main", &options);
    std::fs::write(&path, V4_ADDED_CALLEE).unwrap();

    let request = ContextDeltaRequest {
        symbol: "app.main",
        options: &options,
        claimed_base_revision: &base_revision,
        base_document: &base_document,
    };
    let delta = compute_context_delta(&path, &request).expect("delta computes");
    let value: Value = serde_json::from_str(&delta).expect("valid JSON");
    let diff = &value["diff"];
    let added = diff["added"].as_array().unwrap();
    assert_eq!(added.len(), 1);
    assert_eq!(added[0]["id"], "app.extra");
    let changed = diff["changed"].as_array().unwrap();
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0]["id"], "app.main");
    assert_eq!(diff["removed"].as_array().unwrap().len(), 0);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_dropped_callee_is_reported_as_removed_and_the_caller_as_changed() {
    let path = write_temp(V1_BASE);
    let options = forward_options(1);
    let (base_document, base_revision) = base_snapshot(V1_BASE, "app.main", &options);
    std::fs::write(&path, V5_REMOVED_CALLEE).unwrap();

    let request = ContextDeltaRequest {
        symbol: "app.main",
        options: &options,
        claimed_base_revision: &base_revision,
        base_document: &base_document,
    };
    let delta = compute_context_delta(&path, &request).expect("delta computes");
    let value: Value = serde_json::from_str(&delta).expect("valid JSON");
    let diff = &value["diff"];
    let removed = diff["removed"].as_array().unwrap();
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0], "app.helper");
    let changed = diff["changed"].as_array().unwrap();
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0]["id"], "app.main");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn an_unknown_symbol_is_a_hard_error_not_an_empty_delta() {
    let path = write_temp(V1_BASE);
    let options = forward_options(1);
    let (base_document, base_revision) = base_snapshot(V1_BASE, "app.main", &options);

    let request = ContextDeltaRequest {
        symbol: "app.does_not_exist",
        options: &options,
        claimed_base_revision: &base_revision,
        base_document: &base_document,
    };
    let outcome = compute_context_delta(&path, &request);
    assert!(outcome.is_err());
    let _ = std::fs::remove_file(&path);
}

// ---------------------------------------------------------------------
// Context delta: resynchronization on a stale/unknown/inconsistent base
// (unit-tested directly against `build_delta` so each failure mode is
// exercised in isolation, without needing a distinct fixture module per
// case).
// ---------------------------------------------------------------------

fn sample_current_and_base() -> (String, ParsedFixture) {
    let options = forward_options(1);
    let (document, revision) = base_snapshot(V1_BASE, "app.main", &options);
    let parsed: Value = serde_json::from_str(&document).expect("valid JSON");
    (
        document,
        ParsedFixture {
            revision,
            value: parsed,
        },
    )
}

struct ParsedFixture {
    revision: String,
    value: Value,
}

fn outcome_of(result: &str) -> String {
    let value: Value = serde_json::from_str(result).expect("valid JSON");
    value["outcome"].as_str().unwrap().to_owned()
}

fn reason_of(result: &str) -> String {
    let value: Value = serde_json::from_str(result).expect("valid JSON");
    value["reason"].as_str().unwrap().to_owned()
}

#[test]
fn a_malformed_base_document_forces_resynchronization() {
    let (current, _fixture) = sample_current_and_base();
    let result = build_delta(&current, "irrelevant", "not json at all", 64 * 1024)
        .expect("resync is a success outcome, not an error");
    assert_eq!(outcome_of(&result), "resync_required");
    assert_eq!(reason_of(&result), "malformed_base");
}

#[test]
fn a_schema_mismatched_base_forces_resynchronization() {
    let (current, fixture) = sample_current_and_base();
    let fake = json!({
        "schema": "semaprax.agent-context.v1",
        "revision": fixture.revision,
        "module": fixture.value["module"],
        "root": fixture.value["root"],
        "query": fixture.value["query"],
        "truncation": {"truncated": false},
        "facts": [],
    })
    .to_string();
    let result = build_delta(&current, &fixture.revision, &fake, 64 * 1024).expect("resync");
    assert_eq!(outcome_of(&result), "resync_required");
    assert_eq!(reason_of(&result), "schema_mismatch");
}

#[test]
fn a_base_that_lies_about_its_own_revision_forces_resynchronization() {
    let (current, fixture) = sample_current_and_base();
    let base_document = fixture.value.to_string();
    let claimed = format!("{}-not-the-real-revision", fixture.revision);
    let result = build_delta(&current, &claimed, &base_document, 64 * 1024).expect("resync");
    assert_eq!(outcome_of(&result), "resync_required");
    assert_eq!(reason_of(&result), "base_revision_mismatch");
}

#[test]
fn a_base_for_a_different_target_forces_resynchronization() {
    let (current, fixture) = sample_current_and_base();
    let mut retargeted = fixture.value.clone();
    retargeted["root"] = json!("app.some_other_root");
    let base_document = retargeted.to_string();
    let result =
        build_delta(&current, &fixture.revision, &base_document, 64 * 1024).expect("resync");
    assert_eq!(outcome_of(&result), "resync_required");
    assert_eq!(reason_of(&result), "target_mismatch");
}

#[test]
fn a_base_built_from_a_different_query_shape_forces_resynchronization() {
    let options_depth_1 = forward_options(1);
    let options_depth_2 = forward_options(2);
    let (base_document, base_revision) = base_snapshot(V1_BASE, "app.main", &options_depth_2);
    let program = crate::parse(V1_BASE, "fixture.spx").expect("parses");
    let current_document = graph::agent_context_v2_json(&program, "app.main", &options_depth_1)
        .expect("resolves")
        .expect("root exists");
    let result =
        build_delta(&current_document, &base_revision, &base_document, 64 * 1024).expect("resync");
    assert_eq!(outcome_of(&result), "resync_required");
    assert_eq!(reason_of(&result), "query_mismatch");
}

#[test]
fn a_truncated_base_forces_resynchronization_rather_than_an_incomplete_diff() {
    let (current, fixture) = sample_current_and_base();
    let mut truncated = fixture.value.clone();
    truncated["truncation"]["truncated"] = json!(true);
    let base_document = truncated.to_string();
    let result =
        build_delta(&current, &fixture.revision, &base_document, 64 * 1024).expect("resync");
    assert_eq!(outcome_of(&result), "resync_required");
    assert_eq!(reason_of(&result), "base_truncated");
}

#[test]
fn an_oversized_delta_forces_resynchronization_instead_of_a_partial_diff() {
    let options = forward_options(1);
    let (base_document, base_revision) = base_snapshot(V1_BASE, "app.main", &options);
    let program = crate::parse(V4_ADDED_CALLEE, "fixture.spx").expect("parses");
    let current_document = graph::agent_context_v2_json(&program, "app.main", &options)
        .expect("resolves")
        .expect("root exists");
    // A one-byte budget cannot possibly hold a real delta.
    let result = build_delta(&current_document, &base_revision, &base_document, 1).expect("resync");
    assert_eq!(outcome_of(&result), "resync_required");
    assert_eq!(reason_of(&result), "delta_exceeds_max_bytes");
}

#[test]
fn equal_revisions_with_disagreeing_facts_is_a_hard_invariant_violation() {
    let (current, fixture) = sample_current_and_base();
    let mut forged = fixture.value.clone();
    forged["facts"] =
        json!([{"id": "app.helper", "kind": "function", "name": "forged", "calls": []}]);
    let base_document = forged.to_string();
    let outcome = build_delta(&current, &fixture.revision, &base_document, 64 * 1024);
    assert!(
        outcome.is_err(),
        "a base claiming the current revision but different facts must never be accepted silently"
    );
}
