//! Exact receiver stages through the original caller-specific source profile.
use super::declared_history_inputs::{compiled, digest, expected_file, records, restore, slot};
use std::path::Path;

pub(crate) fn fixture_stage(
    caller: &'static str,
    source: &str,
    supplied: &str,
    original: &str,
    edits: &[serde_json::Value],
) -> String {
    let mut checkpoint = original.to_owned();
    for edit in edits {
        let before = edit["before"].as_str().unwrap();
        assert!(!before.is_empty());
        assert_eq!(checkpoint.matches(before).count(), 1);
        checkpoint = checkpoint.replacen(before, edit["after"].as_str().unwrap(), 1);
    }
    let (part, index) = slot(source, &digest(&checkpoint)).expect("closed fixture checkpoint");
    let row = &records(part)[index];
    assert!(
        row["fixture_originals"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["sha256"] == digest(original)),
        "closed complete fixture predecessor"
    );
    if supplied != compiled(source)
        && supplied != original
        && supplied != canonical_source(caller, source)
    {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(source);
        let expected = digest(supplied);
        let authenticated = expected_file(&path, supplied.len(), &expected)
            .or_else(|| {
                crate::source_reflows::legacy_ci_expected_file(&path, supplied.len(), &expected)
            })
            .unwrap_or_else(|| {
                panic!(
                    "foreign fixture input: {source} bytes={} sha={expected}",
                    supplied.len()
                )
            });
        assert_eq!(authenticated, supplied);
    }
    let restored = restore(source, row);
    assert_eq!(restored, checkpoint);
    restored
}

#[test]
fn fixed_receiver_checkpoints_keep_all_original_edits_and_reject_foreign_inputs() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for directory in [
        "referenced-edge-ownership-proof",
        "exit-prefix-reference-proof",
        "status-fact-borrow-reuse",
    ] {
        let directory = root.join("docs/evidence").join(directory);
        let rows: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(directory.join("exact-source-inverses.json")).unwrap(),
        )
        .unwrap();
        for row in rows.as_array().unwrap() {
            let source = row["source"].as_str().unwrap();
            let original =
                std::fs::read_to_string(directory.join(row["archive"].as_str().unwrap())).unwrap();
            assert_eq!(digest(&original), row["sha256"]);
            let edits = row["edits"].as_array().unwrap();
            let checkpoint = fixture_stage(file!(), source, compiled(source), &original, edits);
            let mut restored = checkpoint.clone();
            for edit in edits.iter().rev() {
                let after = edit["after"].as_str().unwrap();
                assert_eq!(restored.matches(after).count(), 1);
                restored = restored.replacen(after, edit["before"].as_str().unwrap(), 1);
            }
            assert_eq!(restored, original);
            assert!(std::panic::catch_unwind(|| fixture_stage(
                file!(),
                source,
                "foreign",
                &original,
                edits
            ))
            .is_err());
            let mut forged = edits.clone();
            forged[0]["after"] =
                serde_json::Value::String(format!("{}!", forged[0]["after"].as_str().unwrap()));
            assert!(std::panic::catch_unwind(|| fixture_stage(
                file!(),
                source,
                compiled(source),
                &original,
                &forged
            ))
            .is_err());
        }
    }
}

fn canonical_source(caller: &'static str, source: &str) -> &'static str {
    match source {
        "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs"), compiled(source)),
        "src/byte_ops.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/byte_ops.rs"), compiled(source)),
        "src/cleanup_plan/renewal_profile.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/renewal_profile.rs"), compiled(source)),
        "src/cleanup_plan/renewal_profile/funded.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/renewal_profile/funded.rs"), compiled(source)),
        "src/cleanup_plan/replay.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay.rs"), compiled(source)),
        "src/cleanup_plan/replay/branch_clone.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/branch_clone.rs"), compiled(source)),
        "src/cleanup_plan/replay/branch_clone/tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/branch_clone/tests.rs"), compiled(source)),
        "src/cleanup_plan/replay/continuation_storage.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/continuation_storage.rs"), compiled(source)),
        "src/cleanup_plan/replay/continuation_storage/tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/continuation_storage/tests.rs"), compiled(source)),
        "src/cleanup_plan/replay/expression_facts/arena.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/arena.rs"), compiled(source)),
        "src/cleanup_plan/replay/expression_facts/arena/cursor_tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/arena/cursor_tests.rs"), compiled(source)),
        "src/cleanup_plan/replay/expression_facts/arena/prefix.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/arena/prefix.rs"), compiled(source)),
        "src/cleanup_plan/replay/expression_facts/arena/prefix/tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/arena/prefix/tests.rs"), compiled(source)),
        "src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix.rs"), compiled(source)),
        "src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix/tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix/tests.rs"), compiled(source)),
        "src/cleanup_plan/replay/expression_facts/receiving/helpers.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/helpers.rs"), compiled(source)),
        "src/cleanup_plan/replay/expression_facts/receiving/helpers/matching.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/helpers/matching.rs"), compiled(source)),
        "src/cleanup_plan/replay/expression_facts/receiving/helpers/matching/tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/helpers/matching/tests.rs"), compiled(source)),
        "src/cleanup_plan/replay/expression_facts/receiving/helpers/tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/helpers/tests.rs"), compiled(source)),
        "src/cleanup_plan/replay/expression_facts/receiving/remaining.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/remaining.rs"), compiled(source)),
        "src/cleanup_plan/replay/expression_facts/receiving/retained.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained.rs"), compiled(source)),
        "src/cleanup_plan/replay/expression_facts/receiving/retained/exits.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained/exits.rs"), compiled(source)),
        "src/cleanup_plan/replay/expression_facts/receiving/retained/tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained/tests.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/borrowed_canonical/tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/borrowed_canonical/tests.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/oracle_hook.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/oracle_hook.rs"), compiled(source)),
        "src/cleanup_plan/replay/live_oracle/source_custody/b.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/live_oracle/source_custody/b.rs"), compiled(source)),
        "src/cleanup_plan/replay/skeleton_work.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/skeleton_work.rs"), compiled(source)),
        "src/hir.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir.rs"), compiled(source)),
        "src/hir/expr_nodes.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/expr_nodes.rs"), compiled(source)),
        "src/hir/function_value.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/function_value.rs"), compiled(source)),
        "src/hir/iterator_loop.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/iterator_loop.rs"), compiled(source)),
        "src/hir/iterator_loop/renewal.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/iterator_loop/renewal.rs"), compiled(source)),
        "src/hir/lexical_exit.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/lexical_exit.rs"), compiled(source)),
        "src/hir/lexical_exit/owner_funded.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/lexical_exit/owner_funded.rs"), compiled(source)),
        "src/hir/validation.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation.rs"), compiled(source)),
        "src/literal_format.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/literal_format.rs"), compiled(source)),
        "src/literal_format/funded.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/literal_format/funded.rs"), compiled(source)),
        "src/loop_calls/byte_writer.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/loop_calls/byte_writer.rs"), compiled(source)),
        "src/loop_calls/byte_writer/resolved.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/loop_calls/byte_writer/resolved.rs"), compiled(source)),
        "src/loop_calls/byte_writer/source.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/loop_calls/byte_writer/source.rs"), compiled(source)),
        "src/source_reflows/incoming67107_semantic_receivers.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_reflows/incoming67107_semantic_receivers.rs"), compiled(source)),
        "src/source_reflows/semantic_receivers.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_reflows/semantic_receivers.rs"), compiled(source)),
        "src/source_verify/declaration.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_verify/declaration.rs"), compiled(source)),
        "src/string_ops/conditions.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/string_ops/conditions.rs"), compiled(source)),
        "src/string_ops/conditions/funded.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/string_ops/conditions/funded.rs"), compiled(source)),
        "src/workspace_graph/expected_projection/import_stub/core_proofs/tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/import_stub/core_proofs/tests.rs"), compiled(source)),
        "src/workspace_graph/identity_resolution.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/identity_resolution.rs"), compiled(source)),
        "tests/hir_module_boundaries.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/tests/hir_module_boundaries.rs"), compiled(source)),
        "tests/workspace/semantic_graph/source_inventory.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/tests/workspace/semantic_graph/source_inventory.rs"), compiled(source)),
        _ => panic!("unknown complete source profile"),
    }
}
