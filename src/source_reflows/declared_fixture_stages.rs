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
        "docs/evidence/compact-child-cursor-v1/sources.json" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/compact-child-cursor-v1/sources.json"), compiled(source)),
        "src/ast/program_source/decl_slices/alias_tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/ast/program_source/decl_slices/alias_tests.rs"), compiled(source)),
        "src/ast/program_source/decl_slices/before_alias.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/ast/program_source/decl_slices/before_alias.rs"), compiled(source)),
        "src/ast/program_source/function_alias/presentation.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/ast/program_source/function_alias/presentation.rs"), compiled(source)),
        "src/bounded_output.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/bounded_output.rs"), compiled(source)),
        "src/box_ops/wrappers.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/box_ops/wrappers.rs"), compiled(source)),
        "src/cleanup.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup.rs"), compiled(source)),
        "src/cleanup/inventory_builder.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup/inventory_builder.rs"), compiled(source)),
        "src/cleanup_plan/build.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/build.rs"), compiled(source)),
        "src/cleanup_plan/renewal_profile/child_refusal.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/renewal_profile/child_refusal.rs"), compiled(source)),
        "src/cleanup_plan/renewal_profile/ordinary/direct_presence_tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/renewal_profile/ordinary/direct_presence_tests.rs"), compiled(source)),
        "src/cleanup_plan/renewal_profile/selected.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/renewal_profile/selected.rs"), compiled(source)),
        "src/cleanup_plan/renewal_profile/selected_inline.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/renewal_profile/selected_inline.rs"), compiled(source)),
        "src/cleanup_plan/renewal_profile/selected_inline/append.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/renewal_profile/selected_inline/append.rs"), compiled(source)),
        "src/cleanup_plan/replay/allocation/oracle.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/allocation/oracle.rs"), compiled(source)),
        "src/cleanup_plan/replay/branch_clone/funded_tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/branch_clone/funded_tests.rs"), compiled(source)),
        "src/cleanup_plan/replay/branch_clone/production_capacity/tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/branch_clone/production_capacity/tests.rs"), compiled(source)),
        "src/cleanup_plan/replay/expression_facts.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts.rs"), compiled(source)),
        "src/cleanup_plan/replay/expression_facts/arena/retained/tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/arena/retained/tests.rs"), compiled(source)),
        "src/cleanup_plan/replay/expression_facts/errors.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/errors.rs"), compiled(source)),
        "src/cleanup_plan/replay/expression_facts/ordinary_before.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/ordinary_before.rs"), compiled(source)),
        "src/cleanup_plan/replay/expression_facts/receiving.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving.rs"), compiled(source)),
        "src/cleanup_plan/replay/expression_facts/receiving/directory.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/directory.rs"), compiled(source)),
        "src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/counter_before.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/counter_before.rs"), compiled(source)),
        "src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_tests.rs"), compiled(source)),
        "src/cleanup_plan/replay/expression_facts/receiving/return_operands.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/return_operands.rs"), compiled(source)),
        "src/cleanup_plan/replay/expression_facts/receiving/return_operands/exits.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/return_operands/exits.rs"), compiled(source)),
        "src/cleanup_plan/replay/expression_facts/receiving/return_operands/tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/return_operands/tests.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/before_closure_invocation.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/before_closure_invocation.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/canonical_shared.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/canonical_shared/arena_tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/arena_tests.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/canonical_shared/arena_tests_tail.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/arena_tests_tail.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/canonical_shared/before_alias.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/before_alias.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/canonical_shared/before_arena.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/before_arena.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/canonical_shared/before_closure_receiver.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/before_closure_receiver.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/canonical_shared/before_empty.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/before_empty.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/canonical_shared/before_empty_signature.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/before_empty_signature.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/canonical_shared/before_held.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/before_held.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/canonical_shared/before_inline_weighted.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/before_inline_weighted.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/canonical_shared/before_lexical_successors_methods.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/before_lexical_successors_methods.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/canonical_shared/before_ordered.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/before_ordered.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/canonical_shared/before_paid_many.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/before_paid_many.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/canonical_shared/before_receiving_layers.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/before_receiving_layers.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/canonical_shared/before_scratch.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/before_scratch.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/canonical_shared/before_selected_labels_methods.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/before_selected_labels_methods.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/canonical_shared/before_selected_route_methods.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/before_selected_route_methods.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/canonical_shared/before_targets.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/before_targets.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/canonical_shared/before_terminal_leaf.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/before_terminal_leaf.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/canonical_shared/before_unit.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/before_unit.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/canonical_shared/empty_signature_tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/empty_signature_tests.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/canonical_shared/incoming_flat_targets.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/incoming_flat_targets.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/canonical_shared/lexical_successors/frame_loop.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/lexical_successors/frame_loop.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/canonical_shared/lexical_successors/frame_loop/before_arity_dispatch.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/lexical_successors/frame_loop/before_arity_dispatch.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/canonical_shared/lexical_successors/tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/lexical_successors/tests.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/canonical_shared/main_unit_weight.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/main_unit_weight.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/canonical_shared/memo/retained/held_tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/memo/retained/held_tests.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/canonical_shared/ordered_weights/amortized_source.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/ordered_weights/amortized_source.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/canonical_shared/ordered_weights/source.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/ordered_weights/source.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/canonical_shared/paid_many/tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/paid_many/tests.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/canonical_shared/paid_route_tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/paid_route_tests.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/canonical_shared/receiving_layer_tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/receiving_layer_tests.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/canonical_shared/successor_targets.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/successor_targets.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/canonical_shared/terminal_leaf.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/terminal_leaf.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/canonical_shared/tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/tests.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/compact_nodes/before/canonical.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/compact_nodes/before/canonical.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/compact_nodes/tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/compact_nodes/tests.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/continuation_receiver/tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/continuation_receiver/tests.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/instrumentation.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/instrumentation.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/moderate_tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/moderate_tests.rs"), compiled(source)),
        "src/cleanup_plan/replay/factored/oracle_order/tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/oracle_order/tests.rs"), compiled(source)),
        "src/cleanup_plan/replay/flat_records/tests/receiving_status_scalar.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/flat_records/tests/receiving_status_scalar.rs"), compiled(source)),
        "src/cleanup_plan/replay/lexical_replay.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/lexical_replay.rs"), compiled(source)),
        "src/cleanup_plan/replay/lexical_replay/before_operands.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/lexical_replay/before_operands.rs"), compiled(source)),
        "src/cleanup_plan/replay/lexical_replay/operand_tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/lexical_replay/operand_tests.rs"), compiled(source)),
        "src/cleanup_plan/replay/lexical_replay/operand_tests/source.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/lexical_replay/operand_tests/source.rs"), compiled(source)),
        "src/cleanup_plan/replay/lexical_replay/ordinary.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/lexical_replay/ordinary.rs"), compiled(source)),
        "src/cleanup_plan/replay/live_oracle/source_custody/presence.json" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/live_oracle/source_custody/presence.json"), compiled(source)),
        "src/cleanup_plan/replay/ordinary_plan_history/owned_tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/ordinary_plan_history/owned_tests.rs"), compiled(source)),
        "src/cleanup_plan/replay/path_array/sequence_minimum/before.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_array/sequence_minimum/before.rs"), compiled(source)),
        "src/cleanup_plan/replay/path_array/sequence_minimum/optimized_tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_array/sequence_minimum/optimized_tests.rs"), compiled(source)),
        "src/cleanup_plan/replay/path_join/boxed_states/before.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/boxed_states/before.rs"), compiled(source)),
        "src/cleanup_plan/replay/path_join/tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/tests.rs"), compiled(source)),
        "src/cleanup_plan/replay/physical_budget/authentication/tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/physical_budget/authentication/tests.rs"), compiled(source)),
        "src/cleanup_plan/replay/physical_clone.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/physical_clone.rs"), compiled(source)),
        "src/cleanup_plan/replay/scalar_replay/records.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/scalar_replay/records.rs"), compiled(source)),
        "src/cleanup_plan/replay/selected_context.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_context.rs"), compiled(source)),
        "src/cleanup_plan/replay/selected_context/child.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_context/child.rs"), compiled(source)),
        "src/cleanup_plan/replay/selected_context/child/tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_context/child/tests.rs"), compiled(source)),
        "src/cleanup_plan/replay/selected_context/child/tests/source.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_context/child/tests/source.rs"), compiled(source)),
        "src/cleanup_plan/replay/selected_context/paid.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_context/paid.rs"), compiled(source)),
        "src/cleanup_plan/replay/selected_context/paid_before_children.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_context/paid_before_children.rs"), compiled(source)),
        "src/cleanup_plan/replay/selected_entry.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_entry.rs"), compiled(source)),
        "src/cleanup_plan/replay/selected_entry/paid.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_entry/paid.rs"), compiled(source)),
        "src/cleanup_plan/replay/selected_entry/tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_entry/tests.rs"), compiled(source)),
        "src/cleanup_plan/replay/selected_entry/tests/paid.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_entry/tests/paid.rs"), compiled(source)),
        "src/cleanup_plan/replay/selected_entry/tests/session_errors.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_entry/tests/session_errors.rs"), compiled(source)),
        "src/cleanup_plan/replay/selected_entry/tests/session_errors/source.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_entry/tests/session_errors/source.rs"), compiled(source)),
        "src/cleanup_plan/replay/selected_entry/tests/structural_errors.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_entry/tests/structural_errors.rs"), compiled(source)),
        "src/cleanup_plan/replay/selected_owner.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_owner.rs"), compiled(source)),
        "src/cleanup_plan/replay/selected_owner/paid.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_owner/paid.rs"), compiled(source)),
        "src/cleanup_plan/replay/selected_owner/paid_before_children.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_owner/paid_before_children.rs"), compiled(source)),
        "src/cleanup_plan/replay/selected_profile.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_profile.rs"), compiled(source)),
        "src/cleanup_plan/replay/selected_profile/paid.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_profile/paid.rs"), compiled(source)),
        "src/cleanup_plan/replay/selected_profile/paid_before_children.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_profile/paid_before_children.rs"), compiled(source)),
        "src/cleanup_plan/replay/selected_profile/paid_children.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_profile/paid_children.rs"), compiled(source)),
        "src/cleanup_plan/replay/selected_profile/paid_children/tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_profile/paid_children/tests.rs"), compiled(source)),
        "src/cleanup_plan/replay/semantic_refusal.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/semantic_refusal.rs"), compiled(source)),
        "src/cleanup_plan/replay/semantic_refusal/tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/semantic_refusal/tests.rs"), compiled(source)),
        "src/cleanup_plan/replay/sequence_prefix/main24/before.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/sequence_prefix/main24/before.rs"), compiled(source)),
        "src/cleanup_plan/replay/sequence_prefix/main24/tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/sequence_prefix/main24/tests.rs"), compiled(source)),
        "src/cleanup_plan/replay/sequence_prefix/main24/wrapper_context.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/sequence_prefix/main24/wrapper_context.rs"), compiled(source)),
        "src/cleanup_plan/replay/sequence_prefix/main8a/wrapper_context.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/sequence_prefix/main8a/wrapper_context.rs"), compiled(source)),
        "src/cleanup_plan/replay/sequence_prefix/main_c11/wrapper_context.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/sequence_prefix/main_c11/wrapper_context.rs"), compiled(source)),
        "src/cleanup_plan/replay/sequence_vector/before_funded.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/sequence_vector/before_funded.rs"), compiled(source)),
        "src/cleanup_plan/replay/sequence_vector/funded_tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/sequence_vector/funded_tests.rs"), compiled(source)),
        "src/cleanup_plan/replay/sequence_vector/observation_parity.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/sequence_vector/observation_parity.rs"), compiled(source)),
        "src/cleanup_plan/replay/skeleton_work/merge_keys_tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/skeleton_work/merge_keys_tests.rs"), compiled(source)),
        "src/cleanup_plan/replay/structural_control/calls/custody_tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/structural_control/calls/custody_tests.rs"), compiled(source)),
        "src/format.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/format.rs"), compiled(source)),
        "src/hir/declaration_index/identity_payer.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/declaration_index/identity_payer.rs"), compiled(source)),
        "src/hir/declaration_index/nominal.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/declaration_index/nominal.rs"), compiled(source)),
        "src/hir/declaration_index/nominal/legacy.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/declaration_index/nominal/legacy.rs"), compiled(source)),
        "src/hir/declaration_index/nominal_original.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/declaration_index/nominal_original.rs"), compiled(source)),
        "src/hir/identity_resolution/nominal.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/identity_resolution/nominal.rs"), compiled(source)),
        "src/hir/identity_resolution/nominal_original.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/identity_resolution/nominal_original.rs"), compiled(source)),
        "src/hir/identity_resolution/nominal_tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/identity_resolution/nominal_tests.rs"), compiled(source)),
        "src/hir/identity_resolution/nominal_tests/legacy.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/identity_resolution/nominal_tests/legacy.rs"), compiled(source)),
        "src/hir/inspection.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/inspection.rs"), compiled(source)),
        "src/hir/inspection/paid_nul.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/inspection/paid_nul.rs"), compiled(source)),
        "src/hir/lexical_exit/latch_inline.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/lexical_exit/latch_inline.rs"), compiled(source)),
        "src/hir/lexical_exit/selected_child.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/lexical_exit/selected_child.rs"), compiled(source)),
        "src/hir/lexical_exit/walk.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/lexical_exit/walk.rs"), compiled(source)),
        "src/hir/lexical_exit/walk/tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/lexical_exit/walk/tests.rs"), compiled(source)),
        "src/hir/nominal_payload.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/nominal_payload.rs"), compiled(source)),
        "src/hir/nominal_payload/elements.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/nominal_payload/elements.rs"), compiled(source)),
        "src/hir/nominal_payload/legacy.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/nominal_payload/legacy.rs"), compiled(source)),
        "src/hir/nominal_payload/legacy/clone.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/nominal_payload/legacy/clone.rs"), compiled(source)),
        "src/hir/nominal_payload/legacy/source_tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/nominal_payload/legacy/source_tests.rs"), compiled(source)),
        "src/hir/nominal_payload/legacy/tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/nominal_payload/legacy/tests.rs"), compiled(source)),
        "src/hir/nominal_payload/map.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/nominal_payload/map.rs"), compiled(source)),
        "src/hir/resolve_program_methods.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/resolve_program_methods.rs"), compiled(source)),
        "src/hir/resolve_yield.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/resolve_yield.rs"), compiled(source)),
        "src/hir/static_text.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/static_text.rs"), compiled(source)),
        "src/hir/validation/canonical_loans/ownership_tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/canonical_loans/ownership_tests.rs"), compiled(source)),
        "src/hir/validation/constructor_seed/original.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/constructor_seed/original.rs"), compiled(source)),
        "src/hir/validation/function_lookup/paid_custody.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/function_lookup/paid_custody.rs"), compiled(source)),
        "src/hir/validation/function_scope/lifecycle/completed.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/function_scope/lifecycle/completed.rs"), compiled(source)),
        "src/hir/validation/generic_template.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/generic_template.rs"), compiled(source)),
        "src/loan_plan.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/loan_plan.rs"), compiled(source)),
        "src/loan_plan/adjacency.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/loan_plan/adjacency.rs"), compiled(source)),
        "src/loan_plan/cfg_directory.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/loan_plan/cfg_directory.rs"), compiled(source)),
        "src/loan_plan/funded_flat.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/loan_plan/funded_flat.rs"), compiled(source)),
        "src/loan_plan/funded_flat/tests_before.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/loan_plan/funded_flat/tests_before.rs"), compiled(source)),
        "src/loan_plan/funded_flat/work_boundary.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/loan_plan/funded_flat/work_boundary.rs"), compiled(source)),
        "src/loan_plan/paid_candidate.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/loan_plan/paid_candidate.rs"), compiled(source)),
        "src/loan_plan/paid_cfg.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/loan_plan/paid_cfg.rs"), compiled(source)),
        "src/loan_plan/paid_cursor/tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/loan_plan/paid_cursor/tests.rs"), compiled(source)),
        "src/loan_plan/paid_plan.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/loan_plan/paid_plan.rs"), compiled(source)),
        "src/loan_plan/paid_plan_legacy.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/loan_plan/paid_plan_legacy.rs"), compiled(source)),
        "src/loan_plan/tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/loan_plan/tests.rs"), compiled(source)),
        "src/loop_calls/byte_writer/resolved/selected_inline.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/loop_calls/byte_writer/resolved/selected_inline.rs"), compiled(source)),
        "src/loop_calls/byte_writer/source_index.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/loop_calls/byte_writer/source_index.rs"), compiled(source)),
        "src/project/incremental.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/project/incremental.rs"), compiled(source)),
        "src/source_reflows/incoming090_semantic_receivers.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_reflows/incoming090_semantic_receivers.rs"), compiled(source)),
        "src/source_reflows/semantic_receivers_main44.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_reflows/semantic_receivers_main44.rs"), compiled(source)),
        "src/source_reflows/semantic_receivers_main7c6.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_reflows/semantic_receivers_main7c6.rs"), compiled(source)),
        "src/source_reflows/semantic_receivers_main8a.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_reflows/semantic_receivers_main8a.rs"), compiled(source)),
        "src/source_reflows/semantic_receivers_main_afba.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_reflows/semantic_receivers_main_afba.rs"), compiled(source)),
        "src/source_reflows/semantic_receivers_main_f1.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_reflows/semantic_receivers_main_f1.rs"), compiled(source)),
        "src/source_reflows/semantic_receivers_maina3.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_reflows/semantic_receivers_maina3.rs"), compiled(source)),
        "src/source_reflows/semantic_receivers_mainb80.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_reflows/semantic_receivers_mainb80.rs"), compiled(source)),
        "src/source_reflows_merge.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_reflows_merge.rs"), compiled(source)),
        "src/source_reflows_merge/main67107_inputs_tail.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_reflows_merge/main67107_inputs_tail.rs"), compiled(source)),
        "src/source_reflows_merge/main_e177_final_inputs.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_reflows_merge/main_e177_final_inputs.rs"), compiled(source)),
        "src/source_reflows_merge/main_e177_overlay_v2.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_reflows_merge/main_e177_overlay_v2.rs"), compiled(source)),
        "src/source_reflows_merge/runtime_json.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_reflows_merge/runtime_json.rs"), compiled(source)),
        "src/source_reflows_merge/runtime_json/tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_reflows_merge/runtime_json/tests.rs"), compiled(source)),
        "src/source_verify/declaration/functions/old.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_verify/declaration/functions/old.rs"), compiled(source)),
        "src/source_verify/declaration/functions/public_signature_before.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_verify/declaration/functions/public_signature_before.rs"), compiled(source)),
        "src/source_verify/funding.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_verify/funding.rs"), compiled(source)),
        "src/source_verify/generic_inference.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_verify/generic_inference.rs"), compiled(source)),
        "src/source_verify/hints.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_verify/hints.rs"), compiled(source)),
        "src/source_verify/iterative.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_verify/iterative.rs"), compiled(source)),
        "src/source_verify/iterative/enter/enter_before_components.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_verify/iterative/enter/enter_before_components.rs"), compiled(source)),
        "src/source_verify/iterative/enter/enter_before_extended.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_verify/iterative/enter/enter_before_extended.rs"), compiled(source)),
        "src/source_verify/recovery_legacy/enter.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_verify/recovery_legacy/enter.rs"), compiled(source)),
        "src/source_verify/vec_field.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_verify/vec_field.rs"), compiled(source)),
        "src/vec_ops/wrappers.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/vec_ops/wrappers.rs"), compiled(source)),
        "src/workspace_graph.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph.rs"), compiled(source)),
        "src/workspace_graph/constructor_failure/report/stages.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/constructor_failure/report/stages.rs"), compiled(source)),
        "src/workspace_graph/constructor_failure/report/tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/constructor_failure/report/tests.rs"), compiled(source)),
        "src/workspace_graph/constructor_failure/typed.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/constructor_failure/typed.rs"), compiled(source)),
        "src/workspace_graph/expected_projection/borrowed_imports/typed/alias/owned_growth.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/borrowed_imports/typed/alias/owned_growth.rs"), compiled(source)),
        "src/workspace_graph/expected_projection/borrowed_imports/typed/alias/rename_tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/borrowed_imports/typed/alias/rename_tests.rs"), compiled(source)),
        "src/workspace_graph/expected_projection/borrowed_imports/typed/alias/tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/borrowed_imports/typed/alias/tests.rs"), compiled(source)),
        "src/workspace_graph/expected_projection/borrowed_imports/typed_cost.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/borrowed_imports/typed_cost.rs"), compiled(source)),
        "src/workspace_graph/identity_resolution/base/cache_billing/main7f8_reader.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/identity_resolution/base/cache_billing/main7f8_reader.rs"), compiled(source)),
        "src/workspace_graph/identity_resolution/base/cache_billing/source_reference.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/identity_resolution/base/cache_billing/source_reference.rs"), compiled(source)),
        "src/workspace_graph/source_schema/nominal_context/main8a.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/source_schema/nominal_context/main8a.rs"), compiled(source)),
        "src/workspace_graph/validation/before_roots.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/validation/before_roots.rs"), compiled(source)),
        "tests/workspace/semantic_change.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/tests/workspace/semantic_change.rs"), compiled(source)),
        "tests/workspace/semantic_change/reporting.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/tests/workspace/semantic_change/reporting.rs"), compiled(source)),
        "tests/workspace/semantic_structural_change.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/tests/workspace/semantic_structural_change.rs"), compiled(source)),
        "tests/workspace/semantic_structural_change/reporting.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/tests/workspace/semantic_structural_change/reporting.rs"), compiled(source)),
        "docs/evidence/replay-clone-cursor-v1/sources.json" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/replay-clone-cursor-v1/sources.json"), compiled(source)),
        "src/cleanup_plan/replay/allocation/tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/allocation/tests.rs"), compiled(source)),
        "src/cleanup_plan/replay/allocation/tests/clone_cursor.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/allocation/tests/clone_cursor.rs"), compiled(source)),
        "src/cleanup_plan/replay/compact_stage/before.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/compact_stage/before.rs"), compiled(source)),
        "src/cleanup_plan/replay/compact_stage/lexical_before.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/compact_stage/lexical_before.rs"), compiled(source)),
        "src/cleanup_plan/replay/lexical_skeleton.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/lexical_skeleton.rs"), compiled(source)),
        "src/cleanup_plan/replay/physical_budget.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/physical_budget.rs"), compiled(source)),
        "src/cleanup_plan/replay/sequence_prefix/before.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/sequence_prefix/before.rs"), compiled(source)),
        "src/hir/validation/function_scope/lifecycle.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/function_scope/lifecycle.rs"), compiled(source)),
        "src/hir/validation/generic_template/call_graph/tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/generic_template/call_graph/tests.rs"), compiled(source)),
        "src/prelude.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/prelude.rs"), compiled(source)),
        "src/source_reflows.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_reflows.rs"), compiled(source)),
        "src/source_verify/constructor_tests.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_verify/constructor_tests.rs"), compiled(source)),
        "src/wasm/aggregate.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/wasm/aggregate.rs"), compiled(source)),
        "src/wasm/aggregate/semantic_work.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/wasm/aggregate/semantic_work.rs"), compiled(source)),
        "src/workspace_graph/expected_projection.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection.rs"), compiled(source)),
        "src/workspace_graph/expected_projection/held_projection.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/held_projection.rs"), compiled(source)),
        "tests/wasm/scalar_browser_ci_contract.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/tests/wasm/scalar_browser_ci_contract.rs"), compiled(source)),
        "tests/wasm/scalar_browser_ci_contract/historical_builder.rs" => crate::source_reflows::source(caller, concat!(env!("CARGO_MANIFEST_DIR"), "/tests/wasm/scalar_browser_ci_contract/historical_builder.rs"), compiled(source)),
        _ => panic!("unknown complete source profile"),
    }
}

// Preserve the complete original prior-source guard before acquiring the
// separate checkpoint declared by the immutable selected-child inverse table.
pub(crate) fn complete_child_input(
    source: &str,
    raw: &str,
    received: &str,
    row: &serde_json::Value,
) -> String {
    let mut checkpoint = row["original_bytes"].as_str().unwrap().to_owned();
    for edit in row["edits"].as_array().unwrap().iter().rev() {
        let before = edit["before"].as_str().unwrap();
        let after = edit["new"].as_str().unwrap();
        if before.is_empty() {
            assert!(checkpoint.is_empty());
            checkpoint = after.to_owned();
        } else {
            assert_eq!(checkpoint.matches(before).count(), 1);
            checkpoint = checkpoint.replacen(before, after, 1);
        }
    }
    if let Some(acquired) =
        super::declared_history_inputs::expected_value(source, &digest(&checkpoint), raw)
    {
        assert_eq!(acquired, checkpoint);
        if received != raw {
            assert_eq!(
                received, acquired,
                "unchanged original child checkpoint: {source}"
            );
        }
        acquired
    } else {
        received.to_owned()
    }
}

// Acquire only complete checkpoints declared by immutable original inverse tables.
// Callers still execute their own ordered inverses and whole-original assertions.
pub(crate) fn guard_stage(
    caller: &'static str,
    supplied: &str,
    original: &str,
    request: &serde_json::Value,
) -> String {
    let source = request["source"]
        .as_str()
        .or_else(|| request["path"].as_str())
        .unwrap();
    let original_digest = digest(original);
    let mut tables = Vec::new();
    for (index, (path, expected, raw)) in guard_tables().into_iter().enumerate() {
        let rows = guard_records(index, expected, raw);
        if rows.as_array().unwrap().contains(request) {
            assert_eq!(
                std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap(),
                raw
            );
            tables.push((path, expected));
        }
    }
    assert!(!tables.is_empty(), "unchanged original inverse request");
    let declared_original = request["original_sha256"]
        .as_str()
        .or_else(|| request["before_sha256"].as_str())
        .or_else(|| request["sha256"].as_str())
        .unwrap();
    assert_eq!(
        original_digest, declared_original,
        "complete original input"
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
            .expect("closed complete caller input");
        assert_eq!(authenticated, supplied);
    }
    let mut answers = Vec::new();
    for part in 0..27 {
        for row in records(part).as_array().unwrap().iter() {
            if row["source"] != source
                || !row["fixture_originals"].as_array().is_some_and(|proofs| {
                    proofs.iter().any(|proof| {
                        proof["sha256"] == original_digest
                            && tables.iter().any(|(path, sha)| {
                                proof["provenance"] == *path && proof["table_sha256"] == *sha
                            })
                    })
                })
            {
                continue;
            }
            let checkpoint = restore(source, row);
            let edits = request["edits"].as_array().unwrap();
            let inverse = |reverse| {
                let mut restored = checkpoint.clone();
                for index in 0..edits.len() {
                    let edit = &edits[if reverse {
                        edits.len() - 1 - index
                    } else {
                        index
                    }];
                    let after = edit["after"]
                        .as_str()
                        .or_else(|| edit["new"].as_str())
                        .unwrap();
                    if after.is_empty() || restored.matches(after).count() != 1 {
                        return false;
                    }
                    restored = restored.replacen(after, edit["before"].as_str().unwrap(), 1);
                }
                restored == original
            };
            if (inverse(false) || inverse(true)) && !answers.contains(&checkpoint) {
                answers.push(checkpoint);
            }
        }
    }
    assert_eq!(
        answers.len(),
        1,
        "unique complete original inverse checkpoint: {source}"
    );
    answers.pop().unwrap()
}

fn guard_records(index: usize, expected: &str, raw: &str) -> &'static serde_json::Value {
    static CACHE: [std::sync::OnceLock<serde_json::Value>; 13] =
        [const { std::sync::OnceLock::new() }; 13];
    CACHE[index].get_or_init(|| {
        assert_eq!(digest(raw), expected);
        serde_json::from_str(raw).unwrap()
    })
}

fn guard_tables() -> [(&'static str, &'static str, &'static str); 13] {
    [
        (
            "docs/evidence/selected-session-static-refusal-781/exact-source-inverses.json",
            "5080ec6ca01d0d3165427f06fe3a2d8413e171d2725fbf8c8f15c07de91d5cce",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/selected-session-static-refusal-781/exact-source-inverses.json"
            )),
        ),
        (
            "docs/evidence/legacy-nominal-sharing-827/exact-source-inverses.json",
            "98335b571361f43b4d199270a9eb9c5918126e1a4b46a7ceefdaec5e63648e7f",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/legacy-nominal-sharing-827/exact-source-inverses.json"
            )),
        ),
        (
            "docs/evidence/paid-generic-constructor-graph-824/exact-source-inverses.json",
            "7f52d853afc872e686614e632e3ec90bfd5beaa17c4fa7d5d5f19fe772c2840d",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/paid-generic-constructor-graph-824/exact-source-inverses.json"
            )),
        ),
        (
            "docs/evidence/lifecycle-inline-active-path/whole-source-inverses.json",
            "de34a7d186ed6068a4685b63960f8d7c36da5547682f26486ca6567887a49ca3",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/lifecycle-inline-active-path/whole-source-inverses.json"
            )),
        ),
        (
            "docs/evidence/loan-candidate-inventory-reuse/exact-source-inverses.json",
            "94cc7f70e8cb2cb200526edfa0f5b4550b6c2c8b1d6b12361ddbce8dd7115d65",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/loan-candidate-inventory-reuse/exact-source-inverses.json"
            )),
        ),
        (
            "docs/evidence/paid-nul-audit-824/exact-source-inverses.json",
            "a4795c4a596231aad97d70f0e3c603c4a8fa81b03980d05ffec804d36db7b14a",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/paid-nul-audit-824/exact-source-inverses.json"
            )),
        ),
        (
            "docs/evidence/public-boundary-fixture-repair/exact-source-inverses.json",
            "7ee650cb29763d81de4449302b92abbba77b3a426b3c48852b4a2299d6b609ff",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/public-boundary-fixture-repair/exact-source-inverses.json"
            )),
        ),
        (
            "docs/evidence/paid-cfg-child-scratch/exact-source-inverses.json",
            "e333d2031edb8f13c61d7df30d06eb4209944cab6c0154715b963a178e35ff50",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/paid-cfg-child-scratch/exact-source-inverses.json"
            )),
        ),
        (
            "docs/evidence/ordinary-lexical-return-operands-832/exact-source-inverses.json",
            "cbf4b77b009ab67d5cfbb070081e3883842dca83b4457f6db046d64124dfb1f3",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/ordinary-lexical-return-operands-832/exact-source-inverses.json"
            )),
        ),
        ("docs/evidence/maina3-independent-custody-20261016/selected-session-inverse19.json", "7b12f6b0806a8a8daa7089fc385f122dec7c7beaade797e143ca19f8074886ff", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/maina3-independent-custody-20261016/selected-session-inverse19.json"))),
        ("docs/evidence/main8a-independent-custody-20261016/selected-session-inverse19.json", "cbe7d0c63ec9390379e0a967ca777beef308ff4e47529fb178204f32eff9c78c", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main8a-independent-custody-20261016/selected-session-inverse19.json"))),
        ("docs/evidence/maina3-independent-custody-20261016/legacy-nominal-inverse18.json", "16ded8556f9460661a4056b1d8b8a3a113ca5c495b9efeed95f6a404d04a0059", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/maina3-independent-custody-20261016/legacy-nominal-inverse18.json"))),
        ("docs/evidence/prelude-forecast-286/exact-source-inverses.json", "9583da7f424d9e962b60023a62443e3239e5aaa7015e5f3026fdbabc46ef5610", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/prelude-forecast-286/exact-source-inverses.json"))),
    ]
}

#[test]
fn immutable_guard_requests_preserve_full_originals_and_refuse_foreign_inputs() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut checked = 0;
    for (path, _, raw) in guard_tables() {
        let rows: serde_json::Value = serde_json::from_str(raw).unwrap();
        for request in rows.as_array().unwrap() {
            let source = request["source"]
                .as_str()
                .or_else(|| request["path"].as_str())
                .unwrap();
            let original = if let Some(original) = request["original_bytes"].as_str() {
                original.to_owned()
            } else if let Some(archive) = request["archive"]
                .as_str()
                .or_else(|| request["original"].as_str())
            {
                let absolute = root.join(archive);
                let absolute = if absolute.is_file() {
                    absolute
                } else if path == "docs/evidence/maina3-independent-custody-20261016/selected-session-inverse19.json" || path == "docs/evidence/main8a-independent-custody-20261016/selected-session-inverse19.json" {
                    root.join("docs/evidence/selected-session-static-refusal-781").join(archive)
                } else if path == "docs/evidence/maina3-independent-custody-20261016/legacy-nominal-inverse18.json" {
                    root.join("docs/evidence/legacy-nominal-sharing-827").join(archive)
                } else {
                    root.join(path).parent().unwrap().join(archive)
                };
                std::fs::read_to_string(absolute).unwrap()
            } else {
                String::new()
            };
            let checkpoint = guard_stage(file!(), compiled(source), &original, request);
            assert!(!checkpoint.is_empty());
            if checked == 0 {
                for supplied in [
                    "foreign".to_owned(),
                    compiled(source)[..compiled(source).len() - 1].to_owned(),
                    format!("{}{}", compiled(source), compiled(source)),
                ] {
                    assert!(std::panic::catch_unwind(|| guard_stage(
                        file!(),
                        &supplied,
                        &original,
                        request
                    ))
                    .is_err());
                }
                let mut changed = request.clone();
                changed["edits"][0]["before"] =
                    serde_json::Value::String("foreign inverse".to_owned());
                assert!(std::panic::catch_unwind(|| guard_stage(
                    file!(),
                    compiled(source),
                    &original,
                    &changed
                ))
                .is_err());
                assert!(std::panic::catch_unwind(|| guard_stage(
                    file!(),
                    compiled(source),
                    &format!("{original}!"),
                    request
                ))
                .is_err());
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 140);
}
