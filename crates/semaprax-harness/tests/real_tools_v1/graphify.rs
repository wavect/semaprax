//! Provisioned real-tool evidence for the Graphify context provider (HP-07).
//! Requires SEMAPRAX_COMPILER, HARNESS_GRAPHIFY, HARNESS_PYTHON; the provider
//! switch test also needs HARNESS_GRAFT and HARNESS_NODE.

use crate::graft::*;

const NEEDS: &str = "provisioned: needs SEMAPRAX_COMPILER HARNESS_GRAPHIFY HARNESS_PYTHON";

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER HARNESS_GRAPHIFY HARNESS_PYTHON"]
fn graphify_facts_and_spans() {
    let _ = NEEDS;
    scenario_facts_and_spans(Tool::Graphify);
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER HARNESS_GRAPHIFY HARNESS_PYTHON"]
fn graphify_rename_stale_index_and_warm_reuse() {
    scenario_rename_stale_warm(Tool::Graphify);
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER HARNESS_GRAPHIFY HARNESS_PYTHON"]
fn graphify_worktree_switch() {
    scenario_worktree_switch(Tool::Graphify);
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER HARNESS_GRAPHIFY HARNESS_PYTHON"]
fn graphify_absent_provider_fallback_and_required() {
    scenario_absent(Tool::Graphify);
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER HARNESS_GRAPHIFY HARNESS_PYTHON"]
fn graphify_planted_secrets_not_inherited() {
    scenario_planted_secrets(Tool::Graphify);
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER HARNESS_GRAPHIFY HARNESS_PYTHON"]
fn graphify_offline_inner() {
    scenario_offline_inner(Tool::Graphify);
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER HARNESS_GRAPHIFY HARNESS_PYTHON, macOS sandbox-exec"]
fn graphify_network_denied_cold_and_warm() {
    run_offline("graphify::graphify_offline_inner");
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER HARNESS_GRAPHIFY HARNESS_PYTHON HARNESS_GRAFT HARNESS_NODE"]
fn project_switches_graft_to_graphify_by_config_only() {
    scenario_switch_provider();
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER HARNESS_GRAPHIFY HARNESS_PYTHON"]
fn graphify_reference_labels_and_no_absence_claim() {
    scenario_references_labels(Tool::Graphify);
}
