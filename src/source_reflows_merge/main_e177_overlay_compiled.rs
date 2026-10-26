#[path = "main_e177_overlay_lookup.rs"]
#[rustfmt::skip]
mod lookup;
pub(super) use lookup::compiled;

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};
    use std::ops::Range;

    const ROOT: &str = include_str!("main_e177_overlay_v2.rs");
    const CHILD: &str = include_str!("main_e177_overlay_compiled.rs");
    const BEFORE: &str =
        include_str!("../../e/acc0b9f4f9f2b0b411db1bfdbfdf56549973069ddc3d09fbae2ab3879577aa22");
    const GLUE: &str = "#[path = \"main_e177_overlay_compiled.rs\"]\n#[rustfmt::skip]\nmod compiled_sources;\n\npub(super) fn compiled(source: &str) -> Option<&'static str> {\n    compiled_sources::compiled(source)\n}\n";

    fn before() -> &'static str {
        assert_eq!(BEFORE.len(), 80032);
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(BEFORE.as_bytes()))
            ),
            "acc0b9f4f9f2b0b411db1bfdbfdf56549973069ddc3d09fbae2ab3879577aa22"
        );
        BEFORE
    }

    fn constant(code: &str, name: &str) -> Range<usize> {
        let prefix = format!("const {name}:");
        assert_eq!(code.matches(&prefix).count(), 1);
        let start = code.find(&prefix).unwrap();
        let terminator = if matches!(name, "ADDITIONS" | "REPLACEMENTS") {
            "];"
        } else {
            ";"
        };
        let end = start + code[start..].find(terminator).unwrap() + terminator.len();
        start..end
    }

    fn array(code: &str, name: &str) -> Vec<String> {
        let declaration = &code[constant(code, name)];
        let contents = declaration
            .split_once("= &[")
            .unwrap()
            .1
            .strip_suffix("];")
            .unwrap();
        contents
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                if line.is_empty() {
                    None
                } else {
                    Some(serde_json::from_str(line.strip_suffix(',').unwrap()).unwrap())
                }
            })
            .collect()
    }

    fn lookup(code: &str) -> &str {
        let prefix = "pub(super) fn compiled(source: &str) -> Option<&'static str> {\n";
        assert_eq!(code.matches(prefix).count(), 1);
        let start = code.find(prefix).unwrap();
        let end = start + code[start..].find("\n}\n").unwrap() + "\n}\n".len();
        &code[start..end]
    }

    fn root_before_invocation_metadata(root: &str) -> String {
        let mut restored = root.to_owned();
        for (after, before) in [
            ("        assert_eq!((loads.0.get(), loads.1.get()), (1, 1), \"both metadata loaders run once across both whole routes\");\n", ""),
            ("        // Borrow immutable metadata only. Both independently compiled routes\n        // still acquire current/archive bytes afresh for every supplied input.\n        let loads = (std::cell::Cell::new(0), std::cell::Cell::new(0));\n        let mut invocation = super::super::ProjectionInvocation::with_loaders(|| {\n            loads.0.set(loads.0.get() + 1);\n            super::super::packet()\n        }, || {\n            loads.1.set(loads.1.get() + 1);\n            super::super::clippy_stage::rows()\n        });\n        let mut value_direct = |source: &str, expected: &str, supplied: &str| {\n            let live = invocation.value_direct(source, expected, supplied);\n            let old = invocation.independent_old_value_direct(source, expected, supplied);\n            assert_eq!(live, old, \"complete old/live output or foreign refusal parity\");\n            live\n        };\n        super::super::packet_lookup::assert_independent_old_direct_source();\n", ""),
            ("pub(super) fn current(row: &Value) -> Result<String, &'static str> {\n", "fn current(row: &Value) -> Result<String, &'static str> {\n"),
            ("pub(super) fn image(value: &Value) -> Result<String, &'static str> {\n", "fn image(value: &Value) -> Result<String, &'static str> {\n"),
            ("pub(super) const ADDITIONS: &[&str] = &[\n", "const ADDITIONS: &[&str] = &[\n"),
            ("pub(super) const REPLACEMENTS: &[&str] = &[\n", "const REPLACEMENTS: &[&str] = &[\n"),
        ] {
            assert_eq!(restored.matches(after).count(), 1);
            restored = restored.replacen(after, before, 1);
        }
        restored
    }

    fn restore_whole_before(root: &str, child: &str) -> String {
        // These arguments must be the entire independently compiled current
        // source images, before any metadata or whole-source normalization.
        assert_eq!(root, ROOT);
        assert_eq!(child, CHILD);
        let rebound = super::census_split::parent_for_current_scope(child, include_str!("main_e177_overlay_lookup.rs"));
        let child = rebound.as_str();
        // CURRENT_SCOPE_NORMALIZATION_BEGIN
        // Reverse only the exact current literal-list packing and new scope.
        // Actual current root/child equality above precedes every inverse.
        let prior_root = super::super::fragments_v3::v13_before(
            "src/source_reflows_merge/main_e177_overlay_v2.rs", root,
        );
        let root = prior_root.as_str();
        let mut current_scope_root = root.to_owned();
        let label_dispatch = "    if page[\"schema\"] == fragments_v3::SCHEMA_V12 {\n        let expanded = fragments_v3::expand_v12(&before, policy, page)?;\n        compose_values_v2(before, policy, &expanded)\n    } else if page[\"schema\"] == fragments_v3::SCHEMA_V11 {\n";
        assert_eq!(current_scope_root.matches(label_dispatch).count(), 1);
        current_scope_root = current_scope_root.replacen(label_dispatch, "    if page[\"schema\"] == fragments_v3::SCHEMA_V11 {\n", 1);
        let compatibility = "fragments_v3::expand_v10_compatible(&before, policy, page)?";
        assert_eq!(current_scope_root.matches(compatibility).count(), 1);
        current_scope_root = current_scope_root.replacen(compatibility, "fragments_v3::expand_v10(&before, policy, page)?", 1);
        let digest_dispatch = "    if page[\"schema\"] == fragments_v3::SCHEMA_V11 {\n        let expanded = fragments_v3::expand_v11(&before, policy, page)?;\n        compose_values_v2(before, policy, &expanded)\n    } else if page[\"schema\"] == fragments_v3::SCHEMA_V10 {\n";
        assert_eq!(current_scope_root.matches(digest_dispatch).count(), 1);
        current_scope_root = current_scope_root.replacen(digest_dispatch, "    if page[\"schema\"] == fragments_v3::SCHEMA_V10 {\n", 1);
        let wrapped_comment = concat!(
            "    // Original metadata is read as metadata. It is never substituted for a\n",
            "    // changed physical Rust source, and it retains its original individual cap.\n",
        );
        assert_eq!(current_scope_root.matches(wrapped_comment).count(), 2);
        current_scope_root = current_scope_root.replace(wrapped_comment, "");
        for name in ["REPLACEMENTS", "ADDITIONS"] {
            let at = constant(&current_scope_root, name);
            let declaration = current_scope_root[at].to_owned();
            let prefix = format!("const {name}: &[&str] = &");
            let json = declaration
                .strip_prefix(prefix.as_str())
                .unwrap()
                .strip_suffix(';')
                .unwrap();
            let values: Vec<String> = serde_json::from_str(json).unwrap();
            let mut expanded = format!("pub(super) const {name}: &[&str] = &[\n");
            for value in values {
                expanded.push_str(&format!(
                    "    {},\n",
                    serde_json::to_string(&value).unwrap()
                ));
            }
            expanded.push_str("];");
            let packed = format!("#[rustfmt::skip]\npub(super) {declaration}");
            assert_eq!(current_scope_root.matches(&packed).count(), 1);
            current_scope_root = current_scope_root.replacen(&packed, &expanded, 1);
        }
        for source in ["src/cleanup_plan/replay/path_join/before_singleton.rs", "src/cleanup_plan/replay/transition.rs", "src/workspace_graph/initial_report/build.rs", "docs/AGENT-SKILL-BUNDLE-V1.json", "src/cleanup_plan/replay/path_join/main_single_groups/receiver_tests.rs", "src/cleanup_plan/replay/path_join/main3d81_before.rs", "src/cleanup_plan/replay/path_join/field_local/mod.rs", "src/cleanup_plan/replay/path_join/field_local/frontier.rs", "src/cleanup_plan/replay/path_join/field_local/boxed_flow.rs", "src/cleanup_plan/replay/path_join/field_local/tests.rs", "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/retained.rs", "src/cleanup_plan/replay/factored/canonical_shared/lexical_successors/tests.rs", "src/cleanup_plan/replay/factored/canonical_shared/intern_arena.rs", "src/cleanup_plan/replay_tests/factored.rs", "src/cleanup_plan/replay/factored/moderate_tests.rs", "src/cleanup_plan/replay/factored/paired.rs", "src/cleanup_plan/replay/factored/paired/tests.rs", "src/workspace_graph/tests.rs", "src/cleanup_plan/replay/expression_facts/receiving/helpers/tests.rs", "src/cleanup_plan/replay/expression_facts/receiving/retained/exits/tests.rs", "src/cleanup_plan/replay/workspace_oracle.rs", "src/cleanup_plan/replay/branch_clone/inline_tests.rs", "src/cleanup_plan/replay/branch_clone/production_capacity.rs", "src/cleanup_plan/replay/branch_clone/production_capacity/tests.rs", "src/cleanup_plan/replay/factored/canonical_shared/empty_memo.rs", "src/workspace_graph/shape_identity/source_controls.rs", "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory_leaf_views.rs"] {
            let declaration = format!("    {source:?},\n");
            assert_eq!(current_scope_root.matches(&declaration).count(), 1);
            current_scope_root = current_scope_root.replacen(&declaration, "", 1);
        } let fitting_leaf_arm = "        \"src/cleanup_plan/replay_tests/factored.rs\" => Some(include_str!(\"../../src/cleanup_plan/replay_tests/factored.rs\")),\n"; assert_eq!(child.matches(fitting_leaf_arm).count(), 1, "exact new fitting-leaf lookup arm"); let fitting_prior_child = child.replacen(fitting_leaf_arm, "", 1); let child = fitting_prior_child.as_str();
        for source in [
            "src/workspace_graph/expected_projection/borrowed_imports/tests.rs",
            "src/workspace_graph/expected_projection/borrowed_imports/tests/full.rs",
            "src/hir/inspection/paid_nul.rs", "src/hir/inspection/paid_nul/pending.rs", "src/hir/inspection/paid_nul/tests.rs", "src/hir/inspection/paid_nul/reuse_before.rs", "src/hir/inspection/paid_nul/reuse_before/pending.rs", "src/hir/inspection/paid_nul/type_scratch.rs", "src/cleanup_plan/replay/continuation_storage.rs", "src/cleanup_plan/replay/continuation_storage/tests.rs", "src/cleanup_plan/replay/continuation_storage/reached.rs", "src/cleanup_plan/replay/continuation_storage/reached_before.rs", "src/cleanup_plan/replay/continuation_storage/reached_tests.rs","src/cleanup_plan/replay/path_join/fused_before.rs", "src/cleanup_plan/replay/path_join/fused_before_frontier.rs", "src/cleanup_plan/replay/path_join/fused_tests.rs",
            "src/cleanup_plan/replay/path_join/compact_failure_before.rs", "src/cleanup_plan/replay/path_join/compact_failure_tests.rs", "src/cleanup_plan/replay/path_join/contains_before.rs", "src/cleanup_plan/replay/path_join/contains_tests.rs", "src/cleanup_plan/replay/path_join/selected/tests/fixture_scope.rs", "src/source_reflows_merge/main_e177_overlay_digests_v13.rs", "src/source_reflows_merge/main_e177_overlay_digests_v13/logical_v12.rs", "src/source_reflows_merge/main_e177_overlay_digests_v13/tests.rs", "src/source_reflows_merge/main_e177_overlay_digests_v13/controller_transport.rs", "src/cleanup_plan/replay/expression_facts/receiving/retained/case_proof/tests.rs", "src/cleanup_plan/replay/expression_facts/receiving/remaining/tests.rs", "tests/wasm/scalar_browser_ci_contract/current_profiles573.rs",
            "src/cleanup_plan/replay/expression_facts/capacity/before_types.rs", "src/cleanup_plan/replay/expression_facts/capacity/cursor_tests.rs", "src/cleanup_plan/replay/expression_facts/capacity/type_cursor_tests.rs", "src/source_verify/constructor_tests/corpus/operator_refusals.rs", "src/cleanup_plan/replay/path_join/direct_one_before.rs", "src/cleanup_plan/replay/path_join/direct_one_before/boxed_flow.rs", "src/cleanup_plan/replay/path_join/direct_one_before/frontier.rs", "src/cleanup_plan/replay/path_join/direct_one_tests.rs",
            "src/cleanup_plan/replay/skeleton_work/query_proof.rs", "src/cleanup_plan/replay/skeleton_work/query_proof/before.rs", "src/cleanup_plan/replay/skeleton_work/query_proof/tests.rs", "src/bounded_output/exclusive_debits/path_census.rs", "src/bounded_output/request_reservation.rs", "src/source_reflows_merge/main_e177_overlay_lookup.rs",
            "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/hir_last_use.rs", "src/cleanup_plan/replay/factored/hir_memo_release_tests.rs", "src/cleanup_plan/replay/path_join/shared_insertions_before.rs", "src/cleanup_plan/replay/path_join/shared_insertions_before/boxed_flow.rs", "src/cleanup_plan/replay/path_join/shared_insertions_before/boxed_states.rs", "src/cleanup_plan/replay/path_join/shared_insertions_before/frontier.rs", "src/cleanup_plan/replay/path_join/shared_insertions_tests.rs", "src/cleanup_plan/replay/plan_prefix.rs", "src/cleanup_plan/replay/plan_prefix/before.rs", "src/cleanup_plan/replay/plan_prefix/tests.rs", "src/cleanup_plan/replay/skeleton_work/plan_prefix.rs",
            "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/ordinary_receiving_tests.rs", "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/ordinary_receiving_before.rs", "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/compact/ordinary_receiving_before.rs", "src/cleanup_plan/replay/path_join/held_edge_before.rs", "src/cleanup_plan/replay/path_join/held_edge_tests.rs",
            "src/cleanup_plan/replay/branch_clone/production_capacity/before_short.rs", "src/cleanup_plan/replay/branch_clone/short_plan.rs", "src/cleanup_plan/replay/nested_shape.rs", "src/cleanup_plan/replay/nested_shape/scalar_tests.rs",
            "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/ordinary_labels.rs",
            "src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/before_frame_pricing.rs", "src/cleanup_plan/replay/selected_owner/ordinary.rs", "src/cleanup_plan/replay/selected_owner/ordinary/tests.rs", "src/cleanup_plan/replay/expression_facts/receiving/retained/last_use.rs", "src/cleanup_plan/replay/expression_facts/receiving/retained/last_use/before.rs", "src/cleanup_plan/replay/expression_facts/receiving/retained/last_use/tests.rs",
            "src/cleanup_plan/replay/path_join/selected/receiving.rs",
            "src/cleanup_plan/replay/path_join/selected/tests/mod.rs",
            "src/cleanup_plan/replay/path_join/selected/before_frontier/finish.rs",
            "src/cleanup_plan/replay/path_join/selected/before_frontier/mod.rs",
            "src/cleanup_plan/replay/path_join/selected/before_frontier/queries.rs",
            "src/cleanup_plan/replay/path_join/selected/before_frontier/receiving.rs",
            "src/cleanup_plan/replay/path_join/selected/before_frontier/walk.rs",
            "src/cleanup_plan/replay/path_join/selected/boxed_flow.rs",
            "src/cleanup_plan/replay/path_join/selected/frontier.rs",
            "src/cleanup_plan/replay/path_join/selected/frontier_tests.rs",
            "src/cleanup_plan/replay/path_join/selected/frontier_tests/full_callers.rs",
            "src/cleanup_plan/replay/path_join/selected/frontier_tests/lexical_before.rs",
            "src/cleanup_plan/replay/path_join/selected/frontier_tests/lexical_before/ordinary.rs",
            "src/cleanup_plan/replay/path_join/selected/frontier_tests/scalar_before.rs",
            "src/hir/validation/scope_checkpoint/marks/before.rs",
            "src/hir/validation/scope_checkpoint/marks/tests.rs",
            "src/source_reflows_merge/main_e177_overlay_pins_v11.rs",
            "src/cleanup_plan/replay/factored/early_release.rs",
            "src/cleanup_plan/replay/factored/early_release/before.rs",
            "src/hir/validation/continuation_frame.rs",
            "src/hir/validation/frame_reuse.rs",
            "src/hir/validation/frame_reuse/before.rs",
            "src/hir/validation/frame_reuse/before_0.rs",
            "src/hir/validation/frame_reuse/before_1.rs",
            "src/hir/validation/frame_reuse/before_10.rs",
            "src/hir/validation/frame_reuse/before_11.rs",
            "src/hir/validation/frame_reuse/before_12.rs",
            "src/hir/validation/frame_reuse/before_13.rs",
            "src/hir/validation/frame_reuse/before_14.rs",
            "src/hir/validation/frame_reuse/before_15.rs",
            "src/hir/validation/frame_reuse/before_16.rs",
            "src/hir/validation/frame_reuse/before_17.rs",
            "src/hir/validation/frame_reuse/before_2.rs",
            "src/hir/validation/frame_reuse/before_3.rs",
            "src/hir/validation/frame_reuse/before_4.rs",
            "src/hir/validation/frame_reuse/before_5.rs",
            "src/hir/validation/frame_reuse/before_6.rs",
            "src/hir/validation/frame_reuse/before_7.rs",
            "src/hir/validation/frame_reuse/before_8.rs",
            "src/hir/validation/frame_reuse/before_9.rs",
            "src/hir/validation/frame_reuse/contracts.rs",
            "src/hir/validation/frame_reuse/contracts_before.rs",
            "src/hir/validation/frame_reuse/custody.rs",
            "src/hir/validation/frame_reuse/function.rs",
            "src/hir/validation/frame_reuse/function_before.rs",
            "src/hir/validation/frame_reuse/source_inverse.rs",
            "src/hir/validation/frame_reuse/tests.rs",
            "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/compact/active.rs",
            "src/cleanup_plan/replay/owned_suffix.rs",
            "src/cleanup_plan/replay/final_output/current_observation_tests.rs",
            "src/cleanup_plan/replay/final_output/tests.rs",
            "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/compact.rs",
            "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/compact/compact_targets.rs",
            "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/compact/payments.rs",
            "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/tests/compact_results.rs",
            "src/cleanup_plan/replay/final_output.rs",
            "src/cleanup_plan/replay/final_output/compact.rs",
            "src/cleanup_plan/replay/final_output/tests/compact_results.rs",
            "src/cleanup_plan/replay/path_join/boxed_states.rs",
            "src/cleanup_plan/replay/path_join/boxed_states/receiving_tests.rs",
            "src/cleanup_plan/replay/sequence_vector.rs",
            "src/cleanup_plan/replay/skeleton_work/enumerated_prefix/flat_probe.rs",
            "tests/source_locked_contracts.rs",
            "src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/counter_tests.rs",
            "src/cleanup_plan/replay/expression_facts/arena/prefix/tests.rs",
            "src/cleanup_plan/replay/physical_budget/authentication.rs",
            "src/source_reflows_merge/final_ready_inputs/projection_publication.rs",
            "src/cleanup_plan/replay/factored/strategy/compact_receiving.rs",
            "src/cleanup_plan/replay/factored/strategy/compact_receiving/before.rs",
            "src/cleanup_plan/replay/factored/strategy/compact_receiving/tests.rs",
            "src/cli/help/authoring/contract_owners.rs",
            "src/cli/help/authoring/contract_owners/tests.rs",
            "src/source_reflows_merge/boxed_inputs.rs",
            "src/workspace_graph/expected_projection/shared_local_callees.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/before_projection.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/controls_tests.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/invoice.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/tests.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/walk.rs",
            "src/cleanup_plan/replay/ordinary_plan_history/completed_fork.rs",
            "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/before.rs",
            "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/content.rs",
            "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission.rs",
            "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/tests.rs",
            "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/whole_tests.rs",
            "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/shipping.rs",
            "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/tests.rs",
            "src/cleanup_plan/replay/ordinary_plan_history/producer_admission.rs",
            "src/cleanup_plan/replay/ordinary_plan_history/producer_joint_tests.rs",
            "src/cleanup_plan/replay/ordinary_plan_history/producer_view.rs",
            "src/cleanup_plan/replay/ordinary_plan_history/producer_view_tests.rs",
            "src/cleanup_plan/replay/skeleton_work/dag_storage/flat_fanout.rs",
            "src/workspace_graph/expected_projection/call_carriers_joint.rs",
            "src/workspace_graph/expected_projection/call_carriers_directory/reused.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/coupled_projection.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/reuse_tests.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/reused.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/reused_controls.rs",
            "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation.rs",
            "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation/tests.rs",
            "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/shipping/flat_admission.rs",
            "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/shipping/flat_admission/tests.rs",
            "src/source_reflows_merge/main_e177_overlay_labels_v12.rs",
            "src/source_reflows_merge/main_e177_overlay_labels_v12/tests.rs",
            "src/workspace_graph/expected_projection/call_carriers_directory/modular.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/modular_census.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/modular_pair.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/modular_pair_tests.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/modular_projection.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/modular_session.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/modular_tests.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/paired.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/paired_projection.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/paired_tests.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/record_before.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/record_signature_tests.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/record_signatures.rs",
            "src/workspace_graph/expected_projection/private_layout/names/interface_census.rs",
            "src/workspace_graph/expected_projection/private_layout/names/interface_before.rs",
            "src/workspace_graph/expected_projection/private_layout/names/interface_tests.rs",
            "src/cleanup_plan/replay/skeleton_work/enumerated_prefix/flat_probe/before.rs",
            "src/cleanup_plan/replay/skeleton_work/enumerated_prefix/flat_probe/receiving_tests.rs",
            "src/call_index/validation_before.rs",
            "src/call_index/validation_receipt_tests.rs",
            "src/codegen/native_owned_data_provider/emit_provider_before.rs",
            "src/codegen/native_owned_data_provider/validation_reuse_tests.rs",
            "src/cleanup_plan/replay/path_join/before_frontier/boxed_states.rs",
            "src/cleanup_plan/replay/path_join/before_frontier/branch_join.rs",
            "src/cleanup_plan/replay/path_join/before_frontier/mod.rs",
            "src/cleanup_plan/replay/path_join/before_frontier/storage.rs",
            "src/cleanup_plan/replay/path_join/before_frontier/storage/atomic_controls.rs",
            "src/cleanup_plan/replay/path_join/before_frontier/storage/empty_renewals.rs",
            "src/cleanup_plan/replay/path_join/before_frontier/tests.rs",
            "src/cleanup_plan/replay/path_join/before_frontier/transition.rs",
            "src/cleanup_plan/replay/path_join/boxed_flow.rs",
            "src/cleanup_plan/replay/path_join/frontier.rs",
            "src/cleanup_plan/replay/path_join/frontier_tests.rs",
            "src/cleanup_plan/replay/path_join/frontier_tests/custody.rs",
            "src/cleanup_plan/replay/path_join/frontier_tests/whole.rs",
            "src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort.rs",
            "src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/before.rs",
            "src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/tests.rs",
            "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/shipping/flat_admission/immediate_before.rs",
            "src/test_allocation_observer/phase_peaks.rs",
            "src/loan_plan/paid_plan/before_reachability_release.rs",
            "src/loan_plan/paid_plan/reachability_release_tests.rs",
            "src/loan_plan/funded_flat/before_reachability_release.rs",
            "src/workspace_graph/loan_retention/overlap_tests.rs",
        ] {
            let declaration = format!("    {source:?},\n");
            assert_eq!(current_scope_root.matches(&declaration).count(), 1);
            current_scope_root = current_scope_root.replacen(&declaration, "", 1);
        }
        // Retain the exact prior comment-restoration controls after restoring
        // the physical comments carried by the incoming source epoch.
        for (current, previous) in [
            ("        let restored = format!(\"fn load(){old}\");\n", "        let restored = format!(\"fn load(){old}\").replacen(\"    let extension = include_str!(\\\"../../docs/evidence/fmt-local-441-20261018/extension.json\\\");\\n\", \"    // Original metadata is read as metadata. It is never substituted for a\\n    // changed physical Rust source, and it retains its original individual cap.\\n    let extension = include_str!(\\\"../../docs/evidence/fmt-local-441-20261018/extension.json\\\");\\n\", 1);\n"),
            ("                \"Ok((policy, json(&physical)?))\",\n                1,\n            );\n", "                \"Ok((policy, json(&physical)?))\",\n                1,\n            ).replacen(\"    let extension = include_str!(\\\"../../docs/evidence/fmt-local-441-20261018/extension.json\\\");\\n\", \"    // Original metadata is read as metadata. It is never substituted for a\\n    // changed physical Rust source, and it retains its original individual cap.\\n    let extension = include_str!(\\\"../../docs/evidence/fmt-local-441-20261018/extension.json\\\");\\n\", 1);\n"),
        ] {
            assert_eq!(current_scope_root.matches(current).count(), 1);
            current_scope_root = current_scope_root.replacen(current, previous, 1);
        }
        let mut current_scope_child = child.to_owned();
        let packed_separator = ")),        \"src/";
        assert_eq!(current_scope_child.matches(packed_separator).count(), 24);
        current_scope_child = current_scope_child.replace(packed_separator, ")),\n        \"src/");
        for source in [
            "src/workspace_graph/expected_projection/borrowed_imports/tests.rs",
            "src/workspace_graph/expected_projection/borrowed_imports/tests/full.rs",
            "src/cleanup_plan/replay/factored.rs", "src/cleanup_plan/replay/factored/strategy.rs", "src/cleanup_plan/replay/factored/paired.rs", "src/cleanup_plan/replay/factored/paired/tests.rs",
            "src/hir/inspection/paid_nul.rs", "src/hir/inspection/paid_nul/pending.rs", "src/hir/inspection/paid_nul/tests.rs", "src/hir/inspection/paid_nul/reuse_before.rs", "src/hir/inspection/paid_nul/reuse_before/pending.rs", "src/hir/inspection/paid_nul/type_scratch.rs", "src/cleanup_plan/replay/continuation_storage.rs", "src/cleanup_plan/replay/continuation_storage/tests.rs", "src/cleanup_plan/replay/continuation_storage/reached.rs", "src/cleanup_plan/replay/continuation_storage/reached_before.rs", "src/cleanup_plan/replay/continuation_storage/reached_tests.rs","src/cleanup_plan/replay/path_join/fused_before.rs", "src/cleanup_plan/replay/path_join/fused_before_frontier.rs", "src/cleanup_plan/replay/path_join/fused_tests.rs",
            "src/cleanup_plan.rs",
            "src/workspace_graph/tests.rs",
            "src/cleanup_plan/replay/expression_facts/receiving/helpers/tests.rs",
            "src/cleanup_plan/replay/expression_facts/receiving/retained/exits/tests.rs",
            "src/cleanup_plan/replay/workspace_oracle.rs",
            "src/cleanup_plan/replay/path_join/compact_failure_before.rs", "src/cleanup_plan/replay/path_join/compact_failure_tests.rs", "src/cleanup_plan/replay/path_join/contains_before.rs", "src/cleanup_plan/replay/path_join/contains_tests.rs", "src/cleanup_plan/replay/path_join/selected/tests/fixture_scope.rs", "src/source_reflows_merge/main_e177_overlay_digests_v13.rs", "src/source_reflows_merge/main_e177_overlay_digests_v13/logical_v12.rs", "src/source_reflows_merge/main_e177_overlay_digests_v13/tests.rs", "src/source_reflows_merge/main_e177_overlay_digests_v13/controller_transport.rs", "src/cleanup_plan/replay/expression_facts/receiving/retained/case_proof/tests.rs", "src/cleanup_plan/replay/expression_facts/receiving/remaining/tests.rs", "tests/wasm/scalar_browser_ci_contract/current_profiles573.rs",
            "src/cleanup_plan/replay/expression_facts/capacity/before_types.rs", "src/cleanup_plan/replay/expression_facts/capacity/cursor_tests.rs", "src/cleanup_plan/replay/expression_facts/capacity/type_cursor_tests.rs", "src/source_verify/constructor_tests/corpus/operator_refusals.rs", "src/cleanup_plan/replay/path_join/direct_one_before.rs", "src/cleanup_plan/replay/path_join/direct_one_before/boxed_flow.rs", "src/cleanup_plan/replay/path_join/direct_one_before/frontier.rs", "src/cleanup_plan/replay/path_join/direct_one_tests.rs",
            "src/cleanup_plan/replay/skeleton_work/query_proof.rs", "src/cleanup_plan/replay/skeleton_work/query_proof/before.rs", "src/cleanup_plan/replay/skeleton_work/query_proof/tests.rs", "src/bounded_output/exclusive_debits/path_census.rs", "src/bounded_output/request_reservation.rs", "src/source_reflows_merge/main_e177_overlay_lookup.rs",
            "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/hir_last_use.rs", "src/cleanup_plan/replay/factored/hir_memo_release_tests.rs", "src/cleanup_plan/replay/path_join/shared_insertions_before.rs", "src/cleanup_plan/replay/path_join/shared_insertions_before/boxed_flow.rs", "src/cleanup_plan/replay/path_join/shared_insertions_before/boxed_states.rs", "src/cleanup_plan/replay/path_join/shared_insertions_before/frontier.rs", "src/cleanup_plan/replay/path_join/shared_insertions_tests.rs", "src/cleanup_plan/replay/plan_prefix.rs", "src/cleanup_plan/replay/plan_prefix/before.rs", "src/cleanup_plan/replay/plan_prefix/tests.rs", "src/cleanup_plan/replay/skeleton_work/plan_prefix.rs",
            "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/ordinary_receiving_tests.rs", "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/ordinary_receiving_before.rs", "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/compact/ordinary_receiving_before.rs", "src/cleanup_plan/replay/path_join/held_edge_before.rs", "src/cleanup_plan/replay/path_join/held_edge_tests.rs",
            "src/cleanup_plan/replay/branch_clone/production_capacity/before_short.rs", "src/cleanup_plan/replay/branch_clone/short_plan.rs", "src/cleanup_plan/replay/nested_shape.rs", "src/cleanup_plan/replay/nested_shape/scalar_tests.rs",
            "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/ordinary_labels.rs",
            "src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/before_frame_pricing.rs", "src/cleanup_plan/replay/selected_owner/ordinary.rs", "src/cleanup_plan/replay/selected_owner/ordinary/tests.rs", "src/cleanup_plan/replay/expression_facts/receiving/retained/last_use.rs", "src/cleanup_plan/replay/expression_facts/receiving/retained/last_use/before.rs", "src/cleanup_plan/replay/expression_facts/receiving/retained/last_use/tests.rs",
            "src/cleanup_plan/replay/path_join/selected/before_frontier/finish.rs",
            "src/cleanup_plan/replay/path_join/selected/before_frontier/mod.rs",
            "src/cleanup_plan/replay/path_join/selected/before_frontier/queries.rs",
            "src/cleanup_plan/replay/path_join/selected/before_frontier/receiving.rs",
            "src/cleanup_plan/replay/path_join/selected/before_frontier/walk.rs",
            "src/cleanup_plan/replay/path_join/selected/boxed_flow.rs",
            "src/cleanup_plan/replay/path_join/selected/frontier.rs",
            "src/cleanup_plan/replay/path_join/selected/frontier_tests.rs",
            "src/cleanup_plan/replay/path_join/selected/frontier_tests/full_callers.rs",
            "src/cleanup_plan/replay/path_join/selected/frontier_tests/lexical_before.rs",
            "src/cleanup_plan/replay/path_join/selected/frontier_tests/lexical_before/ordinary.rs",
            "src/cleanup_plan/replay/path_join/selected/frontier_tests/scalar_before.rs",
            "src/hir/validation/scope_checkpoint/marks/before.rs",
            "src/hir/validation/scope_checkpoint/marks/tests.rs",
            "src/source_reflows_merge/main_e177_overlay_pins_v11.rs",
            "src/cleanup_plan/replay/factored/early_release.rs",
            "src/cleanup_plan/replay/factored/early_release/before.rs",
            "src/hir/validation/continuation_frame.rs",
            "src/hir/validation/frame_reuse.rs",
            "src/hir/validation/frame_reuse/before.rs",
            "src/hir/validation/frame_reuse/before_0.rs",
            "src/hir/validation/frame_reuse/before_1.rs",
            "src/hir/validation/frame_reuse/before_10.rs",
            "src/hir/validation/frame_reuse/before_11.rs",
            "src/hir/validation/frame_reuse/before_12.rs",
            "src/hir/validation/frame_reuse/before_13.rs",
            "src/hir/validation/frame_reuse/before_14.rs",
            "src/hir/validation/frame_reuse/before_15.rs",
            "src/hir/validation/frame_reuse/before_16.rs",
            "src/hir/validation/frame_reuse/before_17.rs",
            "src/hir/validation/frame_reuse/before_2.rs",
            "src/hir/validation/frame_reuse/before_3.rs",
            "src/hir/validation/frame_reuse/before_4.rs",
            "src/hir/validation/frame_reuse/before_5.rs",
            "src/hir/validation/frame_reuse/before_6.rs",
            "src/hir/validation/frame_reuse/before_7.rs",
            "src/hir/validation/frame_reuse/before_8.rs",
            "src/hir/validation/frame_reuse/before_9.rs",
            "src/hir/validation/frame_reuse/contracts.rs",
            "src/hir/validation/frame_reuse/contracts_before.rs",
            "src/hir/validation/frame_reuse/custody.rs",
            "src/hir/validation/frame_reuse/function.rs",
            "src/hir/validation/frame_reuse/function_before.rs",
            "src/hir/validation/frame_reuse/source_inverse.rs",
            "src/hir/validation/frame_reuse/tests.rs",
            "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/compact/active.rs",
            "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/compact.rs",
            "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/compact/compact_targets.rs",
            "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/compact/payments.rs",
            "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/tests/compact_results.rs",
            "src/cleanup_plan/replay/final_output.rs",
            "src/cleanup_plan/replay/final_output/compact.rs",
            "src/cleanup_plan/replay/final_output/tests/compact_results.rs",
            "src/cleanup_plan/replay/path_join/boxed_states.rs",
            "src/cleanup_plan/replay/path_join/boxed_states/receiving_tests.rs",
            "src/cleanup_plan/replay/sequence_vector.rs",
            "src/cleanup_plan/replay/skeleton_work/enumerated_prefix/flat_probe.rs",
            "src/source_reflows_merge/final_ready_inputs/projection_publication.rs",
            "src/cleanup_plan/replay/factored/strategy/compact_receiving.rs",
            "src/cleanup_plan/replay/factored/strategy/compact_receiving/before.rs",
            "src/cleanup_plan/replay/factored/strategy/compact_receiving/tests.rs",
            "src/cli/help/authoring/contract_owners.rs",
            "src/cli/help/authoring/contract_owners/tests.rs",
            "src/source_reflows_merge/boxed_inputs.rs",
            "src/workspace_graph/expected_projection/shared_local_callees.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/before_projection.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/controls_tests.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/invoice.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/tests.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/walk.rs",
            "src/cleanup_plan/replay/ordinary_plan_history/completed_fork.rs",
            "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/before.rs",
            "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/content.rs",
            "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission.rs",
            "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/tests.rs",
            "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/whole_tests.rs",
            "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/shipping.rs",
            "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/tests.rs",
            "src/cleanup_plan/replay/ordinary_plan_history/producer_admission.rs",
            "src/cleanup_plan/replay/ordinary_plan_history/producer_joint_tests.rs",
            "src/cleanup_plan/replay/ordinary_plan_history/producer_view.rs",
            "src/cleanup_plan/replay/ordinary_plan_history/producer_view_tests.rs",
            "src/cleanup_plan/replay/skeleton_work/dag_storage/flat_fanout.rs",
            "src/workspace_graph/expected_projection/call_carriers_joint.rs",
            "src/workspace_graph/expected_projection/call_carriers_directory/reused.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/coupled_projection.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/reuse_tests.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/reused.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/reused_controls.rs",
            "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation.rs",
            "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation/tests.rs",
            "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/shipping/flat_admission.rs",
            "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/shipping/flat_admission/tests.rs",
            "src/source_reflows_merge/main_e177_overlay_labels_v12.rs",
            "src/source_reflows_merge/main_e177_overlay_labels_v12/tests.rs",
            "src/workspace_graph/expected_projection/call_carriers_directory/modular.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/modular_census.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/modular_pair.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/modular_pair_tests.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/modular_projection.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/modular_session.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/modular_tests.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/paired.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/paired_projection.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/paired_tests.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/record_before.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/record_signature_tests.rs",
            "src/workspace_graph/expected_projection/shared_local_callees/record_signatures.rs",
            "src/workspace_graph/expected_projection/private_layout/names/interface_census.rs",
            "src/workspace_graph/expected_projection/private_layout/names/interface_before.rs",
            "src/workspace_graph/expected_projection/private_layout/names/interface_tests.rs",
            "src/cleanup_plan/replay/skeleton_work/enumerated_prefix/flat_probe/before.rs",
            "src/cleanup_plan/replay/skeleton_work/enumerated_prefix/flat_probe/receiving_tests.rs",
            "src/call_index/validation_before.rs",
            "src/call_index/validation_receipt_tests.rs",
            "src/codegen/native_owned_data_provider/emit_provider_before.rs",
            "src/codegen/native_owned_data_provider/validation_reuse_tests.rs",
            "src/cleanup_plan/replay/path_join/before_frontier/boxed_states.rs",
            "src/cleanup_plan/replay/path_join/before_frontier/branch_join.rs",
            "src/cleanup_plan/replay/path_join/before_frontier/mod.rs",
            "src/cleanup_plan/replay/path_join/before_frontier/storage.rs",
            "src/cleanup_plan/replay/path_join/before_frontier/storage/atomic_controls.rs",
            "src/cleanup_plan/replay/path_join/before_frontier/storage/empty_renewals.rs",
            "src/cleanup_plan/replay/path_join/before_frontier/tests.rs",
            "src/cleanup_plan/replay/path_join/before_frontier/transition.rs",
            "src/cleanup_plan/replay/path_join/boxed_flow.rs",
            "src/cleanup_plan/replay/path_join/frontier.rs",
            "src/cleanup_plan/replay/path_join/frontier_tests.rs",
            "src/cleanup_plan/replay/path_join/frontier_tests/custody.rs",
            "src/cleanup_plan/replay/path_join/frontier_tests/whole.rs",
            "src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort.rs",
            "src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/before.rs",
            "src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/tests.rs",
            "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/shipping/flat_admission/immediate_before.rs",
            "src/test_allocation_observer/phase_peaks.rs",
            "src/loan_plan/paid_plan/before_reachability_release.rs",
            "src/loan_plan/paid_plan/reachability_release_tests.rs",
            "src/loan_plan/funded_flat/before_reachability_release.rs",
            "src/workspace_graph/loan_retention/overlap_tests.rs",
        ] {
            let arm = format!("        {source:?} => Some(include_str!(\"../../{source}\")),\n");
            assert_eq!(current_scope_child.matches(&arm).count(), 1);
            current_scope_child = current_scope_child.replacen(&arm, "", 1);
        }
        let begin = concat!("        // ", "CURRENT_SCOPE_NORMALIZATION_BEGIN\n");
        let end = concat!("        // ", "CURRENT_SCOPE_NORMALIZATION_END\n");
        assert_eq!(current_scope_child.matches(begin).count(), 1);
        assert_eq!(current_scope_child.matches(end).count(), 1);
        let first = current_scope_child.find(begin).unwrap();
        let last = current_scope_child.find(end).unwrap() + end.len();
        assert!(first < last);
        current_scope_child.replace_range(first..last, "");
        let root = current_scope_root.as_str();
        let child = current_scope_child.as_str(); /* The new test-wire helper has complete current physical and compiled channels before its exact admission is removed for the old proof. */ let helper = "tests/workspace/semantic_graph/host_pins.rs"; let physical = std::fs::read_to_string( std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(helper), ).unwrap(); assert_eq!(super::compiled(helper), Some(physical.as_str())); let expected = format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(physical.as_bytes()))); assert_eq!(super::super::value_direct(helper, &expected, &physical), physical);
        // CURRENT_SCOPE_NORMALIZATION_END
        // Reverse this exact later metadata scope and comment-only packing
        // before every unchanged original span and whole-predecessor check.
        let mut preceding_root = root.to_owned();
        for source in [
            "src/cleanup_plan.rs",
            "src/cleanup_plan/replay/path_array/main44/mod.rs",
            "src/cleanup_plan/replay/path_array/main44/source_domain.rs",
            "src/source_reflows_merge/main44_custody.rs",
        ] {
            let declaration = format!("    {source:?},\n");
            assert_eq!(preceding_root.matches(&declaration).count(), 1);
            preceding_root = preceding_root.replacen(&declaration, "", 1);
        }
        for (packed, previous) in [
            ("        let restored = format!(\"fn load(){old}\").replacen(\"    let extension = include_str!(\\\"../../docs/evidence/fmt-local-441-20261018/extension.json\\\");\\n\", \"    // Original metadata is read as metadata. It is never substituted for a\\n    // changed physical Rust source, and it retains its original individual cap.\\n    let extension = include_str!(\\\"../../docs/evidence/fmt-local-441-20261018/extension.json\\\");\\n\", 1);\n", "        let restored = format!(\"fn load(){old}\");\n"),
            ("            ).replacen(\"    let extension = include_str!(\\\"../../docs/evidence/fmt-local-441-20261018/extension.json\\\");\\n\", \"    // Original metadata is read as metadata. It is never substituted for a\\n    // changed physical Rust source, and it retains its original individual cap.\\n    let extension = include_str!(\\\"../../docs/evidence/fmt-local-441-20261018/extension.json\\\");\\n\", 1);\n", "            );\n"),
            ("        // Borrow immutable metadata only. Both independently compiled routes still acquire current/archive bytes afresh for every supplied input.\n", "        // Borrow immutable metadata only. Both independently compiled routes\n        // still acquire current/archive bytes afresh for every supplied input.\n"),
            ("pub(super) fn value_direct(source: &str, expected: &str, supplied: &str) -> String { // A whole logical-table view is selected only after its actual current physical, compiled and archive channels have authenticated. Preserve the complete historical stage fallback below for every other input.\n", "pub(super) fn value_direct(source: &str, expected: &str, supplied: &str) -> String {\n    // A whole logical-table view is selected only after its actual current physical, compiled and archive channels have authenticated. Preserve the complete historical stage fallback below for every other input.\n"),
            ("    } // compose() authenticated policy membership against these exact constants.\n", "    }\n    // compose() authenticated policy membership against these exact constants.\n"),
            ("        } // A genuine declared leaf edge has to use the complete same-scope row.\n", "        }\n        // A genuine declared leaf edge has to use the complete same-scope row.\n"),
            ("        ); // Even a same-key forged DATA object cannot substitute a verdict.\n", "        );\n        // Even a same-key forged DATA object cannot substitute a verdict.\n"),
            ("        assert!(Arc::ptr_eq(&first, &read(&mut data).unwrap())); // Matching physical bytes with foreign compiled bytes are still refused.\n", "        assert!(Arc::ptr_eq(&first, &read(&mut data).unwrap()));\n        // Matching physical bytes with foreign compiled bytes are still refused.\n"),
        ] {
            assert_eq!(preceding_root.matches(packed).count(), 1);
            preceding_root = preceding_root.replacen(packed, previous, 1);
        }
        let preceding_root = root_before_invocation_metadata(&preceding_root);
        let root = preceding_root.as_str();
        // Undo this exact later funded-test acquisition before the complete old inverses.
        let declaration = "    \"src/cleanup_plan/replay/sequence_vector/funded_tests.rs\",\n";
        assert_eq!(root.matches(declaration).count(), 1);
        let prior_root = root.replacen(declaration, "", 1);
        let replacement = "    \"src/cleanup_plan/replay/skeleton_work/prefix_fusion.rs\",\n";
        assert_eq!(prior_root.matches(replacement).count(), 1);
        let prior_root = prior_root.replacen(replacement, "", 1);
        let current_spacing = "        return direct;\n    }\n    if let Some(value) = super::clippy_stage::requested(source, expected, supplied) {\n";
        let previous_spacing = "        return direct;\n    }\n\n    if let Some(value) = super::clippy_stage::requested(source, expected, supplied) {\n";
        assert_eq!(prior_root.matches(current_spacing).count(), 1);
        let prior_root = prior_root.replacen(current_spacing, previous_spacing, 1);
        let current_tail = "}\n// Nondelegating current/prior view front, followed by the entire old stage path.\n";
        let previous_tail = "}\n\n// Nondelegating current/prior view front, followed by the entire old stage path.\n";
        assert_eq!(prior_root.matches(current_tail).count(), 1);
        let prior_root = prior_root.replacen(current_tail, previous_tail, 1);
        let arm = "        \"src/cleanup_plan/replay/sequence_vector/funded_tests.rs\" => Some(include_str!(\"../../src/cleanup_plan/replay/sequence_vector/funded_tests.rs\")),\n";
        assert_eq!(child.matches(arm).count(), 1);
        let prior_child = child.replacen(arm, "", 1);
        let root = prior_root.as_str();
        let child = prior_child.as_str();
        // Remove only these two later exact source admissions before the old inverses.
        let mut admitted_root = root.to_owned();
        for source in [
            "src/bounded_output/classifier_string.rs",
            "src/cleanup_plan/replay/skeleton_work/enumerated_prefix.rs",
        ] {
            let declaration = format!("    {source:?},\n");
            assert_eq!(admitted_root.matches(&declaration).count(), 1);
            admitted_root = admitted_root.replacen(&declaration, "", 1);
        }
        let mut admitted_child = child.to_owned();
        for source in [
            "src/bounded_output/classifier_string.rs",
            "src/cleanup_plan/replay/skeleton_work/enumerated_prefix.rs",
        ] {
            let arm = format!("        {source:?} => Some(include_str!(\"../../{source}\")),\n");
            assert_eq!(admitted_child.matches(&arm).count(), 1);
            admitted_child = admitted_child.replacen(&arm, "", 1);
        }
        let merged = "pub(super) fn value_direct(source: &str, expected: &str, supplied: &str) -> String {\n    // A whole logical-table view is selected only after its actual current physical, compiled and archive channels have authenticated. Preserve the complete historical stage fallback below for every other input.\n";
        let original = "pub(super) fn value_direct(source: &str, expected: &str, supplied: &str) -> String {\n    // A whole logical-table view is selected only after its actual current\n    // physical, compiled and archive channels have authenticated. Preserve the\n    // complete historical stage fallback below for every other input.\n";
        assert_eq!(admitted_root.matches(merged).count(), 1);
        let admitted_root = admitted_root.replacen(merged, original, 1);
        let root = admitted_root.as_str();
        let child = admitted_child.as_str();
        // Undo only this later browser source scope before all original inverses.
        let mut browser_root = root.to_owned();
        let mut browser_child = child.to_owned();
        for source in [
            "tests/wasm/scalar_browser_ci_contract/historical_main_c11.rs",
            "tests/wasm/scalar_browser_ci_contract/before_524.rs",
            "tests/wasm/scalar_browser_ci_contract/before_671.rs",
            "tests/wasm/scalar_browser_ci_contract/before_7d34.rs",
            "tests/wasm/scalar_browser_ci_contract/before_21abc.rs",
            "tests/wasm/scalar_browser_ci_contract/historical_main_7f8.rs",
            "tests/wasm/scalar_browser_ci_contract/browser_custody.rs",
            "tests/wasm/scalar_browser_ci_contract/before_3e66.rs",
            "tests/wasm/scalar_browser_ci_contract/incoming_main3e66.rs",
        ] {
            let declaration = format!("    {source:?},\n");
            assert_eq!(browser_root.matches(&declaration).count(), 1);
            browser_root = browser_root.replacen(&declaration, "", 1);
        }
        for source in [
            "tests/wasm/scalar_browser_ci_contract/historical_main_c11.rs",
            "tests/wasm/scalar_browser_ci_contract/before_524.rs",
            "tests/wasm/scalar_browser_ci_contract/before_671.rs",
            "tests/wasm/scalar_browser_ci_contract/before_7d34.rs",
            "tests/wasm/scalar_browser_ci_contract/before_21abc.rs",
        ] {
            let arm = format!("        {source:?} => Some(include_str!(\"../../{source}\")),\n");
            assert_eq!(browser_child.matches(&arm).count(), 1);
            browser_child = browser_child.replacen(&arm, "", 1);
        }
        let shared_canonical = "use fragments_v3::canonical;\n";
        assert_eq!(browser_root.matches(shared_canonical).count(), 1);
        let canonical_child = include_str!("main_e177_overlay_fragments_v3.rs");
        let canonical_child = super::super::fragments_v3::v13_before(
            "src/source_reflows_merge/main_e177_overlay_fragments_v3.rs", canonical_child,
        );
        let canonical_body = canonical_child.split_once("// Shared canonical metadata encoding; no source acquisition or authority.\n").unwrap().1;
        assert_eq!(canonical_body.matches("pub(super) fn canonical(").count(), 1);
        browser_root = browser_root.replacen(shared_canonical, &canonical_body.replacen("pub(super) fn canonical(", "fn canonical(", 1), 1);
        let v10_dispatch = "    if page[\"schema\"] == fragments_v3::SCHEMA_V10 {\n        let expanded = fragments_v3::expand_v10(&before, policy, page)?;\n        compose_values_v2(before, policy, &expanded)\n    } else if page[\"schema\"] == fragments_v3::SCHEMA_V9 {";
        assert_eq!(browser_root.matches(v10_dispatch).count(), 1);
        browser_root = browser_root.replacen(v10_dispatch, "    if page[\"schema\"] == fragments_v3::SCHEMA_V9 {", 1);
        for (after, before) in [
            ("//! One fixed metadata overlay. Current source admission follows composition.\nuse super::{digest, held_4a6eb, SOURCE_CAP};\nuse serde_json::Value;\nuse std::collections::{BTreeMap, BTreeSet};\nuse std::path::Path;\nuse std::sync::{Arc, Mutex};\nconst PAGE_CAP: usize = 262144;\nconst REPLACEMENTS: &[&str] = &[\n    \"CHANGELOG.md\",\n    \"crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs\",\n    \"docs/ARCHITECTURE.md\",\n    \"docs/SUMMARY.md\",\n    \"src/cleanup_plan/replay.rs\",\n    \"src/cleanup_plan/replay/branch_clone/direct.rs\",\n    \"src/cleanup_plan/replay/branch_clone/direct/before.rs\",\n    \"src/cleanup_plan/replay/branch_clone/direct/tests.rs\",\n    \"src/cleanup_plan/replay/expression_facts/receiving/directory.rs\",\n    \"src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix.rs\",\n    \"src/cleanup_plan/replay/factored/canonical_shared/before_selected_labels.rs\",\n    \"src/cleanup_plan/replay/ordinary_plan_history/comparison_owner.rs\",\n    \"src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests.rs\",\n    \"src/cleanup_plan/replay/skeleton_work.rs\",\n    \"src/source_reflows.rs\",\n    \"src/source_reflows/main_f247_controllers.rs\",\n    \"src/source_reflows_merge.rs\",\n    \"src/source_reflows_merge/main67107_inputs.rs\",\n    \"src/source_reflows_merge/main99f_inputs.rs\",\n", "//! One fixed metadata overlay. Current source admission follows composition.\nuse super::{digest, held_4a6eb, SOURCE_CAP};\nuse serde_json::Value;\nuse std::collections::{BTreeMap, BTreeSet};\nuse std::path::Path;\nuse std::sync::{Arc, Mutex};\nconst PAGE_CAP: usize = 262144;\n// Bound from the final original-parser source union; this draft grants no paths.\nconst REPLACEMENTS: &[&str] = &[\n    \"CHANGELOG.md\",\n    \"crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs\",\n    \"docs/ARCHITECTURE.md\",\n    \"docs/SUMMARY.md\",\n    \"src/cleanup_plan/replay.rs\",\n    \"src/cleanup_plan/replay/branch_clone/direct.rs\",\n    \"src/cleanup_plan/replay/branch_clone/direct/before.rs\",\n    \"src/cleanup_plan/replay/branch_clone/direct/tests.rs\",\n    \"src/cleanup_plan/replay/expression_facts/receiving/directory.rs\",\n    \"src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix.rs\",\n    \"src/cleanup_plan/replay/factored/canonical_shared/before_selected_labels.rs\",\n    \"src/cleanup_plan/replay/ordinary_plan_history/comparison_owner.rs\",\n    \"src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests.rs\",\n    \"src/cleanup_plan/replay/skeleton_work.rs\",\n    \"src/source_reflows.rs\",\n    \"src/source_reflows/main_f247_controllers.rs\",\n    \"src/source_reflows_merge.rs\",\n    \"src/source_reflows_merge/main67107_inputs.rs\",\n    \"src/source_reflows_merge/main99f_inputs.rs\",\n"),
            ("fn normal(path: &str) -> bool {\n    !path.is_empty()\n        && !path.contains(['\\\\', ':', '\\0'])\n        && path\n            .split('/')\n            .all(|p| !p.is_empty() && p != \".\" && p != \"..\")\n}\nfn control(path: &str) -> bool {\n    [\n        POLICY_PATH,\n        PAGE_PATH,\n        EXTENSION_PATH,\n        \"src/source_reflows_merge/main_e177_final_inputs.rs\",\n        \"src/source_reflows_merge/main_e177_overlay_v2.rs\",\n        \"docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py\",\n        \"docs/evidence/main-e177-final-source-binding-20261017/guard_payload.py\",\n        \"docs/evidence/merge-2e92-browser-20261018/guard-config.json\",\n    ]\n    .contains(&path)\n}\nfn canonical(value: &Value) -> Vec<u8> {\n    fn write(v: &Value, out: &mut String) {\n        match v {\n            Value::Object(map) => {\n                out.push('{');\n                for (i, (key, value)) in map.iter().collect::<BTreeMap<_, _>>().iter().enumerate() {\n                    if i != 0 {\n                        out.push(',');\n                    }\n                    out.push_str(&serde_json::to_string(key).unwrap());\n                    out.push(':');\n                    write(value, out);\n                }\n                out.push('}');\n            }\n            Value::Array(array) => {\n                out.push('[');\n                for (i, value) in array.iter().enumerate() {\n                    if i != 0 {\n                        out.push(',');\n                    }\n                    write(value, out);\n                }\n                out.push(']');\n            }\n            _ => out.push_str(&serde_json::to_string(v).unwrap()),\n        }\n    }\n    let mut result = String::new();\n    write(value, &mut result);\n    result.push('\\n');\n    result.into_bytes()\n}\nfn json(raw: &str) -> Result<Value, &'static str> {\n    let value: Value = serde_json::from_str(raw).map_err(|_| \"JSON syntax\")?;\n    require!(\n        canonical(&value) == raw.as_bytes(),\n        \"noncanonical or duplicate JSON\"\n    );\n    Ok(value)\n}\nfn pin(value: &Value) -> Result<(usize, &str, &str), &'static str> {\n    let length =\n        usize::try_from(value[\"length\"].as_u64().ok_or(\"length\")?).map_err(|_| \"length\")?;\n    let sha = value[\"sha256\"].as_str().ok_or(\"digest\")?;\n    let path = value[\"archive\"].as_str().ok_or(\"archive\")?;\n    require!(\n        length <= SOURCE_CAP && normal(path),\n        \"source bound or archive path\"\n    );\n    require!(\n        sha.len() == 64\n            && sha\n                .bytes()\n                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),\n", "fn normal(path: &str) -> bool {\n    !path.is_empty()\n        && !path.contains(['\\\\', ':', '\\0'])\n        && path\n            .split('/')\n            .all(|p| !p.is_empty() && p != \".\" && p != \"..\")\n}\nfn control(path: &str) -> bool {\n    [\n        POLICY_PATH,\n        PAGE_PATH,\n        EXTENSION_PATH,\n        \"src/source_reflows_merge/main_e177_final_inputs.rs\",\n        \"src/source_reflows_merge/main_e177_overlay_v2.rs\",\n        \"docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py\",\n        \"docs/evidence/main-e177-final-source-binding-20261017/guard_payload.py\",\n        \"docs/evidence/merge-2e92-browser-20261018/guard-config.json\",\n    ]\n    .contains(&path)\n}\n// Canonical object keys only; arrays (including every inventory) keep their order.\nfn canonical(value: &Value) -> Vec<u8> {\n    fn write(v: &Value, out: &mut String) {\n        match v {\n            Value::Object(map) => {\n                out.push('{');\n                for (i, (key, value)) in map.iter().collect::<BTreeMap<_, _>>().iter().enumerate() {\n                    if i != 0 {\n                        out.push(',');\n                    }\n                    out.push_str(&serde_json::to_string(key).unwrap());\n                    out.push(':');\n                    write(value, out);\n                }\n                out.push('}');\n            }\n            Value::Array(array) => {\n                out.push('[');\n                for (i, value) in array.iter().enumerate() {\n                    if i != 0 {\n                        out.push(',');\n                    }\n                    write(value, out);\n                }\n                out.push(']');\n            }\n            _ => out.push_str(&serde_json::to_string(v).unwrap()),\n        }\n    }\n    let mut result = String::new();\n    write(value, &mut result);\n    result.push('\\n');\n    result.into_bytes()\n}\nfn json(raw: &str) -> Result<Value, &'static str> {\n    let value: Value = serde_json::from_str(raw).map_err(|_| \"JSON syntax\")?;\n    // This also rejects duplicate keys, alternate escaping, trailing bytes and alternate whitespace: none can reserialize to these exact complete bytes.\n    require!(\n        canonical(&value) == raw.as_bytes(),\n        \"noncanonical or duplicate JSON\"\n    );\n    Ok(value)\n}\nfn pin(value: &Value) -> Result<(usize, &str, &str), &'static str> {\n    let length =\n        usize::try_from(value[\"length\"].as_u64().ok_or(\"length\")?).map_err(|_| \"length\")?;\n    let sha = value[\"sha256\"].as_str().ok_or(\"digest\")?;\n    let path = value[\"archive\"].as_str().ok_or(\"archive\")?;\n    require!(\n        length <= SOURCE_CAP && normal(path),\n        \"source bound or archive path\"\n    );\n    require!(\n        sha.len() == 64\n            && sha\n                .bytes()\n                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),\n"),
            ("        length,\n        sha,\n    )\n}\nfn page_bytes(\n    compiled: &str,\n    physical: &str,\n    length: usize,\n    sha: &str,\n) -> Result<(), &'static str> {\n    require!(\n        length <= PAGE_CAP && physical.len() == length,\n        \"page bound or length\"\n    );\n    require!(\n        compiled == physical && digest(physical.as_bytes()) == sha,\n        \"page compiled/physical/digest\"\n    );\n    Ok(())\n}\nstruct PageData {\n    page: Option<(String, Arc<Value>)>,\n}\nimpl PageData {\n    fn parse(&mut self, raw: &str) -> Result<Arc<Value>, &'static str> {\n        require!(raw.len() <= PAGE_CAP, \"page parse bound\");\n        if let Some((bytes, parsed)) = &self.page {\n            if bytes == raw {\n                require!(\n                    canonical(parsed) == raw.as_bytes(),\n                    \"noncanonical or duplicate JSON\"\n                );\n                return Ok(Arc::clone(parsed));\n            }\n        }\n        let parsed = Arc::new(json(raw)?);\n        self.page = Some((raw.to_owned(), Arc::clone(&parsed)));\n        Ok(parsed)\n    }\n}\nfn page_data(raw: &str) -> Result<Arc<Value>, &'static str> {\n    static DATA: Mutex<PageData> = Mutex::new(PageData { page: None });\n    DATA.lock().map_err(|_| \"page parse data lock\")?.parse(raw)\n}\nfn load() -> Result<(Value, Arc<Value>), &'static str> {\n    let root = Path::new(env!(\"CARGO_MANIFEST_DIR\"));\n    let compiled =\n        include_str!(\"../../docs/evidence/finite-source-overlay-v2-20261018/policy.json\");\n    require!(POLICY_LENGTH <= PAGE_CAP, \"policy bound\");\n    let raw = held_4a6eb(&root.join(POLICY_PATH), POLICY_LENGTH, POLICY_SHA256)?;\n    require!(raw == compiled, \"policy compiled/physical\");\n    let policy = json(&raw)?;\n    require!(\n        keys(\n            &policy,\n", "        length,\n        sha,\n    )\n}\nfn page_bytes(\n    compiled: &str,\n    physical: &str,\n    length: usize,\n    sha: &str,\n) -> Result<(), &'static str> {\n    require!(\n        length <= PAGE_CAP && physical.len() == length,\n        \"page bound or length\"\n    );\n    require!(\n        compiled == physical && digest(physical.as_bytes()) == sha,\n        \"page compiled/physical/digest\"\n    );\n    Ok(())\n}\n// One bounded immutable JSON DATA object, never a cached admission verdict. Callers still acquire fresh regular handles and authenticate every live byte before arriving here. Even hits repeat canonical equality on those new bytes.\nstruct PageData {\n    page: Option<(String, Arc<Value>)>,\n}\nimpl PageData {\n    fn parse(&mut self, raw: &str) -> Result<Arc<Value>, &'static str> {\n        require!(raw.len() <= PAGE_CAP, \"page parse bound\");\n        if let Some((bytes, parsed)) = &self.page {\n            if bytes == raw {\n                require!(\n                    canonical(parsed) == raw.as_bytes(),\n                    \"noncanonical or duplicate JSON\"\n                );\n                return Ok(Arc::clone(parsed));\n            }\n        }\n        // Preserve the original complete parser and duplicate/canonical check. Failed or changed input can never receive the prior parsed object.\n        let parsed = Arc::new(json(raw)?);\n        self.page = Some((raw.to_owned(), Arc::clone(&parsed)));\n        Ok(parsed)\n    }\n}\nfn page_data(raw: &str) -> Result<Arc<Value>, &'static str> {\n    static DATA: Mutex<PageData> = Mutex::new(PageData { page: None });\n    DATA.lock().map_err(|_| \"page parse data lock\")?.parse(raw)\n}\nfn load() -> Result<(Value, Arc<Value>), &'static str> {\n    let root = Path::new(env!(\"CARGO_MANIFEST_DIR\"));\n    let compiled =\n        include_str!(\"../../docs/evidence/finite-source-overlay-v2-20261018/policy.json\");\n    require!(POLICY_LENGTH <= PAGE_CAP, \"policy bound\");\n    let raw = held_4a6eb(&root.join(POLICY_PATH), POLICY_LENGTH, POLICY_SHA256)?;\n    require!(raw == compiled, \"policy compiled/physical\");\n    let policy = json(&raw)?;\n    require!(\n        keys(\n            &policy,\n"),
            ("    require!(policy[\"predecessor\"][\"rows\"] == 519, \"predecessor rows\");\n    require!(\n        policy[\"predecessor\"][\"extension\"]\n            == serde_json::json!({\"path\":EXTENSION_PATH,\"length\":EXTENSION_LENGTH,\"sha256\":EXTENSION_SHA256}),\n        \"predecessor extension\"\n    );\n    require!(\n        policy[\"replacements\"] == serde_json::json!(REPLACEMENTS),\n        \"literal replacement scope\"\n    );\n    require!(\n        policy[\"additions\"]\n            .as_array()\n            .ok_or(\"additions\")?\n            .iter()\n            .map(|r| r[\"source\"].as_str())\n            .collect::<Vec<_>>()\n            == ADDITIONS.iter().map(|p| Some(*p)).collect::<Vec<_>>(),\n        \"literal addition scope\"\n    );\n    let extension = include_str!(\"../../docs/evidence/fmt-local-441-20261018/extension.json\");\n    require!(\n        held_4a6eb(\n            &root.join(EXTENSION_PATH),\n            EXTENSION_LENGTH,\n            EXTENSION_SHA256\n        )? == extension,\n        \"predecessor physical/compiled\"\n    );\n    let length = usize::try_from(policy[\"page\"][\"length\"].as_u64().ok_or(\"page length\")?)\n        .map_err(|_| \"page length\")?;\n    let sha = policy[\"page\"][\"sha256\"].as_str().ok_or(\"page digest\")?;\n    require!(length <= PAGE_CAP, \"page bound\");\n    let physical = held_4a6eb(&root.join(PAGE_PATH), length, sha)?;\n    let compiled = include_str!(\"../../docs/evidence/finite-source-overlay-v2-20261018/page.json\");\n    page_bytes(compiled, &physical, length, sha)?;\n    Ok((policy, page_data(&physical)?))\n}\n#[cfg(test)]\nfn load_before_page_data() -> Result<(Value, Value), &'static str> {\n", "    require!(policy[\"predecessor\"][\"rows\"] == 519, \"predecessor rows\");\n    require!(\n        policy[\"predecessor\"][\"extension\"]\n            == serde_json::json!({\"path\":EXTENSION_PATH,\"length\":EXTENSION_LENGTH,\"sha256\":EXTENSION_SHA256}),\n        \"predecessor extension\"\n    );\n    require!(\n        policy[\"replacements\"] == serde_json::json!(REPLACEMENTS),\n        \"literal replacement scope\"\n    );\n    require!(\n        policy[\"additions\"]\n            .as_array()\n            .ok_or(\"additions\")?\n            .iter()\n            .map(|r| r[\"source\"].as_str())\n            .collect::<Vec<_>>()\n            == ADDITIONS.iter().map(|p| Some(*p)).collect::<Vec<_>>(),\n        \"literal addition scope\"\n    );\n    // Original metadata is read as metadata. It is never substituted for a changed physical Rust source, and it retains its original individual cap.\n    let extension = include_str!(\"../../docs/evidence/fmt-local-441-20261018/extension.json\");\n    require!(\n        held_4a6eb(\n            &root.join(EXTENSION_PATH),\n            EXTENSION_LENGTH,\n            EXTENSION_SHA256\n        )? == extension,\n        \"predecessor physical/compiled\"\n    );\n    let length = usize::try_from(policy[\"page\"][\"length\"].as_u64().ok_or(\"page length\")?)\n        .map_err(|_| \"page length\")?;\n    let sha = policy[\"page\"][\"sha256\"].as_str().ok_or(\"page digest\")?;\n    require!(length <= PAGE_CAP, \"page bound\");\n    let physical = held_4a6eb(&root.join(PAGE_PATH), length, sha)?;\n    let compiled = include_str!(\"../../docs/evidence/finite-source-overlay-v2-20261018/page.json\");\n    page_bytes(compiled, &physical, length, sha)?;\n    Ok((policy, page_data(&physical)?))\n}\n#[cfg(test)]\nfn load_before_page_data() -> Result<(Value, Value), &'static str> {\n"),
            ("    require!(policy[\"predecessor\"][\"rows\"] == 519, \"predecessor rows\");\n    require!(\n        policy[\"predecessor\"][\"extension\"]\n            == serde_json::json!({\"path\":EXTENSION_PATH,\"length\":EXTENSION_LENGTH,\"sha256\":EXTENSION_SHA256}),\n        \"predecessor extension\"\n    );\n    require!(\n        policy[\"replacements\"] == serde_json::json!(REPLACEMENTS),\n        \"literal replacement scope\"\n    );\n    require!(\n        policy[\"additions\"]\n            .as_array()\n            .ok_or(\"additions\")?\n            .iter()\n            .map(|r| r[\"source\"].as_str())\n            .collect::<Vec<_>>()\n            == ADDITIONS.iter().map(|p| Some(*p)).collect::<Vec<_>>(),\n        \"literal addition scope\"\n    );\n    let extension = include_str!(\"../../docs/evidence/fmt-local-441-20261018/extension.json\");\n    require!(\n        held_4a6eb(\n            &root.join(EXTENSION_PATH),\n            EXTENSION_LENGTH,\n            EXTENSION_SHA256\n        )? == extension,\n        \"predecessor physical/compiled\"\n    );\n    let length = usize::try_from(policy[\"page\"][\"length\"].as_u64().ok_or(\"page length\")?)\n        .map_err(|_| \"page length\")?;\n    let sha = policy[\"page\"][\"sha256\"].as_str().ok_or(\"page digest\")?;\n    require!(length <= PAGE_CAP, \"page bound\");\n    let physical = held_4a6eb(&root.join(PAGE_PATH), length, sha)?;\n    let compiled = include_str!(\"../../docs/evidence/finite-source-overlay-v2-20261018/page.json\");\n    page_bytes(compiled, &physical, length, sha)?;\n    Ok((policy, json(&physical)?))\n}\nfn edges(packet: &Value) -> Result<Vec<(String, String)>, &'static str> {\n    let rows = packet[\"rows\"].as_array().ok_or(\"rows\")?;\n", "    require!(policy[\"predecessor\"][\"rows\"] == 519, \"predecessor rows\");\n    require!(\n        policy[\"predecessor\"][\"extension\"]\n            == serde_json::json!({\"path\":EXTENSION_PATH,\"length\":EXTENSION_LENGTH,\"sha256\":EXTENSION_SHA256}),\n        \"predecessor extension\"\n    );\n    require!(\n        policy[\"replacements\"] == serde_json::json!(REPLACEMENTS),\n        \"literal replacement scope\"\n    );\n    require!(\n        policy[\"additions\"]\n            .as_array()\n            .ok_or(\"additions\")?\n            .iter()\n            .map(|r| r[\"source\"].as_str())\n            .collect::<Vec<_>>()\n            == ADDITIONS.iter().map(|p| Some(*p)).collect::<Vec<_>>(),\n        \"literal addition scope\"\n    );\n    // Original metadata is read as metadata. It is never substituted for a\n    // changed physical Rust source, and it retains its original individual cap.\n    let extension = include_str!(\"../../docs/evidence/fmt-local-441-20261018/extension.json\");\n    require!(\n        held_4a6eb(\n            &root.join(EXTENSION_PATH),\n            EXTENSION_LENGTH,\n            EXTENSION_SHA256\n        )? == extension,\n        \"predecessor physical/compiled\"\n    );\n    let length = usize::try_from(policy[\"page\"][\"length\"].as_u64().ok_or(\"page length\")?)\n        .map_err(|_| \"page length\")?;\n    let sha = policy[\"page\"][\"sha256\"].as_str().ok_or(\"page digest\")?;\n    require!(length <= PAGE_CAP, \"page bound\");\n    let physical = held_4a6eb(&root.join(PAGE_PATH), length, sha)?;\n    let compiled = include_str!(\"../../docs/evidence/finite-source-overlay-v2-20261018/page.json\");\n    page_bytes(compiled, &physical, length, sha)?;\n    Ok((policy, json(&physical)?))\n}\nfn edges(packet: &Value) -> Result<Vec<(String, String)>, &'static str> {\n    let rows = packet[\"rows\"].as_array().ok_or(\"rows\")?;\n"),
            ("            let (length, sha, _) = pin(&row[\"current\"])?;\n            held_4a6eb(\n                &Path::new(env!(\"CARGO_MANIFEST_DIR\"))\n                    .join(row[\"source\"].as_str().ok_or(\"source\")?),\n                length,\n                sha,\n            )\n        },\n        &super::compiled,\n        &image,\n    )\n}\nfn affected_row(source: &str) -> Option<Value> {\n    if !REPLACEMENTS.contains(&source) && !ADDITIONS.contains(&source) {\n        return None;\n    }\n    let (policy, page) = load().unwrap();\n    if !affected(&policy, source) {\n        return None;\n    }\n    let packet = compose_values(super::packet_before_overlay(), &policy, &page).unwrap();\n    Some(\n        packet[\"rows\"]\n            .as_array()\n            .unwrap()\n            .iter()\n            .find(|r| r[\"source\"] == source)\n            .unwrap()\n            .clone(),\n    )\n}\npub(super) fn held_source(\n    path: &Path,\n    length: usize,\n    expected: &str,\n) -> Option<Result<String, &'static str>> {\n    let root = Path::new(env!(\"CARGO_MANIFEST_DIR\"));\n    let relative = path.strip_prefix(root).ok()?.to_str()?;\n    if !normal(relative) {\n        return None;\n", "            let (length, sha, _) = pin(&row[\"current\"])?;\n            held_4a6eb(\n                &Path::new(env!(\"CARGO_MANIFEST_DIR\"))\n                    .join(row[\"source\"].as_str().ok_or(\"source\")?),\n                length,\n                sha,\n            )\n        },\n        &super::compiled,\n        &image,\n    )\n}\nfn affected_row(source: &str) -> Option<Value> {\n    if !REPLACEMENTS.contains(&source) && !ADDITIONS.contains(&source) {\n        return None;\n    }\n    let (policy, page) = load().unwrap();\n    if !affected(&policy, source) {\n        return None;\n    }\n    // Metadata replay precedes current validation. No old physical source is read to establish the current source; only the composed row is then admitted.\n    let packet = compose_values(super::packet_before_overlay(), &policy, &page).unwrap();\n    Some(\n        packet[\"rows\"]\n            .as_array()\n            .unwrap()\n            .iter()\n            .find(|r| r[\"source\"] == source)\n            .unwrap()\n            .clone(),\n    )\n}\npub(super) fn held_source(\n    path: &Path,\n    length: usize,\n    expected: &str,\n) -> Option<Result<String, &'static str>> {\n    let root = Path::new(env!(\"CARGO_MANIFEST_DIR\"));\n    let relative = path.strip_prefix(root).ok()?.to_str()?;\n    if !normal(relative) {\n        return None;\n"),
        ] {
            assert_eq!(browser_root.matches(after).count(), 1);
            browser_root = browser_root.replacen(after, before, 1);
        }
        let root = browser_root.as_str();
        let child = browser_child.as_str();
        // Undo the single new literal admission before every prior full-source inverse.
        let source = "src/cleanup_plan/replay/expression_facts.rs";
        let declaration = format!("    {source:?},\n");
        assert_eq!(root.matches(&declaration).count(), 1);
        let expanded_root = root.replacen(&declaration, "", 1);
        let arm = format!("        {source:?} => Some(include_str!(\"../../{source}\")),\n");
        assert_eq!(child.matches(&arm).count(), 1);
        let expanded_child = child.replacen(&arm, "", 1);
        let merged = "    // Metadata replay precedes current validation. No old physical source is read to establish the current source; only the composed row is then admitted.\n";
        let original = "    // Metadata replay precedes current validation. No old physical source is read\n    // to establish the current source; only the composed row is then admitted.\n";
        assert_eq!(expanded_root.matches(merged).count(), 1);
        let expanded_root = expanded_root.replacen(merged, original, 1);
        let root = expanded_root.as_str();
        let child = expanded_child.as_str();
        // Undo only this exact later literal scope and line-budget projection.
        // Every original fixed span and complete predecessor assertion follows.
        let mut checkpoint_root = root.to_owned();
        let workspace = "tests/workspace.rs";
        let declaration = format!("    {workspace:?},\n");
        assert_eq!(checkpoint_root.matches(&declaration).count(), 1);
        checkpoint_root = checkpoint_root.replacen(&declaration, "", 1);
        let arm = format!("        {workspace:?} => Some(include_str!(\"../../{workspace}\")),\n");
        assert_eq!(child.matches(&arm).count(), 1);
        let checkpoint_child = child.replacen(&arm, "", 1);
        let child = checkpoint_child.as_str();
        let merged = "    // Original metadata is read as metadata. It is never substituted for a changed physical Rust source, and it retains its original individual cap.\n";
        let original = "    // Original metadata is read as metadata. It is never substituted for a\n    // changed physical Rust source, and it retains its original individual cap.\n";
        assert_eq!(checkpoint_root.matches(merged).count(), 1);
        checkpoint_root = checkpoint_root.replacen(merged, original, 1);

        for source in [
            "src/cleanup_plan/replay/branch_clone.rs",
            "src/source_reflows_merge/main_e177_inputs.rs",
            "src/source_reflows_merge/main_c883_inputs.rs",
            "src/source_reflows/main043/audit_3.rs",
        ] {
            let literal = format!("    {source:?},\n");
            assert_eq!(checkpoint_root.matches(&literal).count(), 1);
            checkpoint_root = checkpoint_root.replacen(&literal, "", 1);
        }
        for (current, previous) in [
            ("        // Preserve the original complete parser and duplicate/canonical check. Failed or changed input can never receive the prior parsed object.\n", "        // Preserve the original complete parser and duplicate/canonical check.\n        // Failed or changed input can never receive the prior parsed object.\n"),
            ("    // This also rejects duplicate keys, alternate escaping, trailing bytes and alternate whitespace: none can reserialize to these exact complete bytes.\n", "    // This also rejects duplicate keys, alternate escaping, trailing bytes and\n    // alternate whitespace: none can reserialize to these exact complete bytes.\n"),
            ("// One bounded immutable JSON DATA object, never a cached admission verdict. Callers still acquire fresh regular handles and authenticate every live byte before arriving here. Even hits repeat canonical equality on those new bytes.\n", "// One bounded immutable JSON DATA object, never a cached admission verdict.\n// Callers still acquire fresh regular handles and authenticate every live byte\n// before arriving here. Even hits repeat canonical equality on those new bytes.\n"),
        ] {
            assert_eq!(checkpoint_root.matches(current).count(), 1);
            checkpoint_root = checkpoint_root.replacen(current, previous, 1);
        }
        let root = checkpoint_root.as_str();
        // Authenticate the complete actual root above, then undo only the
        // formatter-compatible lookup spelling before the fixed-span inverse.
        let formatted = "    let rows = authenticated[\"rows\"].as_array().unwrap();\n    let row = rows.iter().find(|row| row[\"source\"] == relative).unwrap();";
        let previous = "    let row = authenticated[\"rows\"].as_array().unwrap().iter()\n        .find(|row| row[\"source\"] == relative).unwrap();";
        assert_eq!(root.matches(formatted).count(), 1);
        assert_eq!(root.matches(previous).count(), 0);
        let preformat_root = root.replacen(formatted, previous, 1);
        let root = preformat_root.as_str();
        // Keep the complete moved reader verbatim and restore its original
        // position before every unchanged full predecessor assertion.
        let reader = "pub(super) fn value_direct(source: &str, expected: &str, supplied: &str) -> String {\n    // A whole logical-table view is selected only after its actual current\n    // physical, compiled and archive channels have authenticated. Preserve the\n    // complete historical stage fallback below for every other input.\n    let direct = super::value_direct_4a6eb(source, expected, supplied);\n    if digest(direct.as_bytes()) == expected {\n        return direct;\n    }\n\n    if let Some(value) = super::clippy_stage::requested(source, expected, supplied) {\n        let _ = super::packet();\n        return value;\n    }\n    let normalized = super::clippy_stage::normalize(source, supplied);\n    let result = super::value_direct_4a6eb(source, expected, normalized.as_ref());\n    if normalized.as_ref() != supplied\n        && result == normalized.as_ref()\n        && digest(result.as_bytes()) != expected\n    {\n        supplied.to_owned()\n    } else {\n        result\n    }\n}\n";
        assert_eq!(root.matches(reader).count(), 1);
        let (head, tail) = root.split_once(reader).unwrap();
        let ordered_root = format!("{head}{tail}{reader}");
        let mut ordered_root = ordered_root;
        for (after, before) in [
            ("    compiled_sources::compiled(source)\n}\n#[cfg(test)]\nmod tests", "    compiled_sources::compiled(source)\n}\n\n#[cfg(test)]\nmod tests"),
            ("use std::sync::{Arc, Mutex};\nconst PAGE_CAP:", "use std::sync::{Arc, Mutex};\n\nconst PAGE_CAP:"),
        ] {
            assert_eq!(ordered_root.matches(after).count(), 1);
            ordered_root = ordered_root.replacen(after, before, 1);
        }
        let mut joint_root = ordered_root.to_owned();
        // The two branches restore these same gaps independently. Re-enter
        // main's exact compact stage before its unchanged fixed-span checks.
        for (expanded, packed) in [
            ("    compiled_sources::compiled(source)\n}\n#[cfg(test)]\nmod tests", "    compiled_sources::compiled(source)\n}\n\n#[cfg(test)]\nmod tests"),
            ("use std::sync::{Arc, Mutex};\nconst PAGE_CAP:", "use std::sync::{Arc, Mutex};\n\nconst PAGE_CAP:"),
        ] {
            assert_eq!(joint_root.matches(packed).count(), 1);
            joint_root = joint_root.replacen(packed, expanded, 1);
        }
        let root = joint_root.as_str();
        let before = before();
        let old_additions = array(before, "ADDITIONS");
        let additions = array(root, "ADDITIONS");
        assert!(additions.starts_with(&old_additions));
        let old_replacements = array(before, "REPLACEMENTS");
        let replacements = array(root, "REPLACEMENTS");
        let mut old = old_replacements.iter();
        let mut next = old.next();
        for entry in &replacements {
            if next == Some(entry) {
                next = old.next();
            }
        }
        assert!(next.is_none(), "old replacement order or member changed");
        let mut actual_lookup = lookup(child).to_owned();
        for source in &additions[old_additions.len()..] {
            let arm = format!("        {source:?} => Some(include_str!(\"../../{source}\")),\n");
            assert_eq!(actual_lookup.matches(&arm).count(), 1);
            actual_lookup = actual_lookup.replacen(&arm, "", 1);
        }
        assert_eq!(actual_lookup, lookup(before), "ENTIRE old lookup body");
        // The actual union inventories and ENTIRE lookup above precede this
        // exact main-parent metadata projection. These literal declarations
        // come from b60370fe3550f485707927eddf2ad5e266872a39's original
        // restore_whole_before input stage immediately before its fixed gaps.
        let mut historical_root = root.to_owned();
        for (name, archived) in [
            ("REPLACEMENTS", r#####"const REPLACEMENTS: &[&str] = &[
    "CHANGELOG.md",
    "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs",
    "docs/ARCHITECTURE.md",
    "docs/SUMMARY.md",
    "src/cleanup_plan/replay.rs",
    "src/cleanup_plan/replay/branch_clone/direct.rs",
    "src/cleanup_plan/replay/branch_clone/direct/before.rs",
    "src/cleanup_plan/replay/branch_clone/direct/tests.rs",
    "src/cleanup_plan/replay/expression_facts/receiving/directory.rs",
    "src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix.rs",
    "src/cleanup_plan/replay/factored/canonical_shared/before_selected_labels.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests.rs",
    "src/cleanup_plan/replay/skeleton_work.rs",
    "src/source_reflows.rs",
    "src/source_reflows/main_f247_controllers.rs",
    "src/source_reflows_merge.rs",
    "src/source_reflows_merge/main67107_inputs.rs",
    "src/source_reflows_merge/main99f_inputs.rs",
    "src/source_reflows_merge/next_big_batch_inputs.rs",
    "tests/hir_module_boundaries.rs",
    "tests/workspace/semantic_graph.rs",
    "tests/workspace/semantic_graph/source_inventory.rs",
    "src/cleanup_plan/replay/branch_clone/tests.rs",
    "src/cleanup_plan/replay/expression_facts/receiving/retained.rs",
    "src/cleanup_plan/replay/factored/canonical_shared.rs",
    "src/cleanup_plan/replay/factored/canonical_shared/arena_tests.rs",
    "src/cleanup_plan/replay/factored/canonical_shared/before_selected_route.rs",
    "src/cleanup_plan/replay/factored/canonical_shared/borrowed_labels.rs",
    "src/cleanup_plan/replay/factored/canonical_shared/paid_many/tests.rs",
    "src/cleanup_plan/replay/factored/continuation_receiver.rs",
    "src/cleanup_plan/replay/factored/oracle_hook.rs",
    "src/source_reflows/semantic_receivers.rs",
    "src/cleanup_plan/replay/skeleton_work/enumerated_prefix/tests.rs",
    "platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs",
    "src/cleanup_plan/replay/factored.rs",
    "src/cleanup_plan/replay/factored/canonical_shared/before_lexical_successors.rs",
    "src/cleanup_plan/replay/factored/canonical_shared/lexical_successors/census.rs",
    "src/cleanup_plan/replay/ordinary_plan_history.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor.rs",
    "src/workspace_graph/expected_projection/held_projection.rs",
    "src/cleanup_plan/replay/expression_facts/arena/retained.rs",
    "src/cleanup_plan/replay/expression_facts/arena/retained/tests.rs",
    "src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_tests.rs",
    "src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/retained_lookup.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_census.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_tests.rs",
    "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity.rs",
    "src/bounded_output.rs",
    "src/cleanup/schema_summary/tests.rs",
    "src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix/tests.rs",
    "src/cleanup_plan/replay/expression_facts/receiving/regions_entry/tests.rs",
    "src/cleanup_plan/replay/factored/canonical_shared/paid_many/source.rs",
    "src/cleanup_plan/replay/factored/strategy.rs",
    "src/cleanup_plan/replay/live_oracle/source_custody/a.rs",
    "src/cleanup_plan/replay/live_oracle/source_custody/b.rs",
    "src/cleanup_plan/replay/path_join.rs",
    "src/cleanup_plan/replay/physical_budget.rs",
    "src/cleanup_plan/replay/selected_entry/tests/session_errors.rs",
    "src/cleanup_plan/replay/semantic_refusal.rs",
    "src/cleanup_plan/replay/semantic_refusal/tests.rs",
    "src/cleanup_plan/replay/skeleton_work/enumerated_prefix/before.rs",
    "src/cleanup_plan/replay/structural_control/calls.rs",
    "src/diagnostic/before_writer.rs",
    "src/hir/declaration_index/identity_payer.rs",
    "src/hir/expr_nodes.rs",
    "src/hir/ids/fixed_operation_tests.rs",
    "src/hir/lexical_exit.rs",
    "src/hir/resolve_program.rs",
    "src/hir/resolve_program_methods.rs",
    "src/hir/validation.rs",
    "src/loan_plan/paid_live.rs",
    "src/loop_calls/byte_writer/resolved.rs",
    "src/project/json_codec.rs",
    "src/source_reflows/factored_sources.rs",
    "src/source_reflows/incoming090_semantic_receivers.rs",
    "src/source_reflows/incoming21_semantic_receivers.rs",
    "src/source_reflows/incoming67107_semantic_receivers.rs",
    "src/source_reflows/main043.rs",
    "src/source_reflows/semantic_receivers_mainb80.rs",
    "src/source_reflows_merge/current344.rs",
    "src/source_reflows_merge/final_ready_inputs.rs",
    "src/source_reflows_merge/main67107_inputs_tail.rs",
    "src/source_reflows_merge/main7f8_source_inputs.rs",
    "src/source_reflows_merge/main8a_historical.rs",
    "src/source_reflows_merge/main_eb2_inputs.rs",
    "src/source_reflows_merge/ordinary_plan_inputs.rs",
    "src/source_reflows_merge/runtime_json.rs",
    "src/source_verify/declaration.rs",
    "src/workspace_graph.rs",
    "src/workspace_graph/constructor_failure/receiving/typed/tests.rs",
    "src/workspace_graph/expected_projection/borrowed_imports/typed/alias/owned_growth_tests.rs",
    "src/workspace_graph/expected_projection/type_rewrite/receiving.rs",
    "src/workspace_graph/function_alias/before_core.rs",
    "src/workspace_graph/function_alias/before_core/tests.rs",
    "src/workspace_graph/source_schema/nominal_context/main8a.rs",
    "src/workspace_graph/source_schema/static_text/tests.rs",
    "src/workspace_graph/test_replay_context.rs",
    "tests/doctor_production_provisioner_v1.rs",
    "tests/project/standard_library/application_json.rs",
    "tests/quality_routing.rs",
    "tests/support/production_source_inventory.rs",
    "tests/wasm/scalar_browser_ci_contract.rs",
    "tests/wasm/scalar_browser_ci_contract/historical_builder.rs",
    "docs/evidence/main-e177-final-source-binding-20261017/current-projection-inputs.tsv",
    "src/bounded_output/stage_debits/route_debits.rs",
    "src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified.rs",
    "src/cleanup_plan/replay/factored/canonical_shared/matched_receiving_layers/successor_targets.rs",
    "src/hir/ids/declaration_tag.rs",
    "src/hir/ids/declaration_tag/before.rs",
    "src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_cross_phase.rs",
    "src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_cross_phase_before.rs",
    "src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_cross_phase_tests.rs",
    "src/cleanup_plan/replay/inventory_coverage/paid/queries.rs",
    "src/cleanup_plan/replay/inventory_coverage/paid_before.rs",
    "src/cleanup_plan/replay/inventory_coverage/typed_tests/selected_family.rs",
    "src/cleanup_plan/replay/path_join/selected.rs",
    "src/cleanup_plan/replay/path_join/selected/joins.rs",
    "src/cleanup_plan/replay/path_join/selected/lexical_flow.rs",
    "src/cleanup_plan/replay/path_join/selected/queries.rs",
    "src/cleanup_plan/replay/path_join/selected/renewal.rs",
    "src/cleanup_plan/replay/path_join/selected/strings.rs",
    "src/cleanup_plan/replay/path_join/selected/tests/history.rs",
    "src/cleanup_plan/replay/path_join/selected/tests/lookup.rs",
    "src/cleanup_plan/replay/path_join/selected/walk.rs",
    "src/cleanup_plan/replay/schema/selected_paid.rs",
    "src/cleanup_plan/replay/selected_entry/tests/paid/selected_family.rs",
    "src/cleanup_plan/replay/structural_control/selected.rs",
    "src/cleanup_plan/replay/structural_control/selected/blocks.rs",
    "src/cleanup_plan/replay/structural_control/selected/calls.rs",
    "src/cleanup_plan/replay/structural_control/selected/helpers.rs",
    "src/cleanup_plan/replay/structural_control/selected/place_before_formals.rs",
    "src/cleanup_plan/replay/structural_control/selected/tests/boundary.rs",
    "src/cleanup_plan/replay/inventory_coverage.rs",
    "src/cleanup_plan/replay/inventory_coverage/paid.rs",
    "src/cleanup_plan/replay/lexical_replay.rs",
    "src/cleanup_plan/replay/scalar_replay.rs",
    "src/iterator_ops.rs",
];"#####),
            ("ADDITIONS", r#####"const ADDITIONS: &[&str] = &[
    "src/cleanup_plan/renewal_profile.rs",
    "src/cleanup_plan/renewal_profile/funded.rs",
    "src/cleanup_plan/renewal_profile/funded_receiving.rs",
    "src/cleanup_plan/replay/expression_facts/arena/retained/into_tests.rs",
    "src/cleanup_plan/replay/expression_facts/arena/retained_into.rs",
    "src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_tests/receiving.rs",
    "src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/retained_lookup_before_receiving.rs",
    "src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_cross_phase_before_receiving.rs",
    "src/cleanup_plan/replay/ordinary_receiving.rs",
    "src/cleanup_plan/replay/ordinary_receiving/caller_before.rs",
    "src/cleanup_plan/replay/ordinary_receiving/tests.rs",
    "src/cleanup_plan/replay/skeleton_work/dag_storage.rs",
    "src/cleanup_plan/replay/skeleton_work/receiving.rs",
    "src/hir/lexical_exit/owner_funded.rs",
    "src/hir/lexical_exit/owner_funded_receiving.rs",
    "src/literal_format.rs",
    "src/literal_format/funded.rs",
    "src/literal_format/funded_receiving.rs",
    "src/string_ops/conditions.rs",
    "src/string_ops/conditions/funded.rs",
    "src/string_ops/conditions/funded_receiving.rs",
    "src/cleanup_plan/replay/expression_facts/errors.rs",
    "docs/ORDINARY-REPLAY-RECEIVING-V1.md",
    "src/hir/ids/declaration_backing_tests.rs",
    "src/cleanup_plan/replay/factored/before_inline_singleton.rs",
    "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton.rs",
    "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/inline_exit.rs",
    "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/inline_exit/census.rs",
    "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors.rs",
    "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop.rs",
    "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/borrowed_single.rs",
    "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/terminal_leaf.rs",
    "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/tests.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_paired.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value.rs",
    "src/source_reflows_merge/runtime_json/sealed_io.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_flat_view.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_fused_flat.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/combined_pair_scope_tests.rs",
    "src/ast/program_source/function_alias/scalar_signature.rs",
    "src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_block_before.rs",
    "src/workspace_graph/constructor_failure/before_scalar_dispatch.rs",
    "src/workspace_graph/expected_projection/borrowed_imports/typed/before_scalar_signature.rs",
    "src/workspace_graph/expected_projection/borrowed_imports/typed/scalar_signature_tests.rs",
    "src/ast/program_source/function_alias/before_string.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_retained_tail.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_before_tail.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/matched_tail_tests.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value_before_tail.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/source_format.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source.rs",
    "src/source_reflows_merge/main_e177_historical_before_data.rs",
    "src/source_reflows_merge/main_e177_historical_data.rs",
    "src/source_reflows_merge/main_e177_historical_data_tests.rs",
    "src/source_reflows_merge/main_e177_overlay_fragments_v3.rs",
    "src/workspace_graph/expected_projection/borrowed_imports/typed/before_string_signature.rs",
    "src/workspace_graph/expected_projection/borrowed_imports/typed/string_signature_tests.rs",
    "src/workspace_graph/expected_projection/call_carriers.rs",
    "src/workspace_graph/expected_projection/call_carriers_historical_tests.rs",
    "src/workspace_graph/expected_projection/call_carriers_inverse.rs",
    "src/workspace_graph/expected_projection/call_carriers_original.rs",
    "src/workspace_graph/expected_projection/call_carriers_tests.rs",
    "src/ast/program_source/decl_slices.rs",
    "src/workspace_graph/expected_projection/borrowed_imports/typed.rs",
    "src/workspace_graph/expected_projection/borrowed_imports/typed/alias.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_prepared_pairing.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_before_prepared_links.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/prepared_pairing_tests.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links.rs",
    "src/workspace_graph/expected_projection/call_carriers_before_directory.rs",
    "src/workspace_graph/expected_projection/call_carriers_directory.rs",
    "src/workspace_graph/expected_projection/call_carriers_directory_tests.rs",
    "src/workspace_graph/expected_projection.rs",
    "src/workspace_graph/expected_projection/prelude_forecast_tests.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_identity_prefix.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_before_identity.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value_before_identity.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_tests.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_reachable_primitives.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_reachable_before.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/reachable_pair_scope_tests.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_event_prefix.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_shared_steps.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_shared_steps.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/main99_pair_scope_tests.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value_before_event.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_before_event.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_before_event_tests.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_event_tests.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps.rs",
    "src/cleanup_plan/replay/lexical_replay/operand_tests/source.rs",
    "src/workspace_graph/expected_projection/private_layout/capacity_tests/source.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_certified_heads.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_shared_runs.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_heads.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_heads_tests.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_before_certified.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_shared_runs.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/main7e_pair_scope_tests.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value_before_certified.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_before_certified.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps/shared_boundaries.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps/shared_boundaries/shared_runs.rs",
    "src/workspace_graph/expected_projection/private_layout/registry/scratch_tests.rs",
    "src/workspace_graph/cold_canonical/tests.rs",
    "src/workspace_graph/expected_projection/cost.rs",
    "src/bounded_output/stage_debits/route_debits/json_codec_probe.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_main001.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_main487.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_main_b692.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_pair_links.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_paired_tail.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/borrowed_reader_composition_tests.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_heads_pair_links.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_heads_paired_tail.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_main001.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_main487.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_main_b692.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_pair_links.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_paired_tail.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/main001_pair_scope_tests.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/main487_pair_scope_tests.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/main_b692_pair_scope_tests.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/main_b692_paired_tails.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_link_composition_tests.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_tail_composition_tests.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value_pair_links.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value_paired_tail.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_pair_links.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_paired_tail.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/constructor.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/fused_readers.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/json_ledger.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/paired_tails.rs",
    "src/source_reflows_merge/main_e177_overlay_compiled.rs",
    "src/source_reflows_merge/main_e177_packet_lookup.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_empty_selector.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_heads_before_empty.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/empty_selector_tests.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_before_empty.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value_before_empty.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_before_empty.rs",
    "src/test_allocation_observer/backing.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_final_affine.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_heads_before_final_affine.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/final_affine.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/final_affine_tests.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_before_final_affine.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value_before_final_affine.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_before_final_affine.rs",
    "src/bounded_output/exclusive_debits.rs",
    "src/hir/ids/lowering_identity_tests.rs",
    "src/hir/owned_collection_record/identity_before.rs",
    "src/hir/resolve_expr/sort_keys/identity_before.rs",
    "src/hir/resolve_expr/sort_keys/identity_tests.rs",
    "src/hir/resolve_for/identity_before.rs",
    "src/hir/resolve_for_own/identity_before.rs",
    "src/source_reflows_merge/main_e177_overlay_pins_v9.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/frontier_tests.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/retained_main_289/fixture_helpers.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/retained_main_289/lexical_cursor.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/retained_main_289/mod.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/retained_main_289/pair_scope_census.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/retained_main_289/pair_scope_tests.rs",
    "src/bounded_output/stage_debits/route_debits/retained_main_289.rs",
];"#####),
            ("POLICY_LENGTH", r#####"const POLICY_LENGTH: usize = 106728;"#####),
            ("POLICY_SHA256", r#####"const POLICY_SHA256: &str = "4f331f9b8e0dab9f71fa0e722354c8f1cc52fc3b8e7b9a1fac4902836a8bd3b3";"#####),
        ] {
            let at = constant(&historical_root, name);
            historical_root.replace_range(at, archived);
        }
        assert_eq!(historical_root.len(), 68651);
        assert_eq!(super::super::digest(historical_root.as_bytes()), "d1e54d6502b6ee61b2f33541de88a897340693cd5d810bffaea4fac29c2f46d1");
        let root = historical_root.as_str();
        // Restore only the four token gaps removed to keep the current module
        // within its source-size budget; every original inverse runs below.
        let mut uncompressed_root = root.to_owned();
        assert_eq!(&uncompressed_root[49557..49558], "\n");
        uncompressed_root.replace_range(49557..49558, "\n\n");
        assert_eq!(&uncompressed_root[49453..49454], "\n");
        uncompressed_root.replace_range(49453..49454, "\n\n");
        assert_eq!(&uncompressed_root[23915..23916], "\n");
        uncompressed_root.replace_range(23915..23916, "\n\n");
        assert_eq!(&uncompressed_root[239..240], "\n");
        uncompressed_root.replace_range(239..240, "\n\n");
        let root = uncompressed_root.as_str();
        assert_eq!(root.matches(GLUE).count(), 1);
        let mut restored = root.replacen(GLUE, &actual_lookup, 1);
        // The later same-invocation reader is a complete additive function.
        // Bind the whole current root/child above, then restore its exact
        // predecessor before every unchanged historical split assertion.
        let context_reader = r####"pub(super) fn held_source_with_packet(
    authenticated: &Value,
    path: &Path,
    length: usize,
    expected: &str,
) -> Option<Result<String, &'static str>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let relative = path.strip_prefix(root).ok()?.to_str()?;
    if !normal(relative) {
        return None;
    }
    // compose() authenticated policy membership against these exact constants.
    if !REPLACEMENTS.contains(&relative) && !ADDITIONS.contains(&relative) {
        return None;
    }
    let row = authenticated["rows"].as_array().unwrap().iter()
        .find(|row| row["source"] == relative).unwrap();
    Some((|| {
        let actual = current(row)?;
        if row["current"]["length"].as_u64() == Some(length as u64)
            && row["current"]["sha256"] == expected
        {
            return Ok(actual);
        }
        let prior = row["views"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["length"].as_u64() == Some(length as u64) && p["sha256"] == expected)
            .ok_or("foreign overlay tuple")?;
        image(prior)
    })())
}
"####;
        assert_eq!(restored.matches(context_reader).count(), 1);
        restored = restored.replacen(context_reader, "", 1);
        let v9_dispatch = "    if page[\"schema\"] == fragments_v3::SCHEMA_V9 {\n        let expanded = fragments_v3::expand_v9(&before, policy, page)?;\n        compose_values_v2(before, policy, &expanded)\n    } else if page[\"schema\"] == fragments_v3::SCHEMA_V8 {";
        assert_eq!(restored.matches(v9_dispatch).count(), 1);
        restored = restored.replacen(
            v9_dispatch,
            "    if page[\"schema\"] == fragments_v3::SCHEMA_V8 {",
            1,
        );
        let v8_dispatch = "    if page[\"schema\"] == fragments_v3::SCHEMA_V8 {\n        let expanded = fragments_v3::expand_v8(&before, policy, page)?;\n        compose_values_v2(before, policy, &expanded)\n    } else if page[\"schema\"] == fragments_v3::SCHEMA_V7 {";
        assert_eq!(restored.matches(v8_dispatch).count(), 1);
        restored = restored.replacen(
            v8_dispatch,
            "    if page[\"schema\"] == fragments_v3::SCHEMA_V7 {",
            1,
        );
        let v7_dispatch = "    if page[\"schema\"] == fragments_v3::SCHEMA_V7 {\n        let expanded = fragments_v3::expand_v7(&before, policy, page)?;\n        compose_values_v2(before, policy, &expanded)\n    } else if page[\"schema\"] == fragments_v3::SCHEMA_V6 {";
        assert_eq!(restored.matches(v7_dispatch).count(), 1);
        restored = restored.replacen(
            v7_dispatch,
            "    if page[\"schema\"] == fragments_v3::SCHEMA_V6 {",
            1,
        );
        // Only the existing generated literal source lists and policy pins
        // are normalized; every other predecessor byte remains compared.
        for name in [
            "REPLACEMENTS",
            "ADDITIONS",
            "POLICY_LENGTH",
            "POLICY_SHA256",
        ] {
            let at = constant(&restored, name);
            restored.replace_range(at, &before[constant(before, name)]);
        }
        assert_eq!(
            restored, before,
            "ENTIRE pre-split root and unknown source text"
        );
        restored
    }

    #[test]
    fn compiled_lookup_split_preserves_the_complete_predecessor_and_every_old_mapping() {
        assert_eq!(restore_whole_before(ROOT, CHILD), before());
        let old_lookup = lookup(before());
        let sources: Vec<_> = old_lookup
            .lines()
            .map(str::trim)
            .filter(|line| line.starts_with('"'))
            // A wrapped include_str! argument is source DATA, not a match key.
            .filter_map(|line| line.split_once(" => ").map(|(source, _)| source))
            .collect();
        assert_eq!(old_lookup.matches("_ => None").count(), 1);
        assert_eq!(
            sources.len() + 1,
            old_lookup.matches("=>").count(),
            "every old literal match arm plus its sole fallback must be covered"
        );
        for source in sources {
            let source: String = serde_json::from_str(source).unwrap();
            assert!(
                super::compiled(&source).is_some(),
                "missing old compiled source {source}"
            );
        }
    }

    #[test]
    fn compiled_lookup_split_refuses_child_parent_span_eof_and_duplicate_glue() {
        assert_eq!(restore_whole_before(ROOT, CHILD), before());
        for child in [
            format!("{CHILD} "),
            CHILD.replacen("match source", "match sourcf", 1),
            CHILD.replacen("Some(include_str!", "None/*include_str!", 1),
        ] {
            assert_ne!(child, CHILD);
            assert!(std::panic::catch_unwind(|| restore_whole_before(ROOT, &child)).is_err());
        }
        let current_glue = GLUE.replacen("\n\n", "\n", 1);
        assert_eq!(ROOT.matches(&current_glue).count(), 1);
        for root in [
            format!("{ROOT} "),
            ROOT.replacen(&current_glue, &(current_glue.to_owned() + &current_glue), 1),
            ROOT.replacen("fn keys(", "fn keyz(", 1),
        ] {
            assert_ne!(root, ROOT);
            assert!(std::panic::catch_unwind(|| restore_whole_before(&root, CHILD)).is_err());
        }
    }
    #[rustfmt::skip] #[test] fn remaining_source_view_requires_the_complete_current_compiled_channel() { use sha2::{Digest, Sha256}; let source = "src/cleanup_plan/replay/expression_facts/receiving/remaining/tests.rs"; let raw = include_str!(concat!( env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/remaining/tests.rs" )); assert_eq!(super::compiled(source), Some(raw)); let old = crate::source_reflows_merge::main_e177_final_inputs::value_direct( source, "e6d9241f9287e8e00d5c8413623383b53b46e5efbe6da50e576dcf6bbd642db4", raw, ); assert_eq!( old, include_str!(concat!( env!("CARGO_MANIFEST_DIR"), "/e/e6d9241f9287e8e00d5c8413623383b53b46e5efbe6da50e576dcf6bbd642db4" )) ); assert_eq!( format!( "{:x}", crate::digest_hex::LowerHex(Sha256::digest(old.as_bytes())) ), "e6d9241f9287e8e00d5c8413623383b53b46e5efbe6da50e576dcf6bbd642db4" ); } }

// CENSUS_LOOKUP_SPLIT_CUSTODY: additive controls; complete old parents above.
#[path = "../workspace_graph/identity_resolution/base/cache_billing/main7f8_reader.rs"] mod main48_historical_reader; #[cfg(test)] mod census_split { use sha2::{Digest, Sha256}; const ROOT: &str = include_str!("main_e177_overlay_compiled.rs"); const LOOKUP: &str = include_str!("main_e177_overlay_lookup.rs"); const BEFORE: &str = include_str!("main_e177_overlay_compiled.before.rs.txt"); const PYTHON: &str = include_str!(concat!( env!("CARGO_MANIFEST_DIR"), "/docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py" )); const CODECS: &str = include_str!(concat!( env!("CARGO_MANIFEST_DIR"), "/docs/evidence/finite-source-overlay-v2-20261018/overlay_page_codecs.py" )); const PYTHON_BEFORE: &str = include_str!(concat!( env!("CARGO_MANIFEST_DIR"), "/docs/evidence/finite-source-overlay-v2-20261018/overlay_page.before.py.txt" ));
    const GLUE: &str = r####"#[path = "main_e177_overlay_lookup.rs"]
#[rustfmt::skip]
mod lookup;
pub(super) use lookup::compiled;

"####;
    const BRIDGE: &str = r####"        let rebound = super::census_split::parent_for_current_scope(child, include_str!("main_e177_overlay_lookup.rs"));
        let child = rebound.as_str();
"####;
    const LOADER: &str = r####"# Fixed local codec source; authenticate bytes before Python evaluates them.
def _load_fixed_codecs():
 import os, stat
 from pathlib import Path
 path=Path(__file__).with_name('overlay_page_codecs.py')
 length=10905
 fd=os.open(path,os.O_RDONLY|os.O_CLOEXEC|os.O_NOFOLLOW|os.O_NONBLOCK)
 try:
  info=os.fstat(fd)
  assert stat.S_ISREG(info.st_mode) and info.st_size==length, 'codec regular fixed length'
  with os.fdopen(fd,'rb',closefd=False) as stream:raw=stream.read(length+1)
  after=os.fstat(fd);named=os.lstat(path)
  identity=lambda value:(value.st_dev,value.st_ino,value.st_mode,value.st_size,value.st_mtime_ns,value.st_ctime_ns)
  assert stat.S_ISREG(after.st_mode) and stat.S_ISREG(named.st_mode), 'codec postread regularity'
  assert identity(info)==identity(after)==identity(named), 'codec postread identity'
  assert len(raw)==length and hashlib.sha256(raw).hexdigest()=='a0f906f11a8c7205a2124e5cd9c1909fa1a53a12731a0775d3e360d844bab794', 'codec fixed bytes'
 finally:os.close(fd)
 exec(compile(raw,str(path),'exec'),globals())
_load_fixed_codecs()
del _load_fixed_codecs

"####;
    const KEYS: &[&str] = &[ "src/cleanup_plan/replay/skeleton_work/query_proof.rs", "src/cleanup_plan/replay/skeleton_work/query_proof/before.rs", "src/cleanup_plan/replay/skeleton_work/query_proof/tests.rs", "src/bounded_output/exclusive_debits/path_census.rs", "src/bounded_output/request_reservation.rs", "src/source_reflows_merge/main_e177_overlay_lookup.rs", "src/hir/inspection/paid_nul.rs", "src/hir/inspection/paid_nul/pending.rs", "src/hir/inspection/paid_nul/tests.rs", "src/hir/inspection/paid_nul/reuse_before.rs", "src/hir/inspection/paid_nul/reuse_before/pending.rs", "src/hir/inspection/paid_nul/type_scratch.rs", "src/cleanup_plan/replay/continuation_storage.rs", "src/cleanup_plan/replay/continuation_storage/tests.rs", "src/cleanup_plan/replay/continuation_storage/reached.rs", "src/cleanup_plan/replay/continuation_storage/reached_before.rs",
        "src/cleanup_plan/replay/continuation_storage/reached_tests.rs",
        "src/cleanup_plan.rs",
        "src/workspace_graph/tests.rs",
        "src/cleanup_plan/replay/expression_facts/receiving/helpers/tests.rs",
        "src/cleanup_plan/replay/expression_facts/receiving/retained/exits/tests.rs",
        "src/cleanup_plan/replay/workspace_oracle.rs",
        "src/cleanup_plan/replay/factored.rs",
        "src/cleanup_plan/replay/factored/strategy.rs",
        "src/cleanup_plan/replay/factored/paired.rs",
        "src/cleanup_plan/replay/factored/paired/tests.rs",
        "src/cleanup_plan/replay/path_join/fused_before.rs",
        "src/cleanup_plan/replay/path_join/fused_before_frontier.rs",
        "src/cleanup_plan/replay/path_join/fused_tests.rs",
        "src/cleanup_plan/replay_tests/factored.rs",
];
    const SCOPE_ADDITIONS_LINE: &str = r####"            "src/cleanup_plan/replay/skeleton_work/query_proof.rs", "src/cleanup_plan/replay/skeleton_work/query_proof/before.rs", "src/cleanup_plan/replay/skeleton_work/query_proof/tests.rs", "src/bounded_output/exclusive_debits/path_census.rs", "src/bounded_output/request_reservation.rs", "src/source_reflows_merge/main_e177_overlay_lookup.rs",
"####;
    const SCOPE_REPLACEMENTS_BEFORE: &str = r####"        for source in ["src/cleanup_plan/replay/branch_clone/inline_tests.rs", "src/cleanup_plan/replay/branch_clone/production_capacity.rs", "src/cleanup_plan/replay/branch_clone/production_capacity/tests.rs", "src/cleanup_plan/replay/factored/canonical_shared/empty_memo.rs"] {
"####;
    const SCOPE_REPLACEMENTS_AFTER: &str = r####"        for source in ["src/cleanup_plan/replay/branch_clone/inline_tests.rs", "src/cleanup_plan/replay/branch_clone/production_capacity.rs", "src/cleanup_plan/replay/branch_clone/production_capacity/tests.rs", "src/cleanup_plan/replay/factored/canonical_shared/empty_memo.rs", "src/workspace_graph/shape_identity/source_controls.rs", "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory_leaf_views.rs"] {
"####;
    const COMBINED_9664_KEYS: &[&str] = &["src/cleanup_plan/replay/path_join/compact_failure_before.rs","src/cleanup_plan/replay/path_join/compact_failure_tests.rs","src/cleanup_plan/replay/path_join/contains_before.rs","src/cleanup_plan/replay/path_join/contains_tests.rs","src/cleanup_plan/replay/path_join/selected/tests/fixture_scope.rs","src/source_reflows_merge/main_e177_overlay_digests_v13.rs","src/source_reflows_merge/main_e177_overlay_digests_v13/logical_v12.rs","src/source_reflows_merge/main_e177_overlay_digests_v13/tests.rs","src/source_reflows_merge/main_e177_overlay_digests_v13/controller_transport.rs","src/cleanup_plan/replay/expression_facts/receiving/retained/case_proof/tests.rs","src/cleanup_plan/replay/expression_facts/receiving/remaining/tests.rs","tests/wasm/scalar_browser_ci_contract/current_profiles573.rs"];
    const COMBINED_9664_SCOPE_LINE: &str = r####"            "src/cleanup_plan/replay/path_join/compact_failure_before.rs", "src/cleanup_plan/replay/path_join/compact_failure_tests.rs", "src/cleanup_plan/replay/path_join/contains_before.rs", "src/cleanup_plan/replay/path_join/contains_tests.rs", "src/cleanup_plan/replay/path_join/selected/tests/fixture_scope.rs", "src/source_reflows_merge/main_e177_overlay_digests_v13.rs", "src/source_reflows_merge/main_e177_overlay_digests_v13/logical_v12.rs", "src/source_reflows_merge/main_e177_overlay_digests_v13/tests.rs", "src/source_reflows_merge/main_e177_overlay_digests_v13/controller_transport.rs", "src/cleanup_plan/replay/expression_facts/receiving/retained/case_proof/tests.rs", "src/cleanup_plan/replay/expression_facts/receiving/remaining/tests.rs", "tests/wasm/scalar_browser_ci_contract/current_profiles573.rs",
"####;
    const MAIN_C907_KEYS: &[&str] = &["src/cleanup_plan/replay/expression_facts/capacity/before_types.rs","src/cleanup_plan/replay/expression_facts/capacity/cursor_tests.rs","src/cleanup_plan/replay/expression_facts/capacity/type_cursor_tests.rs","src/source_verify/constructor_tests/corpus/operator_refusals.rs","src/cleanup_plan/replay/path_join/direct_one_before.rs","src/cleanup_plan/replay/path_join/direct_one_before/boxed_flow.rs","src/cleanup_plan/replay/path_join/direct_one_before/frontier.rs","src/cleanup_plan/replay/path_join/direct_one_tests.rs"];
    const MAIN_C907_SCOPE_LINE: &str = r####"            "src/cleanup_plan/replay/expression_facts/capacity/before_types.rs", "src/cleanup_plan/replay/expression_facts/capacity/cursor_tests.rs", "src/cleanup_plan/replay/expression_facts/capacity/type_cursor_tests.rs", "src/source_verify/constructor_tests/corpus/operator_refusals.rs", "src/cleanup_plan/replay/path_join/direct_one_before.rs", "src/cleanup_plan/replay/path_join/direct_one_before/boxed_flow.rs", "src/cleanup_plan/replay/path_join/direct_one_before/frontier.rs", "src/cleanup_plan/replay/path_join/direct_one_tests.rs",
"####;
    const MAIN_C907_REPLACEMENT_LINES: &[&str] = &["            \"src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/counter_tests.rs\",\n","            \"src/cleanup_plan/replay/expression_facts/arena/prefix/tests.rs\",\n"];
    const PY_SCOPE_INSERTIONS: &[&str] = &[ "\"src/cleanup_plan/replay/path_join/before_singleton.rs\",",",\"src/cleanup_plan/replay/transition.rs\"",",\"src/workspace_graph/initial_report/build.rs\"",",\"docs/AGENT-SKILL-BUNDLE-V1.json\"",",\"src/cleanup_plan/replay/path_join/main_single_groups/receiver_tests.rs\"",",\"src/cleanup_plan/replay/path_join/main3d81_before.rs\"", ",_REPLAY+'path_join/field_local/mod.rs'",",_REPLAY+'path_join/field_local/frontier.rs'",",_REPLAY+'path_join/field_local/boxed_flow.rs'",",_REPLAY+'path_join/field_local/tests.rs'", ",_REPLAY+'ordinary_plan_history/completed_fork/emission/retained.rs'", "'src/workspace_graph/tests.rs',", ",_REPLAY+'expression_facts/receiving/helpers/tests.rs',_REPLAY+'expression_facts/receiving/retained/exits/tests.rs',_REPLAY+'workspace_oracle.rs',_HIR+'inspection/paid_nul.rs',_HIR+'inspection/paid_nul/pending.rs',_HIR+'inspection/paid_nul/tests.rs',_HIR+'inspection/paid_nul/reuse_before.rs',_HIR+'inspection/paid_nul/reuse_before/pending.rs',_HIR+'inspection/paid_nul/type_scratch.rs',_REPLAY+'continuation_storage.rs',_REPLAY+'continuation_storage/tests.rs',_REPLAY+'continuation_storage/reached.rs',_REPLAY+'continuation_storage/reached_before.rs',_REPLAY+'continuation_storage/reached_tests.rs','src/cleanup_plan/replay/factored/paired.rs','src/cleanup_plan/replay/factored/paired/tests.rs','src/cleanup_plan/replay/path_join/fused_before.rs','src/cleanup_plan/replay/path_join/fused_before_frontier.rs','src/cleanup_plan/replay/path_join/fused_tests.rs'",
        "_REPLAY+'factored/canonical_shared/lexical_successors/tests.rs',", "_REPLAY+'factored/canonical_shared/intern_arena.rs',", ",'src/cleanup_plan/replay_tests/factored.rs'",
        "_REPLAY+'physical_budget/authentication.rs',", "_REPLAY+'factored/moderate_tests.rs',",
        ",_REPLAY+'path_join/compact_failure_before.rs',_REPLAY+'path_join/compact_failure_tests.rs',_REPLAY+'path_join/contains_before.rs',_REPLAY+'path_join/contains_tests.rs',_REPLAY+'path_join/selected/tests/fixture_scope.rs',_REFLOWS+'main_e177_overlay_digests_v13.rs',_REFLOWS+'main_e177_overlay_digests_v13/logical_v12.rs',_REFLOWS+'main_e177_overlay_digests_v13/tests.rs',_REFLOWS+'main_e177_overlay_digests_v13/controller_transport.rs',_REPLAY+'expression_facts/receiving/remaining/tests.rs','tests/wasm/scalar_browser_ci_contract/current_profiles573.rs'",
        "_REPLAY+'expression_facts/receiving/retained/case_proof/tests.rs',",
        ",_REPLAY+'expression_facts/capacity/before_types.rs',_REPLAY+'expression_facts/capacity/cursor_tests.rs',_REPLAY+'expression_facts/capacity/type_cursor_tests.rs','src/source_verify/constructor_tests/corpus/operator_refusals.rs',_REPLAY+'path_join/direct_one_before.rs',_REPLAY+'path_join/direct_one_before/boxed_flow.rs',_REPLAY+'path_join/direct_one_before/frontier.rs',_REPLAY+'path_join/direct_one_tests.rs'",
        "_REPLAY+'expression_facts/arena/prefix/tests.rs',",
        "_REPLAY+'expression_facts/receiving/retained/case_certified/counter_tests.rs',",
        "'src/workspace_graph/shape_identity/source_controls.rs',",
        "'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory_leaf_views.rs',",
        ",_REPLAY+'skeleton_work/query_proof.rs',_REPLAY+'skeleton_work/query_proof/before.rs',_REPLAY+'skeleton_work/query_proof/tests.rs','src/bounded_output/exclusive_debits/path_census.rs','src/bounded_output/request_reservation.rs',_REFLOWS+'main_e177_overlay_lookup.rs'",
    ];
    fn digest(value: &str) -> String {
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(value.as_bytes()))
        )
    }
    #[rustfmt::skip] pub(super) fn parent_for_current_scope(parent: &str, lookup: &str) -> String {
        assert_eq!(parent, ROOT);
        assert_eq!(lookup, LOOKUP); let main_ac791_arm = "        \"src/workspace_graph/expected_projection/borrowed_imports_original.rs\" => Some(include_str!(\"../../src/workspace_graph/expected_projection/borrowed_imports_original.rs\")),\n"; assert_eq!(lookup.len(), 94357); assert_eq!(digest(lookup), "87e1e8f1d4986faba675ed8a1465f1b575b5655b6f2343bb8bfb5d59a371ca83"); let actual_main_ac791_lookup = super::main48_historical_reader::read(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/source_reflows_merge/main_e177_overlay_lookup.rs"), 94357, "87e1e8f1d4986faba675ed8a1465f1b575b5655b6f2343bb8bfb5d59a371ca83", 2_097_152).expect("complete current compiled lookup physical image"); assert_eq!(actual_main_ac791_lookup, lookup); assert_eq!(lookup.matches(main_ac791_arm).count(), 1, "exact new main borrowed original lookup arm"); let before_main_ac791_admission = lookup.replacen(main_ac791_arm, "", 1); assert_eq!(before_main_ac791_admission.len(), 94177); assert_eq!(digest(&before_main_ac791_admission), "4205124f1f409cdd227d4fb14c3a931bce34d527c937d8c10383943066c96954"); let lookup = before_main_ac791_admission.as_str(); assert_eq!(lookup.len(), 94177); assert_eq!(digest(lookup), "4205124f1f409cdd227d4fb14c3a931bce34d527c937d8c10383943066c96954"); let mut before_main48_admission = lookup.to_owned(); for arm in ["        \"src/workspace_graph/expected_projection/borrowed_imports/tests.rs\" => Some(include_str!(\"../../src/workspace_graph/expected_projection/borrowed_imports/tests.rs\")),\n","        \"src/workspace_graph/expected_projection/borrowed_imports/tests/full.rs\" => Some(include_str!(\"../../src/workspace_graph/expected_projection/borrowed_imports/tests/full.rs\")),\n"] { assert_eq!(before_main48_admission.matches(arm).count(), 1, "exact main48 two compiled lookup arms"); before_main48_admission = before_main48_admission.replacen(arm, "", 1); } assert_eq!(before_main48_admission.len(), 93819); assert_eq!(digest(&before_main48_admission), "35c97c45831981c91aea6d9253aebf59eee044081d7c336abf4c244274a0ae2f"); let lookup = before_main48_admission.as_str(); assert_eq!(lookup.len(), 93819); assert_eq!(digest(lookup), "35c97c45831981c91aea6d9253aebf59eee044081d7c336abf4c244274a0ae2f"); let mut before_direct_place_admission = lookup.to_owned(); for arm in ["        \"src/hir/validation/scope_checkpoint/marks/direct_before.rs\" => Some(include_str!(\"../../src/hir/validation/scope_checkpoint/marks/direct_before.rs\")),\n","        \"src/hir/validation/scope_checkpoint/marks/direct_tests.rs\" => Some(include_str!(\"../../src/hir/validation/scope_checkpoint/marks/direct_tests.rs\")),\n"] { assert_eq!(before_direct_place_admission.matches(arm).count(), 1, "exact direct Place compiled lookup arms"); before_direct_place_admission = before_direct_place_admission.replacen(arm, "", 1); } assert_eq!(before_direct_place_admission.len(), 93501); assert_eq!(digest(&before_direct_place_admission), "6d4c6200aa557eae5b04014a5597fdc1bd188a676609a81cf2313180e49c3b3d"); let lookup = before_direct_place_admission.as_str(); assert_eq!(lookup.len(), 93501); assert_eq!(digest(lookup), "6d4c6200aa557eae5b04014a5597fdc1bd188a676609a81cf2313180e49c3b3d"); let mut before_main328_admission = lookup.to_owned(); for arm in ["        \"src/source_reflows_merge/constructor_stage.rs\" => Some(include_str!(\"../../src/source_reflows_merge/constructor_stage.rs\")),\n","        \"src/workspace_graph/expected_projection/borrowed_imports/typed/alias/owned_growth.rs\" => Some(include_str!(\"../../src/workspace_graph/expected_projection/borrowed_imports/typed/alias/owned_growth.rs\")),\n","        \"src/workspace_graph/expected_projection/defaults/field_directory.rs\" => Some(include_str!(\"../../src/workspace_graph/expected_projection/defaults/field_directory.rs\")),\n","        \"src/vec_ops/wrappers.rs\" => Some(include_str!(\"../../src/vec_ops/wrappers.rs\")),\n","        \"src/box_ops/wrappers.rs\" => Some(include_str!(\"../../src/box_ops/wrappers.rs\")),\n","        \"src/workspace_graph/expected_projection/defaults.rs\" => Some(include_str!(\"../../src/workspace_graph/expected_projection/defaults.rs\")),\n","        \"src/workspace_graph/expected_projection/import_stub.rs\" => Some(include_str!(\"../../src/workspace_graph/expected_projection/import_stub.rs\")),\n","        \"src/workspace_graph/expected_projection/borrowed_imports/tests/old.rs\" => Some(include_str!(\"../../src/workspace_graph/expected_projection/borrowed_imports/tests/old.rs\")),\n","        \"src/workspace_graph/expected_projection/borrowed_imports/tests/full/original_construct.rs\" => Some(include_str!(\"../../src/workspace_graph/expected_projection/borrowed_imports/tests/full/original_construct.rs\")),\n","        \"src/workspace_graph/expected_projection/borrowed_imports/tests/full/publication.rs\" => Some(include_str!(\"../../src/workspace_graph/expected_projection/borrowed_imports/tests/full/publication.rs\")),\n","        \"src/ast/program_source/decl_slices/alias_tests.rs\" => Some(include_str!(\"../../src/ast/program_source/decl_slices/alias_tests.rs\")),\n","        \"src/ast/program_source/decl_slices/before_alias.rs\" => Some(include_str!(\"../../src/ast/program_source/decl_slices/before_alias.rs\")),\n","        \"src/ast/program_source/function_alias.rs\" => Some(include_str!(\"../../src/ast/program_source/function_alias.rs\")),\n","        \"src/workspace_graph/expected_projection/borrowed_imports/typed/alias/tests.rs\" => Some(include_str!(\"../../src/workspace_graph/expected_projection/borrowed_imports/typed/alias/tests.rs\")),\n","        \"src/hir/identity_resolution/nominal.rs\" => Some(include_str!(\"../../src/hir/identity_resolution/nominal.rs\")),\n"] { assert_eq!(before_main328_admission.matches(arm).count(), 1, "exact incoming fifteen compiled lookup arms"); before_main328_admission = before_main328_admission.replacen(arm, "", 1); } assert_eq!(before_main328_admission.len(), 91159); assert_eq!(digest(&before_main328_admission), "228b873c91c1283c148742795dc45bf8b4464a49d0e574b02046762336b3e30d"); let lookup = before_main328_admission.as_str(); assert_eq!(lookup.len(), 91159); assert_eq!(digest(lookup), "228b873c91c1283c148742795dc45bf8b4464a49d0e574b02046762336b3e30d"); let closed_roots_test_arm = "        \"src/workspace_graph/expected_projection/call_carriers/closed_roots_tests.rs\" => Some(include_str!(\"../../src/workspace_graph/expected_projection/call_carriers/closed_roots_tests.rs\")),\n"; assert_eq!(lookup.matches(closed_roots_test_arm).count(), 1, "exact cfg(test) closed-roots extraction arm"); let before_closed_roots_extraction = lookup.replacen(closed_roots_test_arm, "", 1); let lookup = before_closed_roots_extraction.as_str(); assert_eq!(lookup.len(), 90965); assert_eq!(format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(lookup.as_bytes()))), "64f9913b90f4f7d68ff9bad77634f1814085d47b39456587f3946ed590028b78"); let admission1011_arms = "        \"src/source_reflows_merge/main_e177_overlay_names_v11/main_e177_overlay_pins_v10.rs\" => Some(include_str!(\"../../src/source_reflows_merge/main_e177_overlay_names_v11/main_e177_overlay_pins_v10.rs\")),\n        \"src/source_reflows_merge/main_e177_overlay_incoming064.rs\" => Some(include_str!(\"../../src/source_reflows_merge/main_e177_overlay_incoming064.rs\")),\n        \"src/cleanup_plan/replay/expression_facts/receiving/directory/invocation.rs\" => Some(include_str!(\"../../src/cleanup_plan/replay/expression_facts/receiving/directory/invocation.rs\")),\n        \"src/cleanup_plan/replay/expression_facts/receiving/directory/invocation_tests.rs\" => Some(include_str!(\"../../src/cleanup_plan/replay/expression_facts/receiving/directory/invocation_tests.rs\")),\n        \"src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_status_invocation.rs\" => Some(include_str!(\"../../src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_status_invocation.rs\")),\n        \"src/cleanup_plan/replay/projection_backing.rs\" => Some(include_str!(\"../../src/cleanup_plan/replay/projection_backing.rs\")),\n        \"src/cleanup_plan/replay/projection_backing/tests.rs\" => Some(include_str!(\"../../src/cleanup_plan/replay/projection_backing/tests.rs\")),\n        \"src/hir/canonical_loan_storage/caller/stream.rs\" => Some(include_str!(\"../../src/hir/canonical_loan_storage/caller/stream.rs\")),\n        \"src/hir/canonical_loan_storage/caller/stream/tests.rs\" => Some(include_str!(\"../../src/hir/canonical_loan_storage/caller/stream/tests.rs\")),\n        \"src/hir/validation/lexical_exit/lexical_join.rs\" => Some(include_str!(\"../../src/hir/validation/lexical_exit/lexical_join.rs\")),\n        \"src/loan_plan/canonical_sink.rs\" => Some(include_str!(\"../../src/loan_plan/canonical_sink.rs\")),\n        \"src/loan_plan/paid_materialize/immediate_before.rs\" => Some(include_str!(\"../../src/loan_plan/paid_materialize/immediate_before.rs\")),\n        \"src/loan_plan/paid_materialize/owned_inputs_tests.rs\" => Some(include_str!(\"../../src/loan_plan/paid_materialize/owned_inputs_tests.rs\")),\n        \"src/workspace_graph/expected_projection/call_carriers/closed_roots.rs\" => Some(include_str!(\"../../src/workspace_graph/expected_projection/call_carriers/closed_roots.rs\")),\n        \"src/source_reflows_merge/main_e177_overlay_digests_v13/typed_refs_v18.rs\" => Some(include_str!(\"../../src/source_reflows_merge/main_e177_overlay_digests_v13/typed_refs_v18.rs\")),\n        \"src/hir/validation/scope_checkpoint/merge/moved_direct.rs\" => Some(include_str!(\"../../src/hir/validation/scope_checkpoint/merge/moved_direct.rs\")),\n        \"src/hir/validation/scope_checkpoint/merge/moved_direct/before.rs\" => Some(include_str!(\"../../src/hir/validation/scope_checkpoint/merge/moved_direct/before.rs\")),\n        \"src/hir/validation/scope_checkpoint/merge/moved_direct/tests.rs\" => Some(include_str!(\"../../src/hir/validation/scope_checkpoint/merge/moved_direct/tests.rs\")),\n"; assert_eq!(lookup.matches(admission1011_arms).count(), 1, "exact eighteen compiled logical additions"); let admission1011_before = lookup.replacen(admission1011_arms, "", 1); assert_eq!(admission1011_before.len(), 88007); assert_eq!(format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(admission1011_before.as_bytes()))), "767833956590d53f846bbe706da3c9a79cfdf040f2bdb2a3deb64397fb41f287"); let admission1011_archive = super::super::held_4a6eb(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("e/767833956590d53f846bbe706da3c9a79cfdf040f2bdb2a3deb64397fb41f287"), 88007, "767833956590d53f846bbe706da3c9a79cfdf040f2bdb2a3deb64397fb41f287").unwrap(); assert_eq!(admission1011_before, admission1011_archive); let lookup = admission1011_before.as_str(); let observer_arm = "        \"src/workspace_graph/forecast_observation.rs\" => Some(include_str!(\"../../src/workspace_graph/forecast_observation.rs\")),\n"; assert_eq!(lookup.matches(observer_arm).count(), 1, "exact newly audited forecast observer lookup arm"); let before_observer = lookup.replacen(observer_arm, "", 1); let lookup = before_observer.as_str(); let hostpin_arm = "        \"tests/workspace/semantic_graph/host_pins.rs\" => Some(include_str!(\"../../tests/workspace/semantic_graph/host_pins.rs\")),\n"; assert_eq!(lookup.matches(hostpin_arm).count(), 1, "exact incoming host-pin lookup arm"); let before_hostpins = lookup.replacen(hostpin_arm, "", 1); let lookup = before_hostpins.as_str(); let main8_903_arms = "        \"src/cleanup_plan/replay/expression_facts/arena/cursor_tests.rs\" => Some(include_str!(\"../../src/cleanup_plan/replay/expression_facts/arena/cursor_tests.rs\")),\n        \"src/workspace_graph/expected_projection/retained_scratch.rs\" => Some(include_str!(\"../../src/workspace_graph/expected_projection/retained_scratch.rs\")),\n        \"src/workspace_graph/expected_projection/shared_local_callees/before903.rs\" => Some(include_str!(\"../../src/workspace_graph/expected_projection/shared_local_callees/before903.rs\")),\n        \"src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers.rs\" => Some(include_str!(\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers.rs\")),\n        \"src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_directory.rs\" => Some(include_str!(\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_directory.rs\")),\n        \"src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_directory/modular.rs\" => Some(include_str!(\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_directory/modular.rs\")),\n        \"src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_directory/reused.rs\" => Some(include_str!(\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_directory/reused.rs\")),\n        \"src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_joint.rs\" => Some(include_str!(\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_joint.rs\")),\n        \"src/workspace_graph/expected_projection/shared_local_callees/before903/held_projection/modular_projection.rs\" => Some(include_str!(\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/held_projection/modular_projection.rs\")),\n        \"src/workspace_graph/expected_projection/shared_local_callees/before903/held_projection/paired_projection.rs\" => Some(include_str!(\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/held_projection/paired_projection.rs\")),\n        \"src/workspace_graph/expected_projection/shared_local_callees/before903/modular_pair.rs\" => Some(include_str!(\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/modular_pair.rs\")),\n        \"src/workspace_graph/expected_projection/shared_local_callees/before903/paired.rs\" => Some(include_str!(\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/paired.rs\")),\n        \"src/workspace_graph/expected_projection/shared_local_callees/before903/shared_local_callees/modular_census.rs\" => Some(include_str!(\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/shared_local_callees/modular_census.rs\")),\n        \"src/workspace_graph/expected_projection/uncached_peak/retained_scratch_tests.rs\" => Some(include_str!(\"../../src/workspace_graph/expected_projection/uncached_peak/retained_scratch_tests.rs\")),\n        \"src/workspace_graph/expected_projection/uncached_peak_before.rs\" => Some(include_str!(\"../../src/workspace_graph/expected_projection/uncached_peak_before.rs\")),\n        \"src/hir.rs\" => Some(include_str!(\"../../src/hir.rs\")),\n        \"src/hir/function_directory.rs\" => Some(include_str!(\"../../src/hir/function_directory.rs\")),\n        \"src/loan_plan/owned_capacity.rs\" => Some(include_str!(\"../../src/loan_plan/owned_capacity.rs\")),\n        \"src/workspace_graph/expected_projection/held_projection_original.rs\" => Some(include_str!(\"../../src/workspace_graph/expected_projection/held_projection_original.rs\")),\n        \"src/workspace_graph/expected_projection/prelude_forecast_original.rs\" => Some(include_str!(\"../../src/workspace_graph/expected_projection/prelude_forecast_original.rs\")),\n        \"src/workspace_graph/type_proof_tests.rs\" => Some(include_str!(\"../../src/workspace_graph/type_proof_tests.rs\")),\n        \"src/workspace_graph/tests/builder_limits.rs\" => Some(include_str!(\"../../src/workspace_graph/tests/builder_limits.rs\")),\n        \"src/bounded_output/stage_debits.rs\" => Some(include_str!(\"../../src/bounded_output/stage_debits.rs\")),\n        \"src/cleanup_plan/renewal_profile/child_refusal.rs\" => Some(include_str!(\"../../src/cleanup_plan/renewal_profile/child_refusal.rs\")),\n        \"src/cleanup_plan/replay/branch_clone/before.rs\" => Some(include_str!(\"../../src/cleanup_plan/replay/branch_clone/before.rs\")),\n        \"src/cleanup_plan/replay/compact_stage.rs\" => Some(include_str!(\"../../src/cleanup_plan/replay/compact_stage.rs\")),\n        \"src/cleanup_plan/replay/factored/canonical_shared/main_unit_weight/unit_weight_tests.rs\" => Some(include_str!(\"../../src/cleanup_plan/replay/factored/canonical_shared/main_unit_weight/unit_weight_tests.rs\")),\n        \"src/cleanup_plan/replay/factored/oracle_order/tests.rs\" => Some(include_str!(\"../../src/cleanup_plan/replay/factored/oracle_order/tests.rs\")),\n        \"src/cleanup_plan/replay/path_array/funded_tests.rs\" => Some(include_str!(\"../../src/cleanup_plan/replay/path_array/funded_tests.rs\")),\n        \"src/cleanup_plan/replay/path_array/tests.rs\" => Some(include_str!(\"../../src/cleanup_plan/replay/path_array/tests.rs\")),\n        \"src/source_reflows_merge/maina3_custody.rs\" => Some(include_str!(\"../../src/source_reflows_merge/maina3_custody.rs\")),\n        \"src/cleanup_plan/replay/factored/compact_nodes/tests.rs\" => Some(include_str!(\"../../src/cleanup_plan/replay/factored/compact_nodes/tests.rs\")),\n        \"src/cleanup_plan/replay/selected_context/child.rs\" => Some(include_str!(\"../../src/cleanup_plan/replay/selected_context/child.rs\")),\n        \"src/workspace_graph/diagnostics.rs\" => Some(include_str!(\"../../src/workspace_graph/diagnostics.rs\")),\n        \"src/workspace_graph/diagnostics/borrowed_help_tests.rs\" => Some(include_str!(\"../../src/workspace_graph/diagnostics/borrowed_help_tests.rs\")),\n        \"src/workspace_graph/expected_projection/import_stub/core_proofs/tests.rs\" => Some(include_str!(\"../../src/workspace_graph/expected_projection/import_stub/core_proofs/tests.rs\")),\n"; assert_eq!(lookup.matches(main8_903_arms).count(), 1, "exact main8 and constructor-retention lookup suffix"); let prior_main8_903_lookup = lookup.replacen(main8_903_arms, "", 1); assert_eq!(prior_main8_903_lookup.len(), 78389); assert_eq!(digest(&prior_main8_903_lookup), "fc334e8e33719e8c8725de541564f7a03e9d5925cd613f3953f2505a01dffeb4"); let closure926_930_arms = "        \"src/cleanup_plan/replay/continuation_storage/oracle_reached.rs\" => Some(include_str!(\"../../src/cleanup_plan/replay/continuation_storage/oracle_reached.rs\")),\n        \"src/cleanup_plan/replay/continuation_storage/oracle_reached_tests.rs\" => Some(include_str!(\"../../src/cleanup_plan/replay/continuation_storage/oracle_reached_tests.rs\")),\n        \"src/cleanup_plan/replay/factored/oracle_order.rs\" => Some(include_str!(\"../../src/cleanup_plan/replay/factored/oracle_order.rs\")),\n        \"src/cleanup_plan/replay/factored/oracle_order/before_flat_identity.rs\" => Some(include_str!(\"../../src/cleanup_plan/replay/factored/oracle_order/before_flat_identity.rs\")),\n        \"src/cleanup_plan/replay/factored/oracle_order/flat_identity_tests.rs\" => Some(include_str!(\"../../src/cleanup_plan/replay/factored/oracle_order/flat_identity_tests.rs\")),\n        \"src/cleanup_plan/replay/oracle_closure_before/custody.rs\" => Some(include_str!(\"../../src/cleanup_plan/replay/oracle_closure_before/custody.rs\")),\n        \"src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_countdown_tests.rs\" => Some(include_str!(\"../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_countdown_tests.rs\")),\n        \"src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_heads/before_countdown.rs\" => Some(include_str!(\"../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_heads/before_countdown.rs\")),\n        \"src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/before_branch_frames.rs\" => Some(include_str!(\"../../src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/before_branch_frames.rs\")),\n        \"src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/branch_frames.rs\" => Some(include_str!(\"../../src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/branch_frames.rs\")),\n        \"src/source_reflows/ci_inputs.rs\" => Some(include_str!(\"../../src/source_reflows/ci_inputs.rs\")),\n        \"src/workflow_profile/plan_comparison.rs\" => Some(include_str!(\"../../src/workflow_profile/plan_comparison.rs\")),\n        \"src/workspace_graph/expected_projection/identity_prebound_tests.rs\" => Some(include_str!(\"../../src/workspace_graph/expected_projection/identity_prebound_tests.rs\")),\n"; assert_eq!(prior_main8_903_lookup.matches(closure926_930_arms).count(), 1, "exact thirteen closure926-930 lookup arms"); let prior_main8_903_lookup = prior_main8_903_lookup.replacen(closure926_930_arms, "", 1); assert_eq!(prior_main8_903_lookup.len(), 76063); assert_eq!(digest(&prior_main8_903_lookup), "c621ebfa81b46f511fda7446d73f2b6a3dc5a6c73d5b3a08f426cf01c1e437cb"); let lookup = prior_main8_903_lookup.as_str(); let next_leaf_arm_0 = "        \"src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/immediate_before.rs\" => Some(include_str!(\"../../src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/immediate_before.rs\")),\n"; assert_eq!(lookup.matches(next_leaf_arm_0).count(), 1, "exact newly registered leaf lookup arm"); let prior_leaf_lookup_0 = lookup.replacen(next_leaf_arm_0, "", 1); let lookup = prior_leaf_lookup_0.as_str(); let next_leaf_arm_1 = "        \"tests/wasm/scalar_browser_ci_contract/current_profiles3d81.rs\" => Some(include_str!(\"../../tests/wasm/scalar_browser_ci_contract/current_profiles3d81.rs\")),\n"; assert_eq!(lookup.matches(next_leaf_arm_1).count(), 1, "exact newly registered leaf lookup arm"); let prior_leaf_lookup_1 = lookup.replacen(next_leaf_arm_1, "", 1); let lookup = prior_leaf_lookup_1.as_str(); let main3d81_arms = "        \"src/cleanup_plan/replay/transition.rs\" => Some(include_str!(\"../../src/cleanup_plan/replay/transition.rs\")),\n        \"src/workspace_graph/initial_report/build.rs\" => Some(include_str!(\"../../src/workspace_graph/initial_report/build.rs\")),\n        \"src/cleanup_plan/replay/path_join/main_single_groups/receiver_tests.rs\" => Some(include_str!(\"../../src/cleanup_plan/replay/path_join/main_single_groups/receiver_tests.rs\")),\n        \"src/cleanup_plan/replay/path_join/main3d81_before.rs\" => Some(include_str!(\"../../src/cleanup_plan/replay/path_join/main3d81_before.rs\")),\n"; assert_eq!(lookup.matches(main3d81_arms).count(), 1, "exact incoming3d81 lookup arms and complete oracle"); let main3d81_lookup = lookup.replacen(main3d81_arms, "", 1); let lookup = main3d81_lookup.as_str(); let field_local_arms = "        \"src/cleanup_plan/replay/path_join/field_local/mod.rs\" => Some(include_str!(\"../../src/cleanup_plan/replay/path_join/field_local/mod.rs\")),\n        \"src/cleanup_plan/replay/path_join/field_local/frontier.rs\" => Some(include_str!(\"../../src/cleanup_plan/replay/path_join/field_local/frontier.rs\")),\n        \"src/cleanup_plan/replay/path_join/field_local/boxed_flow.rs\" => Some(include_str!(\"../../src/cleanup_plan/replay/path_join/field_local/boxed_flow.rs\")),\n        \"src/cleanup_plan/replay/path_join/field_local/tests.rs\" => Some(include_str!(\"../../src/cleanup_plan/replay/path_join/field_local/tests.rs\")),\n"; assert_eq!(lookup.matches(field_local_arms).count(), 1, "exact four field-local lookup arms"); let field_prior_lookup = lookup.replacen(field_local_arms, "", 1); let lookup = field_prior_lookup.as_str(); let retained_leaf_arm = "        \"src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/retained.rs\" => Some(include_str!(\"../../src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/retained.rs\")),\n"; assert_eq!(lookup.matches(retained_leaf_arm).count(), 1, "exact new retained-fork lookup arm"); let prior_lookup = lookup.replacen(retained_leaf_arm, "", 1); let lookup = prior_lookup.as_str();
        let prior_parent = super::super::fragments_v3::v13_before(
            "src/source_reflows_merge/main_e177_overlay_compiled.rs", parent,
        );
        let parent = prior_parent.as_str();
        let marker = concat!("\n// ", "CENSUS_LOOKUP_SPLIT_CUSTODY: additive controls; complete old parents above.\n");
        assert_eq!(parent.matches(marker).count(), 1);
        let (prefix, _) = parent.split_once(marker).unwrap();
        let mut restored = prefix.to_owned();
        let canonical_acquisition = "        let canonical_child = super::super::fragments_v3::v13_before(\n            \"src/source_reflows_merge/main_e177_overlay_fragments_v3.rs\", canonical_child,\n        );\n";
        assert_eq!(restored.matches(canonical_acquisition).count(), 1);
        restored = restored.replacen(canonical_acquisition, "", 1);
        assert_eq!(restored.matches(GLUE).count(), 1);
        assert_eq!(restored.matches(BRIDGE).count(), 1);
        let visibility = "pub(crate) fn compiled";
        assert_eq!(lookup.matches(visibility).count(), 1);
        let table = format!("{lookup}\n").replacen(visibility, "pub(super) fn compiled", 1);
        restored = restored.replacen(GLUE, &table, 1);
        restored.replacen(BRIDGE, "", 1)
    }
    pub(super) fn parent_before(parent: &str, lookup: &str) -> String {
        let mut restored = parent_for_current_scope(parent, lookup);
        let family_scope = "            \"src/hir/inspection/paid_nul.rs\", \"src/hir/inspection/paid_nul/pending.rs\", \"src/hir/inspection/paid_nul/tests.rs\", \"src/hir/inspection/paid_nul/reuse_before.rs\", \"src/hir/inspection/paid_nul/reuse_before/pending.rs\", \"src/hir/inspection/paid_nul/type_scratch.rs\", \"src/cleanup_plan/replay/continuation_storage.rs\", \"src/cleanup_plan/replay/continuation_storage/tests.rs\", \"src/cleanup_plan/replay/continuation_storage/reached.rs\", \"src/cleanup_plan/replay/continuation_storage/reached_before.rs\", \"src/cleanup_plan/replay/continuation_storage/reached_tests.rs\",\"src/cleanup_plan/replay/path_join/fused_before.rs\", \"src/cleanup_plan/replay/path_join/fused_before_frontier.rs\", \"src/cleanup_plan/replay/path_join/fused_tests.rs\",\n";
        assert_eq!(restored.matches(family_scope).count(), 2);
        restored = restored.replace(family_scope, "");
        for key in COMBINED_9664_KEYS {
            let arm = format!("        {key:?} => Some(include_str!(\"../../{key}\")),\n");
            assert_eq!(restored.matches(&arm).count(), 1);
            restored = restored.replacen(&arm, "", 1);
        }
        assert_eq!(restored.matches(COMBINED_9664_SCOPE_LINE).count(), 2);
        restored = restored.replace(COMBINED_9664_SCOPE_LINE, "");
        for key in MAIN_C907_KEYS {
            let arm = format!("        {key:?} => Some(include_str!(\"../../{key}\")),\n");
            assert_eq!(restored.matches(&arm).count(), 1);
            restored = restored.replacen(&arm, "", 1);
        }
        assert_eq!(restored.matches(MAIN_C907_SCOPE_LINE).count(), 2);
        restored = restored.replace(MAIN_C907_SCOPE_LINE, "");
        for inserted in MAIN_C907_REPLACEMENT_LINES {
            assert_eq!(restored.matches(inserted).count(), 1);
            restored = restored.replacen(inserted, "", 1);
        }
        let auth_replacement = "            \"src/cleanup_plan/replay/physical_budget/authentication.rs\",\n";
        assert_eq!(restored.matches(auth_replacement).count(), 1);
        restored = restored.replacen(auth_replacement, "", 1);
        assert_eq!(BEFORE.len(), 176187);
        assert_eq!(digest(BEFORE), "8dacf765e1d67f7b25d368dc740c80c9987af7155fd9a54c99bd3258a275b8db");
        for key in KEYS {
            let arm = format!("        {key:?} => Some(include_str!(\"../../{key}\")),\n");
            assert_eq!(restored.matches(&arm).count(), 1);
            restored = restored.replacen(&arm, "", 1);
        }
        assert_eq!(restored.matches(SCOPE_ADDITIONS_LINE).count(), 2);
        restored = restored.replace(SCOPE_ADDITIONS_LINE, "");
        let paired_scope = "            \"src/cleanup_plan/replay/factored.rs\", \"src/cleanup_plan/replay/factored/strategy.rs\", \"src/cleanup_plan/replay/factored/paired.rs\", \"src/cleanup_plan/replay/factored/paired/tests.rs\",\n";
        assert_eq!(restored.matches(paired_scope).count(), 1);
        restored = restored.replacen(paired_scope, "", 1);
        let main99_scope = "            \"src/cleanup_plan.rs\",\n            \"src/workspace_graph/tests.rs\",\n            \"src/cleanup_plan/replay/expression_facts/receiving/helpers/tests.rs\",\n            \"src/cleanup_plan/replay/expression_facts/receiving/retained/exits/tests.rs\",\n            \"src/cleanup_plan/replay/workspace_oracle.rs\",\n";
        assert_eq!(restored.matches(main99_scope).count(), 1);
        restored = restored.replacen(main99_scope, "", 1);
        let financed_scope = "        for source in [\"src/cleanup_plan/replay/factored/canonical_shared/lexical_successors/tests.rs\", \"src/cleanup_plan/replay/factored/canonical_shared/intern_arena.rs\", \"src/cleanup_plan/replay_tests/factored.rs\", \"src/cleanup_plan/replay/factored/moderate_tests.rs\", \"src/cleanup_plan/replay/factored/paired.rs\", \"src/cleanup_plan/replay/factored/paired/tests.rs\", \"src/workspace_graph/tests.rs\", \"src/cleanup_plan/replay/expression_facts/receiving/helpers/tests.rs\", \"src/cleanup_plan/replay/expression_facts/receiving/retained/exits/tests.rs\", \"src/cleanup_plan/replay/workspace_oracle.rs\", ";
        assert_eq!(restored.matches(financed_scope).count(), 1);
        restored = restored.replacen(financed_scope, "        for source in [", 1);
        assert_eq!(restored.matches(SCOPE_REPLACEMENTS_AFTER).count(), 1);
        restored = restored.replacen(SCOPE_REPLACEMENTS_AFTER, SCOPE_REPLACEMENTS_BEFORE, 1);
        let fitting_adapter = " let fitting_leaf_arm = \"        \\\"src/cleanup_plan/replay_tests/factored.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay_tests/factored.rs\\\")),\\n\"; assert_eq!(child.matches(fitting_leaf_arm).count(), 1, \"exact new fitting-leaf lookup arm\"); let fitting_prior_child = child.replacen(fitting_leaf_arm, \"\", 1); let child = fitting_prior_child.as_str();"; assert_eq!(restored.matches(fitting_adapter).count(), 1); restored = restored.replacen(fitting_adapter, "", 1); assert_eq!(restored, BEFORE, "ENTIRE published pre-split parent");
        restored
    }
    fn python_before(parent: &str, child: &str) -> String {
        assert_eq!(parent, PYTHON);
        assert_eq!(child, CODECS);
        assert_eq!(PYTHON_BEFORE.len(), 65515);
        assert_eq!(
            digest(PYTHON_BEFORE),
            "ab489a4486a46998595d993477806bed9e21ace1874774c64433debeb2be6c72"
        );
        let prior_parent = super::super::fragments_v3::v13_before(
            "docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py", parent,
        );
        let prior_child = super::super::fragments_v3::v13_before(
            "docs/evidence/finite-source-overlay-v2-20261018/overlay_page_codecs.py", child,
        );
        let parent = prior_parent.as_str();
        let child = prior_child.as_str();
        assert_eq!(parent.matches(LOADER).count(), 1);
        let mut restored = parent.to_owned();
        for &inserted in PY_SCOPE_INSERTIONS {
            assert_eq!(restored.matches(inserted).count(), 1);
            restored = restored.replacen(inserted, "", 1);
        }
        let mut restored = restored.replacen(LOADER, &format!("{child}\n"), 1);
        // Policy declarations are derived metadata; the complete current parent
        // and its V13 caller acquisition authenticated above precede this exact
        // two-field transport to the immutable published source image.
        for prefix in ["POLICY_LENGTH = ", "POLICY_SHA256 = "] {
            let current: Vec<_> = parent.lines().filter(|line| line.starts_with(prefix)).collect();
            let previous: Vec<_> = PYTHON_BEFORE.lines().filter(|line| line.starts_with(prefix)).collect();
            assert_eq!((current.len(), previous.len()), (1, 1));
            assert_eq!(restored.matches(current[0]).count(), 1);
            restored = restored.replacen(current[0], previous[0], 1);
        }
        assert_eq!(
            restored, PYTHON_BEFORE,
            "ENTIRE published pre-split Python parent"
        );
        restored
    }
    fn physical_codec_loader(hostile: bool) {
        const PROBE: &str = r####"import os, pathlib, subprocess, sys, tempfile
parent, child, mode = sys.argv[1:]
probe = "import runpy,sys; ns=runpy.run_path(sys.argv[1],run_name='codec_fixture'); assert callable(ns['encode_v12_view_labels']); assert ns['PAGE_CAP']==262144 and ns['SOURCE_CAP']==9753094"
with tempfile.TemporaryDirectory(prefix='semaprax-codec-loader-') as temporary:
 directory=pathlib.Path(temporary)
 program=directory/'overlay_page.py'; codec=directory/'overlay_page_codecs.py'
 program.write_text(parent); codec.write_text(child)
 def invoke(code=probe):
  return subprocess.run([sys.executable,'-B','-c',code,str(program)],capture_output=True,timeout=3)
 result=invoke()
 assert result.returncode==0,result.stderr.decode()
 if mode=='hostile':
  for case in ('missing','length','hash','symlink','directory','fifo','drift'):
   if codec.is_dir() and not codec.is_symlink():codec.rmdir()
   elif codec.exists() or codec.is_symlink():codec.unlink()
   if case=='length':codec.write_text(child+' ')
   elif case=='hash':codec.write_text(('x' if child[0]!='x' else 'y')+child[1:])
   elif case=='symlink':
    target=directory/'real-codec.py';target.write_text(child);codec.symlink_to(target)
   elif case=='directory':codec.mkdir()
   elif case=='fifo':os.mkfifo(codec)
   elif case=='drift':codec.write_text(child)
   code=probe
   if case=='drift':
    code="""import os,pathlib,runpy,sys
original=os.fstat
calls=0
def replace_after_read(fd):
 global calls
 calls+=1
 if calls==2:
  path=pathlib.Path(sys.argv[1]).with_name('overlay_page_codecs.py')
  replacement=path.with_name('replacement.py')
  replacement.write_bytes(path.read_bytes())
  os.replace(replacement,path)
 return original(fd)
os.fstat=replace_after_read
runpy.run_path(sys.argv[1],run_name='codec_fixture')
"""
   result=invoke(code)
   assert result.returncode!=0,case
   if case=='drift':assert b'codec postread identity' in result.stderr,result.stderr.decode()
"####;
        let output = std::process::Command::new("python3")
            .args(["-B", "-c", PROBE, PYTHON, CODECS, if hostile { "hostile" } else { "positive" }])
            .output().expect("run fixed codec loader controls");
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    }
    #[test]
    fn census_lookup_and_codec_split_preserve_complete_published_parents_and_all_mappings() {
        assert_eq!(parent_before(ROOT, LOOKUP), BEFORE);
        assert_eq!(python_before(PYTHON, CODECS), PYTHON_BEFORE);
        physical_codec_loader(false);
        for key in KEYS {
            assert!(super::compiled(key).is_some(), "missing new source {key}");
        }
    }
    #[test]
    fn census_lookup_and_codec_split_refuse_every_source_seam_drift() {
        physical_codec_loader(true);
        for parent in [
            format!("{ROOT} "),
            ROOT.replacen(GLUE, "", 1),
            ROOT.replacen(BRIDGE, "", 1),
        ] {
            assert!(std::panic::catch_unwind(|| parent_before(&parent, LOOKUP)).is_err());
        }
        for child in [
            format!("{LOOKUP} "),
            LOOKUP.replacen("match source", "match sourcf", 1),
            LOOKUP.replacen("Some(include_str!", "None/*include_str!", 1),
        ] {
            assert!(std::panic::catch_unwind(|| parent_before(ROOT, &child)).is_err());
        }
        let current_loader = LOADER.replacen(" length=10905\n", &format!(" length={}\n", CODECS.len()), 1).replacen("a0f906f11a8c7205a2124e5cd9c1909fa1a53a12731a0775d3e360d844bab794", &digest(CODECS), 1); assert_eq!(PYTHON.matches(&current_loader).count(), 1); for parent in [
            format!("{PYTHON} "),
            PYTHON.replacen(&current_loader, "", 1),
            PYTHON.replacen("os.O_NOFOLLOW", "0", 1),
        ] {
            assert!(std::panic::catch_unwind(|| python_before(&parent, CODECS)).is_err());
        }
        for child in [
            format!("{CODECS} "),
            CODECS.replacen("def v10_archive_slots", "def v10_archive_slotx", 1),
        ] {
            assert!(std::panic::catch_unwind(|| python_before(PYTHON, &child)).is_err());
        }
    }

    #[rustfmt::skip]
    fn incoming48_wire_before(root: &str) -> String {
        const INCOMING: &str = include_str!("../../e/066e37a105113138b4fa62a0154ff94f54078da735e05fd924b3596954d8107f");
        const TARGET: &str = include_str!("../../e/b66377d59526b82e1020058f9b8514419902bbeaf3bfa019a721b8ae087b6f3f");
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("e/066e37a105113138b4fa62a0154ff94f54078da735e05fd924b3596954d8107f");
        let physical = super::main48_historical_reader::read(&path, 73910, "066e37a105113138b4fa62a0154ff94f54078da735e05fd924b3596954d8107f", 2_097_152).expect("whole incoming48 root archive");
        assert_eq!(root, INCOMING); assert_eq!(physical, INCOMING); assert_eq!(root.len(), 73910); assert_eq!(digest(root), "066e37a105113138b4fa62a0154ff94f54078da735e05fd924b3596954d8107f");
        // Undo only the complete new wire dispatch and its test view; all old inverses follow.
        let mut v11_root = root.to_owned();
        let helper = "// Closed policy membership only; current source/archive bytes are never retained.\npub(super) fn declares_source(source: &str) -> bool {\n    REPLACEMENTS.contains(&source) || ADDITIONS.contains(&source)\n}\n\n";
        assert_eq!(v11_root.matches(helper).count(), 1);
        v11_root = v11_root.replacen(helper, "", 1);
        let added = "    if page[\"schema\"] == fragments_v3::SCHEMA_V11 {\n        let expanded = fragments_v3::expand_v11(&before, policy, page)?;\n        return compose_values_v2(before, policy, &expanded);\n    }\n";
        assert_eq!(v11_root.matches(added).count(), 1);
        v11_root = v11_root.replacen(added, "", 1);
        let added = "        // Decode only the new wire envelope before the unchanged full predecessor checks.\n        let page = if page[\"schema\"] == fragments_v3::SCHEMA_V11 {\n            fragments_v3::expand_v11(&before, &policy, &page).unwrap()\n        } else {\n            (*page).clone()\n        };\n";
        assert_eq!(v11_root.matches(added).count(), 1);
        v11_root = v11_root.replacen(added, "", 1);
        let root = v11_root.as_str();
        assert_eq!(root, TARGET, "ENTIRE named before-main48-wire stage"); assert_eq!(root.len(), 73227); assert_eq!(digest(root), "b66377d59526b82e1020058f9b8514419902bbeaf3bfa019a721b8ae087b6f3f");
        assert_eq!(super::main48_historical_reader::read(&path, 73910, "066e37a105113138b4fa62a0154ff94f54078da735e05fd924b3596954d8107f", 2_097_152).unwrap(), INCOMING, "late incoming archive reread");
        v11_root
    }
    #[rustfmt::skip] #[test]
    fn incoming48_wire_inverse_preserves_the_entire_authenticated_named_stage() {
        let incoming = include_str!("../../e/066e37a105113138b4fa62a0154ff94f54078da735e05fd924b3596954d8107f"); assert_eq!(incoming48_wire_before(incoming), include_str!("../../e/b66377d59526b82e1020058f9b8514419902bbeaf3bfa019a721b8ae087b6f3f"));
        assert!(std::panic::catch_unwind(|| incoming48_wire_before(&format!("{incoming} "))).is_err());
    }
}
