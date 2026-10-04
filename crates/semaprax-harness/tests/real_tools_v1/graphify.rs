//! Provisioned real-tool evidence for the Graphify context provider (HP-07).
//! Requires SEMAPRAX_COMPILER, HARNESS_GRAPHIFY, HARNESS_PYTHON; the provider
//! switch test also needs HARNESS_GRAFT and HARNESS_NODE.

use crate::graft::*;
use crate::support::required_tool;

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

/// Re-runs the host scenarios against the newer qualified Graphify (HARNESS_GRAPHIFY_NEW, a
/// graphifyy 0.9.75 venv executable) by overriding HARNESS_GRAPHIFY in a child of this binary.
#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER HARNESS_GRAPHIFY_NEW HARNESS_PYTHON"]
fn graphify_new_version_runs_host_scenarios() {
    let new = std::env::var("HARNESS_GRAPHIFY_NEW").expect("HARNESS_GRAPHIFY_NEW");
    let exe = std::env::current_exe().unwrap();
    for name in [
        "graphify::graphify_facts_and_spans",
        "graphify::graphify_rename_stale_index_and_warm_reuse",
        "graphify::graphify_worktree_switch",
        "graphify::graphify_absent_provider_fallback_and_required",
        "graphify::graphify_planted_secrets_not_inherited",
        "graphify::graphify_offline_inner",
        "graphify::graphify_reference_labels_and_no_absence_claim",
    ] {
        let out = std::process::Command::new(&exe)
            .args([name, "--exact", "--ignored", "--nocapture"])
            .env("HARNESS_GRAPHIFY", &new)
            .output()
            .expect("spawn");
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            out.status.success() && text.contains("1 passed"),
            "{name} failed on 0.9.75:\n{text}"
        );
    }
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER HARNESS_GRAPHIFY (graphifyy 0.9.75) HARNESS_PYTHON"]
fn graphify_adopts_a_compatible_user_graph_read_only_through_harness_context() {
    scenario_adopt_user_index(Tool::Graphify, "read-only");
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER HARNESS_GRAPHIFY (graphifyy 0.9.75) HARNESS_PYTHON"]
fn graphify_adopts_a_compatible_user_graph_as_a_copied_snapshot_through_harness_context() {
    scenario_adopt_user_index(Tool::Graphify, "copied-snapshot");
}

/// An index written by another graphify version (a changed parser) is never current evidence:
/// the adapter falls back to its owned cache and the user's files stay byte-identical.
#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER HARNESS_GRAPHIFY (0.9.75) HARNESS_GRAPHIFY_OLD (0.9.25) HARNESS_PYTHON"]
fn graphify_refuses_a_user_graph_built_by_another_version_and_falls_back_to_an_owned_cache() {
    let rig = Rig::new(Tool::Graphify, "adopt-old");
    rig.adopt_trust(Tool::Graphify);
    rig.write(
        "semaprax.harness.toml",
        &format!(
            "schema = \"semaprax.harness-config.v1\"\n\n[capability.\"context.repository\"]\nmode = \"required\"\nprovider = \"{}\"\n\n[capability.\"context.repository\".config]\nadopt_index = \"read-only\"\n",
            Tool::Graphify.id()
        ),
    );
    let o = rig.sh(&["resolve", rig.project.to_str().unwrap()]);
    assert_eq!(o.code, 0, "{}{}", o.stdout, o.stderr);
    let dirname =
        build_user_index_with(&rig, Tool::Graphify, required_tool("HARNESS_GRAPHIFY_OLD"));
    let before = tree_digest(&rig.project.join(dirname));
    let d = rig.ctx(&["renderTotal", "--max-bytes", "16000"]);
    assert!(text_has(&d, "renderTotal"), "{d}");
    assert!(rig.index_stamp().0 > 0, "an owned cache served the answer");
    assert_eq!(tree_digest(&rig.project.join(dirname)), before);
}
