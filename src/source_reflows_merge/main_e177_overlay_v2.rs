//! One fixed metadata overlay. Current source admission follows composition.
use super::{digest, held_4a6eb, SOURCE_CAP};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::{Arc, Mutex};
const PAGE_CAP: usize = 262144;
#[rustfmt::skip]
const REPLACEMENTS: &[&str] = &["CHANGELOG.md","crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs","docs/ARCHITECTURE.md","docs/SUMMARY.md","src/cleanup_plan/replay.rs","src/cleanup_plan/replay/branch_clone/direct.rs","src/cleanup_plan/replay/branch_clone/direct/before.rs","src/cleanup_plan/replay/branch_clone/direct/tests.rs","src/cleanup_plan/replay/expression_facts/receiving/directory.rs","src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix.rs","src/cleanup_plan/replay/factored/canonical_shared/before_selected_labels.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests.rs","src/cleanup_plan/replay/skeleton_work.rs","src/cleanup_plan/replay/skeleton_work/merge_search.rs","src/source_reflows.rs","src/source_reflows/main_f247_controllers.rs","src/source_reflows_merge.rs","src/source_reflows_merge/main67107_inputs.rs","src/source_reflows_merge/main99f_inputs.rs","src/source_reflows_merge/main_e177_inputs.rs","src/source_reflows_merge/next_big_batch_inputs.rs","tests/hir_module_boundaries.rs","tests/wasm/scalar_browser_ci_contract/before_3e66.rs","tests/wasm/scalar_browser_ci_contract/incoming_main3e66.rs","tests/workspace/semantic_graph.rs","tests/workspace/semantic_graph/source_inventory.rs","src/cleanup_plan/replay/branch_clone/tests.rs","src/cleanup_plan/replay/expression_facts/receiving/retained.rs","src/cleanup_plan/replay/factored/canonical_shared.rs","src/cleanup_plan/replay/factored/canonical_shared/arena_tests.rs","src/cleanup_plan/replay/factored/canonical_shared/before_selected_route.rs","src/cleanup_plan/replay/factored/canonical_shared/borrowed_labels.rs","src/cleanup_plan/replay/factored/canonical_shared/empty_memo.rs","src/cleanup_plan/replay/factored/canonical_shared/paid_many/tests.rs","src/cleanup_plan/replay/factored/continuation_receiver.rs","src/cleanup_plan/replay/factored/oracle_hook.rs","src/source_reflows/semantic_receivers.rs","src/cleanup_plan/replay/skeleton_work/enumerated_prefix/tests.rs","platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs","src/workspace_graph/type_proof_tests.rs","src/cleanup_plan.rs","src/cleanup_plan/replay/factored.rs","src/cleanup_plan/replay/factored/canonical_shared/before_lexical_successors.rs","src/cleanup_plan/replay/factored/canonical_shared/lexical_successors/census.rs","src/cleanup_plan/replay/ordinary_plan_history.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor.rs","src/cleanup_plan/replay/owned_suffix.rs","src/cleanup_plan/replay/owned_suffix/tests.rs","src/workspace_graph/expected_projection/held_projection.rs","src/workspace_graph/tests/builder_limits.rs","src/cleanup_plan/replay/expression_facts/arena.rs","src/cleanup_plan/replay/expression_facts/arena/prefix.rs","src/cleanup_plan/replay/expression_facts/arena/prefix/tests.rs","src/cleanup_plan/replay/expression_facts/arena/retained.rs","src/cleanup_plan/replay/expression_facts/arena/retained/tests.rs","src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_tests.rs","src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/retained_lookup.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_census.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_tests.rs","src/source_reflows_merge/main_c883_inputs.rs","src/workspace_graph/tests.rs","crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity.rs","src/bounded_output.rs","src/bounded_output/stage_debits.rs","src/cleanup/schema_summary/tests.rs","src/cleanup_plan/renewal_profile/child_refusal.rs","src/cleanup_plan/replay/branch_clone.rs","src/cleanup_plan/replay/branch_clone/before.rs","src/cleanup_plan/replay/branch_clone/inline_tests.rs","src/cleanup_plan/replay/branch_clone/production_capacity.rs","src/cleanup_plan/replay/branch_clone/production_capacity/tests.rs","src/cleanup_plan/replay/compact_stage.rs","src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix/tests.rs","src/cleanup_plan/replay/expression_facts/receiving/regions_entry/tests.rs","src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/counter_tests.rs","src/cleanup_plan/replay/expression_facts/receiving/retained/case_proof/tests.rs","src/cleanup_plan/replay/factored/canonical_shared/main_unit_weight/unit_weight_tests.rs","src/cleanup_plan/replay/factored/canonical_shared/paid_many/source.rs","src/cleanup_plan/replay/factored/moderate_tests.rs","src/cleanup_plan/replay/factored/oracle_order/tests.rs","src/cleanup_plan/replay/factored/strategy.rs","src/cleanup_plan/replay/final_output/current_observation_tests.rs","src/cleanup_plan/replay/final_output/tests.rs","src/cleanup_plan/replay/live_oracle/source_custody/a.rs","src/cleanup_plan/replay/live_oracle/source_custody/b.rs","src/cleanup_plan/replay/path_array/funded_tests.rs","src/cleanup_plan/replay/path_array/main44/mod.rs","src/cleanup_plan/replay/path_array/main44/source_domain.rs","src/cleanup_plan/replay/path_array/tests.rs","src/cleanup_plan/replay/path_join.rs","src/cleanup_plan/replay/path_join/before_singleton.rs","src/cleanup_plan/replay/physical_budget.rs","src/cleanup_plan/replay/selected_entry/tests/session_errors.rs","src/cleanup_plan/replay/semantic_refusal.rs","src/cleanup_plan/replay/semantic_refusal/tests.rs","src/cleanup_plan/replay/skeleton_work/enumerated_prefix/before.rs","src/cleanup_plan/replay/skeleton_work/merge_keys.rs","src/cleanup_plan/replay/skeleton_work/prefix_fusion.rs","src/cleanup_plan/replay/structural_control/calls.rs","src/diagnostic/before_writer.rs","src/hir/declaration_index/identity_payer.rs","src/hir/expr_nodes.rs","src/hir/ids/fixed_operation_tests.rs","src/hir/lexical_exit.rs","src/hir/resolve_program.rs","src/hir/resolve_program_methods.rs","src/hir/validation.rs","src/loan_plan/paid_live.rs","src/loop_calls/byte_writer/resolved.rs","src/project/json_codec.rs","src/project/mod.rs","src/source_reflows/factored_sources.rs","src/source_reflows/incoming090_semantic_receivers.rs","src/source_reflows/incoming21_semantic_receivers.rs","src/source_reflows/incoming67107_semantic_receivers.rs","src/source_reflows/main043.rs","src/source_reflows/main043/audit_3.rs","src/source_reflows/semantic_receivers_mainb80.rs","src/source_reflows_merge/current344.rs","src/source_reflows_merge/final_ready_inputs.rs","src/source_reflows_merge/main44_custody.rs","src/source_reflows_merge/main67107_inputs_tail.rs","src/source_reflows_merge/main7f8_source_inputs.rs","src/source_reflows_merge/main8a_historical.rs","src/source_reflows_merge/main_eb2_inputs.rs","src/source_reflows_merge/maina3_custody.rs","src/source_reflows_merge/ordinary_plan_inputs.rs","src/source_reflows_merge/runtime_json.rs","src/source_verify/declaration.rs","src/workspace_graph.rs","src/workspace_graph/constructor_failure/receiving/typed/tests.rs","src/workspace_graph/diagnostics.rs","src/workspace_graph/expected_projection/borrowed_imports.rs","src/workspace_graph/expected_projection/borrowed_imports/typed/alias/owned_growth_tests.rs","src/workspace_graph/expected_projection/borrowed_imports/typed/alias/rename_tests.rs","src/workspace_graph/expected_projection/type_rewrite/receiving.rs","src/workspace_graph/expected_projection/uncached_peak.rs","src/workspace_graph/function_alias/before_core.rs","src/workspace_graph/function_alias/before_core/tests.rs","src/workspace_graph/source_schema/nominal_context/main8a.rs","src/workspace_graph/source_schema/static_text/tests.rs","src/workspace_graph/test_replay_context.rs","tests/doctor_production_provisioner_v1.rs","tests/project/standard_library/application_json.rs","tests/quality_routing.rs","tests/source_locked_contracts.rs","tests/support/production_source_inventory.rs","tests/wasm/scalar_browser_ci_contract.rs","tests/wasm/scalar_browser_ci_contract/browser_custody.rs","tests/wasm/scalar_browser_ci_contract/historical_builder.rs","tests/wasm/scalar_browser_ci_contract/historical_main_7f8.rs","docs/evidence/main-e177-final-source-binding-20261017/current-projection-inputs.tsv","src/bounded_output/stage_debits/route_debits.rs","src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified.rs","src/cleanup_plan/replay/factored/canonical_shared/matched_receiving_layers/successor_targets.rs","src/cleanup_plan/replay/factored/compact_nodes/tests.rs","src/hir/ids/declaration_tag.rs","src/hir/ids/declaration_tag/before.rs","src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_cross_phase.rs","src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_cross_phase_before.rs","src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_cross_phase_tests.rs","src/cleanup_plan/replay/inventory_coverage/paid/queries.rs","src/cleanup_plan/replay/inventory_coverage/paid_before.rs","src/cleanup_plan/replay/inventory_coverage/typed_tests/selected_family.rs","src/cleanup_plan/replay/path_join/selected.rs","src/cleanup_plan/replay/path_join/selected/joins.rs","src/cleanup_plan/replay/path_join/selected/lexical_flow.rs","src/cleanup_plan/replay/path_join/selected/queries.rs","src/cleanup_plan/replay/path_join/selected/renewal.rs","src/cleanup_plan/replay/path_join/selected/strings.rs","src/cleanup_plan/replay/path_join/selected/tests/history.rs","src/cleanup_plan/replay/path_join/selected/tests/lookup.rs","src/cleanup_plan/replay/path_join/selected/walk.rs","src/cleanup_plan/replay/schema/selected_paid.rs","src/cleanup_plan/replay/selected_entry/tests/paid/selected_family.rs","src/cleanup_plan/replay/structural_control/selected.rs","src/cleanup_plan/replay/structural_control/selected/blocks.rs","src/cleanup_plan/replay/structural_control/selected/calls.rs","src/cleanup_plan/replay/structural_control/selected/helpers.rs","src/cleanup_plan/replay/structural_control/selected/place_before_formals.rs","src/cleanup_plan/replay/structural_control/selected/tests/boundary.rs","src/cleanup_plan/replay/inventory_coverage.rs","src/cleanup_plan/replay/inventory_coverage/paid.rs","src/cleanup_plan/replay/lexical_replay.rs","src/cleanup_plan/replay/scalar_replay.rs","src/cleanup_plan/replay/selected_context/child.rs","src/iterator_ops.rs"];
#[rustfmt::skip]
const ADDITIONS: &[&str] = &["src/cleanup_plan/renewal_profile.rs","src/cleanup_plan/renewal_profile/funded.rs","src/cleanup_plan/renewal_profile/funded_receiving.rs","src/cleanup_plan/replay/expression_facts/arena/retained/into_tests.rs","src/cleanup_plan/replay/expression_facts/arena/retained_into.rs","src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_tests/receiving.rs","src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/retained_lookup_before_receiving.rs","src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_cross_phase_before_receiving.rs","src/cleanup_plan/replay/ordinary_receiving.rs","src/cleanup_plan/replay/ordinary_receiving/caller_before.rs","src/cleanup_plan/replay/ordinary_receiving/tests.rs","src/cleanup_plan/replay/skeleton_work/dag_storage.rs","src/cleanup_plan/replay/skeleton_work/receiving.rs","src/hir/lexical_exit/owner_funded.rs","src/hir/lexical_exit/owner_funded_receiving.rs","src/literal_format.rs","src/literal_format/funded.rs","src/literal_format/funded_receiving.rs","src/string_ops/conditions.rs","src/string_ops/conditions/funded.rs","src/string_ops/conditions/funded_receiving.rs","src/cleanup_plan/replay/expression_facts/errors.rs","docs/ORDINARY-REPLAY-RECEIVING-V1.md","src/hir/ids/declaration_backing_tests.rs","src/cleanup_plan/replay/factored/before_inline_singleton.rs","src/cleanup_plan/replay/factored/canonical_shared/inline_singleton.rs","src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/inline_exit.rs","src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/inline_exit/census.rs","src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors.rs","src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop.rs","src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/borrowed_single.rs","src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/terminal_leaf.rs","src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/tests.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_paired.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value.rs","src/source_reflows_merge/runtime_json/sealed_io.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_flat_view.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_fused_flat.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/combined_pair_scope_tests.rs","src/ast/program_source/function_alias/scalar_signature.rs","src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_block_before.rs","src/workspace_graph/constructor_failure/before_scalar_dispatch.rs","src/workspace_graph/expected_projection/borrowed_imports/typed/before_scalar_signature.rs","src/workspace_graph/expected_projection/borrowed_imports/typed/scalar_signature_tests.rs","src/ast/program_source/function_alias/before_string.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_retained_tail.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_before_tail.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/matched_tail_tests.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value_before_tail.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/source_format.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source.rs","src/source_reflows_merge/main_e177_historical_before_data.rs","src/source_reflows_merge/main_e177_historical_data.rs","src/source_reflows_merge/main_e177_historical_data_tests.rs","src/source_reflows_merge/main_e177_overlay_fragments_v3.rs","src/workspace_graph/expected_projection/borrowed_imports/typed/before_string_signature.rs","src/workspace_graph/expected_projection/borrowed_imports/typed/string_signature_tests.rs","src/workspace_graph/expected_projection/call_carriers.rs","src/workspace_graph/expected_projection/call_carriers_historical_tests.rs","src/workspace_graph/expected_projection/call_carriers_inverse.rs","src/workspace_graph/expected_projection/call_carriers_original.rs","src/workspace_graph/expected_projection/call_carriers_tests.rs","src/ast/program_source/decl_slices.rs","src/workspace_graph/expected_projection/borrowed_imports/typed.rs","src/workspace_graph/expected_projection/borrowed_imports/typed/alias.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_prepared_pairing.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_before_prepared_links.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/prepared_pairing_tests.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links.rs","src/workspace_graph/expected_projection/call_carriers_before_directory.rs","src/workspace_graph/expected_projection/call_carriers_directory.rs","src/workspace_graph/expected_projection/call_carriers_directory_tests.rs","src/workspace_graph/expected_projection.rs","src/workspace_graph/expected_projection/prelude_forecast_tests.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_identity_prefix.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_before_identity.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value_before_identity.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_tests.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_reachable_primitives.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_reachable_before.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/reachable_pair_scope_tests.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_event_prefix.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_shared_steps.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_shared_steps.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/main99_pair_scope_tests.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value_before_event.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_before_event.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_before_event_tests.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_event_tests.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps.rs","src/cleanup_plan/replay/lexical_replay/operand_tests/source.rs","src/workspace_graph/expected_projection/private_layout/capacity_tests/source.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_certified_heads.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_shared_runs.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_heads.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_heads_tests.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_before_certified.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_shared_runs.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/main7e_pair_scope_tests.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value_before_certified.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_before_certified.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps/shared_boundaries.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps/shared_boundaries/shared_runs.rs","src/workspace_graph/expected_projection/private_layout/registry/scratch_tests.rs","src/workspace_graph/cold_canonical/tests.rs","src/workspace_graph/expected_projection/cost.rs","src/bounded_output/stage_debits/route_debits/json_codec_probe.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_main001.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_main487.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_main_b692.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_pair_links.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_paired_tail.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/borrowed_reader_composition_tests.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_heads_pair_links.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_heads_paired_tail.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_main001.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_main487.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_main_b692.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_pair_links.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_paired_tail.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/main001_pair_scope_tests.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/main487_pair_scope_tests.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/main_b692_pair_scope_tests.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/main_b692_paired_tails.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_link_composition_tests.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_tail_composition_tests.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value_pair_links.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value_paired_tail.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_pair_links.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_paired_tail.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/constructor.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/fused_readers.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/json_ledger.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/paired_tails.rs","src/source_reflows_merge/main_e177_overlay_compiled.rs","src/source_reflows_merge/main_e177_packet_lookup.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_empty_selector.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_heads_before_empty.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/empty_selector_tests.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_before_empty.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value_before_empty.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_before_empty.rs","src/test_allocation_observer/backing.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_final_affine.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_heads_before_final_affine.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/final_affine.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/final_affine_tests.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_before_final_affine.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value_before_final_affine.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_before_final_affine.rs","src/bounded_output/exclusive_debits.rs","src/hir/ids/lowering_identity_tests.rs","src/hir/owned_collection_record/identity_before.rs","src/hir/resolve_expr/sort_keys/identity_before.rs","src/hir/resolve_expr/sort_keys/identity_tests.rs","src/hir/resolve_for/identity_before.rs","src/hir/resolve_for_own/identity_before.rs","src/source_reflows_merge/main_e177_overlay_pins_v9.rs","src/cleanup_plan/replay/ordinary_plan_history/frontier_tests.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/retained_main_289/fixture_helpers.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/retained_main_289/lexical_cursor.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/retained_main_289/mod.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/retained_main_289/pair_scope_census.rs","src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/retained_main_289/pair_scope_tests.rs","src/bounded_output/stage_debits/route_debits/retained_main_289.rs","tests/workspace.rs","src/cleanup_plan/replay/expression_facts.rs","tests/wasm/scalar_browser_ci_contract/historical_main_c11.rs","tests/wasm/scalar_browser_ci_contract/before_524.rs","tests/wasm/scalar_browser_ci_contract/before_671.rs","tests/wasm/scalar_browser_ci_contract/before_7d34.rs","tests/wasm/scalar_browser_ci_contract/before_21abc.rs","src/bounded_output/classifier_string.rs","src/cleanup_plan/replay/skeleton_work/enumerated_prefix.rs","src/cleanup_plan/replay/sequence_vector/funded_tests.rs","src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/compact.rs","src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/compact/compact_targets.rs","src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/compact/payments.rs","src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/tests/compact_results.rs","src/cleanup_plan/replay/final_output.rs","src/cleanup_plan/replay/final_output/compact.rs","src/cleanup_plan/replay/final_output/tests/compact_results.rs","src/cleanup_plan/replay/path_join/boxed_states.rs","src/cleanup_plan/replay/path_join/boxed_states/receiving_tests.rs","src/cleanup_plan/replay/sequence_vector.rs","src/cleanup_plan/replay/skeleton_work/enumerated_prefix/flat_probe.rs","src/source_reflows_merge/final_ready_inputs/projection_publication.rs","src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/compact/active.rs","src/source_reflows_merge/main_e177_overlay_pins_v10.rs","src/cleanup_plan/replay/branch_clone/production_capacity/before_short.rs","src/cleanup_plan/replay/branch_clone/short_plan.rs","src/cleanup_plan/replay/nested_shape.rs","src/cleanup_plan/replay/nested_shape/scalar_tests.rs","src/cleanup_plan/replay/expression_facts/receiving/remaining/tests.rs","tests/wasm/scalar_browser_ci_contract/current_profiles573.rs","src/cleanup_plan/replay/expression_facts/receiving/helpers/tests.rs","src/cleanup_plan/replay/expression_facts/receiving/retained/exits/tests.rs","src/cleanup_plan/replay/workspace_oracle.rs","src/cleanup_plan/replay/factored/paired.rs","src/cleanup_plan/replay/factored/paired/tests.rs","src/cleanup_plan/replay/transition.rs","src/workspace_graph/initial_report/build.rs","docs/AGENT-SKILL-BUNDLE-V1.json","src/cleanup_plan/replay/path_join/main_single_groups/receiver_tests.rs","tests/wasm/scalar_browser_ci_contract/current_profiles3d81.rs","src/cleanup_plan/replay/expression_facts/arena/cursor_tests.rs","tests/workspace/semantic_graph/host_pins.rs","src/workspace_graph/diagnostics/borrowed_help_tests.rs","docs/evidence/borrowed-refusal-help-824/exact-source-inverse.json","src/workspace_graph/expected_projection/call_sites.rs","docs/WORKSPACE-SEMANTIC-GRAPH-V1.md","docs/evidence/borrowed-refusal-help-824/default-message-20261025.json","docs/evidence/compact-replay-h006-transport-20261025/source-coverage-followup.json","src/workspace_graph/expected_projection/identity_prebound_tests.rs","src/workspace_graph/expected_projection/private_layout/authority_invocation/scratch/tests.rs","docs/evidence/prelude-forecast-286/exact-source-inverses.json","docs/evidence/finite-source-overlay-v2-20261018/README.md","src/source_reflows_merge/constructor_stage.rs","docs/CONSTRUCTOR-FAILURE-RECEIVING-V1.md","src/workspace_graph/expected_projection/borrowed_imports/typed/alias/owned_growth.rs","src/workspace_graph/expected_projection/defaults/field_directory.rs","src/vec_ops/wrappers.rs","src/box_ops/wrappers.rs","src/workspace_graph/expected_projection/defaults.rs","src/workspace_graph/expected_projection/import_stub.rs","src/workspace_graph/expected_projection/borrowed_imports/tests/old.rs","src/workspace_graph/expected_projection/borrowed_imports/tests/full/original_construct.rs","src/workspace_graph/expected_projection/borrowed_imports/tests/full/publication.rs","src/ast/program_source/decl_slices/alias_tests.rs","src/ast/program_source/decl_slices/before_alias.rs","src/ast/program_source/function_alias.rs","src/workspace_graph/expected_projection/borrowed_imports/typed/alias/tests.rs","src/hir/identity_resolution/nominal.rs","src/workspace_graph/expected_projection/borrowed_imports/tests.rs","src/workspace_graph/expected_projection/borrowed_imports/tests/full.rs","src/workspace_graph/expected_projection/borrowed_imports_original.rs","src/cleanup_plan/replay/continuation_storage/boxed_frames.rs","src/cleanup_plan/replay/factored/unpaid_continuation_receiver.rs","src/cleanup_plan/replay/expression_facts/receiving/retained/reference_coverage.rs","src/loan_plan/root_directory_before.rs","src/loan_plan/phase_pairs_before.rs"];
const POLICY_PATH: &str = "docs/evidence/finite-source-overlay-v2-20261018/policy.json";
const PAGE_PATH: &str = "docs/evidence/finite-source-overlay-v2-20261018/page.json";
const POLICY_LENGTH: usize = 114753; // Filled from the complete literal policy below.
const POLICY_SHA256: &str = "18e62aecd2fa96463cbb209b2c706f06d0d30ebcb35a0e9fa68d9f4fe8fae9c7";
const EXTENSION_PATH: &str = "docs/evidence/fmt-local-441-20261018/extension.json";
const EXTENSION_LENGTH: usize = 262076;
const EXTENSION_SHA256: &str = "2af7299253e5cfb027186ba4fc151011a937214ff381c9267bfc11ef7d0003b6";
macro_rules! require {
    ($condition:expr, $message:literal) => {
        if !$condition {
            return Err($message);
        }
    };
}
fn keys(value: &Value, expected: &[&str]) -> bool {
    value.as_object().is_some_and(|v| {
        v.len() == expected.len() && expected.iter().all(|key| v.contains_key(*key))
    })
}
fn normal(path: &str) -> bool {
    !path.is_empty()
        && !path.contains(['\\', ':', '\0'])
        && path
            .split('/')
            .all(|p| !p.is_empty() && p != "." && p != "..")
}
fn control(path: &str) -> bool {
    [
        POLICY_PATH,
        PAGE_PATH,
        EXTENSION_PATH,
        "src/source_reflows_merge/main_e177_final_inputs.rs",
        "src/source_reflows_merge/main_e177_overlay_v2.rs",
        "docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py",
        "docs/evidence/main-e177-final-source-binding-20261017/guard_payload.py",
        "docs/evidence/merge-2e92-browser-20261018/guard-config.json",
    ]
    .contains(&path)
}
fn canonical(value: &Value) -> Vec<u8> {
    fn quoted(value: &str, out: &mut String) {
        let bytes = value.as_bytes();
        let mut index = 0;
        while index < bytes.len() {
            let byte = bytes[index];
            if !matches!(byte, 0x20..=0x7e) || matches!(byte, b'"' | b'\\') {
                out.push_str(&serde_json::to_string(value).unwrap());
                return;
            }
            index += 1;
        }
        out.push('"');
        out.push_str(value);
        out.push('"');
    }
    fn write(v: &Value, out: &mut String) {
        match v {
            Value::Object(map) => {
                out.push('{');
                let mut emit = |index: usize, key: &str, value: &Value| {
                    if index != 0 {
                        out.push(',');
                    }
                    quoted(key, out);
                    out.push(':');
                    write(value, out);
                };
                if map.keys().is_sorted() {
                    for (index, (key, value)) in map.iter().enumerate() {
                        emit(index, key, value);
                    }
                } else if map.len() <= 16 {
                    // Borrow every field. Large/unknown dictionaries retain the full fallback.
                    let mut fields = [None; 16];
                    for (slot, entry) in fields.iter_mut().zip(map.iter()) {
                        *slot = Some(entry);
                    }
                    let fields = &mut fields[..map.len()];
                    fields.sort_unstable_by_key(|entry| entry.unwrap().0);
                    for (index, entry) in fields.iter().enumerate() {
                        let (key, value) = entry.unwrap();
                        emit(index, key, value);
                    }
                } else {
                    for (index, (key, value)) in
                        map.iter().collect::<BTreeMap<_, _>>().iter().enumerate()
                    {
                        emit(index, key, value);
                    }
                }
                out.push('}');
            }
            Value::Array(array) => {
                out.push('[');
                for (index, value) in array.iter().enumerate() {
                    if index != 0 {
                        out.push(',');
                    }
                    write(value, out);
                }
                out.push(']');
            }
            Value::String(value) => quoted(value, out),
            Value::Null => out.push_str("null"),
            Value::Bool(value) => out.push_str(if *value { "true" } else { "false" }),
            _ => out.push_str(&serde_json::to_string(v).unwrap()),
        }
    }
    let mut result = String::new();
    write(value, &mut result);
    result.push('\n');
    result.into_bytes()
}
fn json(raw: &str) -> Result<Value, &'static str> {
    let value: Value = serde_json::from_str(raw).map_err(|_| "JSON syntax")?;
    require!(
        canonical(&value) == raw.as_bytes(),
        "noncanonical or duplicate JSON"
    );
    Ok(value)
}
fn pin(value: &Value) -> Result<(usize, &str, &str), &'static str> {
    let length =
        usize::try_from(value["length"].as_u64().ok_or("length")?).map_err(|_| "length")?;
    let sha = value["sha256"].as_str().ok_or("digest")?;
    let path = value["archive"].as_str().ok_or("archive")?;
    require!(
        length <= SOURCE_CAP && normal(path),
        "source bound or archive path"
    );
    require!(
        sha.len() == 64
            && sha
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "digest"
    );
    Ok((length, sha, path))
}
fn image(value: &Value) -> Result<String, &'static str> {
    let (length, sha, path) = pin(value)?;
    held_4a6eb(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join(path),
        length,
        sha,
    )
}
fn page_bytes(
    compiled: &str,
    physical: &str,
    length: usize,
    sha: &str,
) -> Result<(), &'static str> {
    require!(
        length <= PAGE_CAP && physical.len() == length,
        "page bound or length"
    );
    require!(
        compiled == physical && digest(physical.as_bytes()) == sha,
        "page compiled/physical/digest"
    );
    Ok(())
}
struct PageData {
    page: Option<(String, Arc<Value>)>,
}
impl PageData {
    fn parse(&mut self, raw: &str) -> Result<Arc<Value>, &'static str> {
        require!(raw.len() <= PAGE_CAP, "page parse bound");
        if let Some((bytes, parsed)) = &self.page {
            if bytes == raw {
                require!(
                    canonical(parsed) == raw.as_bytes(),
                    "noncanonical or duplicate JSON"
                );
                return Ok(Arc::clone(parsed));
            }
        }
        let parsed = Arc::new(json(raw)?);
        self.page = Some((raw.to_owned(), Arc::clone(&parsed)));
        Ok(parsed)
    }
}
fn page_data(raw: &str) -> Result<Arc<Value>, &'static str> {
    static DATA: Mutex<PageData> = Mutex::new(PageData { page: None });
    DATA.lock().map_err(|_| "page parse data lock")?.parse(raw)
}
fn load() -> Result<(Value, Arc<Value>), &'static str> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let compiled =
        include_str!("../../docs/evidence/finite-source-overlay-v2-20261018/policy.json");
    require!(POLICY_LENGTH <= PAGE_CAP, "policy bound");
    let raw = held_4a6eb(&root.join(POLICY_PATH), POLICY_LENGTH, POLICY_SHA256)?;
    require!(raw == compiled, "policy compiled/physical");
    let policy = json(&raw)?;
    require!(
        keys(
            &policy,
            &[
                "schema",
                "page",
                "predecessor",
                "replacements",
                "additions",
                "new_fragment_edges"
            ]
        ),
        "policy keys"
    );
    require!(
        policy["schema"] == "semaprax.finite-source-overlay-policy.v2",
        "policy schema"
    );
    require!(
        keys(&policy["page"], &["path", "length", "sha256"]) && policy["page"]["path"] == PAGE_PATH,
        "one literal page"
    );
    require!(
        keys(
            &policy["predecessor"],
            &["extension", "rows", "logical_sha256"]
        ),
        "predecessor keys"
    );
    require!(policy["predecessor"]["rows"] == 519, "predecessor rows");
    require!(
        policy["predecessor"]["extension"]
            == serde_json::json!({"path":EXTENSION_PATH,"length":EXTENSION_LENGTH,"sha256":EXTENSION_SHA256}),
        "predecessor extension"
    );
    require!(
        policy["replacements"] == serde_json::json!(REPLACEMENTS),
        "literal replacement scope"
    );
    require!(
        policy["additions"]
            .as_array()
            .ok_or("additions")?
            .iter()
            .map(|r| r["source"].as_str())
            .collect::<Vec<_>>()
            == ADDITIONS.iter().map(|p| Some(*p)).collect::<Vec<_>>(),
        "literal addition scope"
    );
    // Original metadata is read as metadata. It is never substituted for a
    // changed physical Rust source, and it retains its original individual cap.
    let extension = include_str!("../../docs/evidence/fmt-local-441-20261018/extension.json");
    require!(
        held_4a6eb(
            &root.join(EXTENSION_PATH),
            EXTENSION_LENGTH,
            EXTENSION_SHA256
        )? == extension,
        "predecessor physical/compiled"
    );
    let length = usize::try_from(policy["page"]["length"].as_u64().ok_or("page length")?)
        .map_err(|_| "page length")?;
    let sha = policy["page"]["sha256"].as_str().ok_or("page digest")?;
    require!(length <= PAGE_CAP, "page bound");
    let physical = held_4a6eb(&root.join(PAGE_PATH), length, sha)?;
    let compiled = include_str!("../../docs/evidence/finite-source-overlay-v2-20261018/page.json");
    page_bytes(compiled, &physical, length, sha)?;
    Ok((policy, page_data(&physical)?))
}
#[cfg(test)]
fn load_before_page_data() -> Result<(Value, Value), &'static str> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let compiled =
        include_str!("../../docs/evidence/finite-source-overlay-v2-20261018/policy.json");
    require!(POLICY_LENGTH <= PAGE_CAP, "policy bound");
    let raw = held_4a6eb(&root.join(POLICY_PATH), POLICY_LENGTH, POLICY_SHA256)?;
    require!(raw == compiled, "policy compiled/physical");
    let policy = json(&raw)?;
    require!(
        keys(
            &policy,
            &[
                "schema",
                "page",
                "predecessor",
                "replacements",
                "additions",
                "new_fragment_edges"
            ]
        ),
        "policy keys"
    );
    require!(
        policy["schema"] == "semaprax.finite-source-overlay-policy.v2",
        "policy schema"
    );
    require!(
        keys(&policy["page"], &["path", "length", "sha256"]) && policy["page"]["path"] == PAGE_PATH,
        "one literal page"
    );
    require!(
        keys(
            &policy["predecessor"],
            &["extension", "rows", "logical_sha256"]
        ),
        "predecessor keys"
    );
    require!(policy["predecessor"]["rows"] == 519, "predecessor rows");
    require!(
        policy["predecessor"]["extension"]
            == serde_json::json!({"path":EXTENSION_PATH,"length":EXTENSION_LENGTH,"sha256":EXTENSION_SHA256}),
        "predecessor extension"
    );
    require!(
        policy["replacements"] == serde_json::json!(REPLACEMENTS),
        "literal replacement scope"
    );
    require!(
        policy["additions"]
            .as_array()
            .ok_or("additions")?
            .iter()
            .map(|r| r["source"].as_str())
            .collect::<Vec<_>>()
            == ADDITIONS.iter().map(|p| Some(*p)).collect::<Vec<_>>(),
        "literal addition scope"
    );
    // Original metadata is read as metadata. It is never substituted for a
    // changed physical Rust source, and it retains its original individual cap.
    let extension = include_str!("../../docs/evidence/fmt-local-441-20261018/extension.json");
    require!(
        held_4a6eb(
            &root.join(EXTENSION_PATH),
            EXTENSION_LENGTH,
            EXTENSION_SHA256
        )? == extension,
        "predecessor physical/compiled"
    );
    let length = usize::try_from(policy["page"]["length"].as_u64().ok_or("page length")?)
        .map_err(|_| "page length")?;
    let sha = policy["page"]["sha256"].as_str().ok_or("page digest")?;
    require!(length <= PAGE_CAP, "page bound");
    let physical = held_4a6eb(&root.join(PAGE_PATH), length, sha)?;
    let compiled = include_str!("../../docs/evidence/finite-source-overlay-v2-20261018/page.json");
    page_bytes(compiled, &physical, length, sha)?;
    Ok((policy, json(&physical)?))
}
fn edges(packet: &Value) -> Result<Vec<(String, String)>, &'static str> {
    let rows = packet["rows"].as_array().ok_or("rows")?;
    let by: BTreeMap<_, _> = rows
        .iter()
        .map(|r| (r["source"].as_str().unwrap_or(""), r))
        .collect();
    require!(
        by.len() == rows.len() && !by.contains_key(""),
        "duplicate source"
    );
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    for row in rows {
        if let Some(children) = row.get("fragments") {
            for child in children.as_array().ok_or("fragments")? {
                let source = child["source"].as_str().ok_or("fragment source")?;
                require!(
                    child.get("fragments").is_none() && child["compiled"] == true,
                    "noncompiled or nested fragment"
                );
                require!(
                    by.get(source) == Some(&child) && row["source"] != source,
                    "fragment canonical row or self edge"
                );
                let edge = (
                    row["source"].as_str().ok_or("parent")?.to_owned(),
                    source.to_owned(),
                );
                require!(seen.insert(edge.clone()), "duplicate fragment edge");
                result.push(edge);
            }
        }
    }
    Ok(result)
}
#[path = "main_e177_overlay_fragments_v3.rs"]
mod fragments_v3;
fn compose_values(before: Value, policy: &Value, page: &Value) -> Result<Value, &'static str> {
    if page["schema"] == fragments_v3::SCHEMA_V11 {
        let expanded = fragments_v3::expand_v11(&before, policy, page)?;
        return compose_values_v2(before, policy, &expanded);
    }
    if page["schema"] == fragments_v3::SCHEMA_V10 {
        let expanded = fragments_v3::expand_v10(&before, policy, page)?;
        compose_values_v2(before, policy, &expanded)
    } else if page["schema"] == fragments_v3::SCHEMA_V9 {
        let expanded = fragments_v3::expand_v9(&before, policy, page)?;
        compose_values_v2(before, policy, &expanded)
    } else if page["schema"] == fragments_v3::SCHEMA_V8 {
        let expanded = fragments_v3::expand_v8(&before, policy, page)?;
        compose_values_v2(before, policy, &expanded)
    } else if page["schema"] == fragments_v3::SCHEMA_V7 {
        let expanded = fragments_v3::expand_v7(&before, policy, page)?;
        compose_values_v2(before, policy, &expanded)
    } else if page["schema"] == fragments_v3::SCHEMA_V6 {
        let expanded = fragments_v3::expand_v6(&before, policy, page)?;
        compose_values_v2(before, policy, &expanded)
    } else if page["schema"] == fragments_v3::SCHEMA_V5 {
        let expanded = fragments_v3::expand_v5(&before, policy, page)?;
        compose_values_v2(before, policy, &expanded)
    } else if page["schema"] == fragments_v3::SCHEMA_V4 {
        let expanded = fragments_v3::expand_v4(&before, policy, page)?;
        compose_values_v2(before, policy, &expanded)
    } else if page["schema"] == fragments_v3::SCHEMA {
        let expanded = fragments_v3::expand(&before, policy, page)?;
        compose_values_v2(before, policy, &expanded)
    } else {
        compose_values_v2(before, policy, page)
    }
}
fn compose_values_v2(before: Value, policy: &Value, page: &Value) -> Result<Value, &'static str> {
    require!(
        keys(
            page,
            &[
                "schema",
                "index",
                "total_pages",
                "predecessor",
                "replacements",
                "additions"
            ]
        ),
        "page keys"
    );
    require!(
        page["schema"] == "semaprax.finite-source-overlay-page.v2"
            && page["index"] == 1
            && page["total_pages"] == 1,
        "one page schema/index"
    );
    require!(
        page["predecessor"] == policy["predecessor"],
        "wrong predecessor"
    );
    require!(
        digest(&canonical(&before)) == policy["predecessor"]["logical_sha256"],
        "whole predecessor logical digest"
    );
    let old = before["rows"].as_array().ok_or("old rows")?;
    require!(
        old.len() as u64 == policy["predecessor"]["rows"].as_u64().ok_or("old count")?,
        "old count"
    );
    require!(
        before["source_cap"].as_u64() == Some(SOURCE_CAP as u64),
        "unchanged source cap domain"
    );
    let mut result = before.clone();
    let rows = result["rows"].as_array_mut().ok_or("rows")?;
    let positions: BTreeMap<_, _> = old
        .iter()
        .enumerate()
        .map(|(i, r)| (r["source"].as_str().unwrap_or(""), i))
        .collect();
    require!(
        positions.len() == old.len() && !positions.contains_key(""),
        "old uniqueness"
    );
    let replacements = page["replacements"].as_array().ok_or("replacement rows")?;
    let paths = policy["replacements"]
        .as_array()
        .ok_or("replacement paths")?;
    require!(paths.len() == replacements.len(), "missing replacement");
    let mut previous = None;
    let mut seen = BTreeSet::new();
    for (row, path) in replacements.iter().zip(paths) {
        let source = path.as_str().ok_or("replacement path")?;
        require!(
            normal(source) && !control(source) && row["source"] == source && seen.insert(source),
            "foreign/duplicate/control source"
        );
        let index = *positions.get(source).ok_or("foreign replacement")?;
        require!(previous.is_none_or(|p| p < index), "reordered replacements");
        previous = Some(index);
        let prior = &old[index];
        require!(
            row["compiled"] == prior["compiled"],
            "classification changed"
        );
        let views = row["views"].as_array().ok_or("views")?;
        let old_views = prior["views"].as_array().ok_or("old views")?;
        require!(
            views.len() >= old_views.len() && &views[..old_views.len()] == old_views.as_slice(),
            "old views lost/reordered/changed"
        );
        pin(&row["current"])?;
        for view in &views[old_views.len()..] {
            pin(view)?;
        }
        if row["current"] != prior["current"] {
            let mut retained = prior["current"].clone();
            retained["name"] = "whole-pre-overlay-v2-current".into();
            require!(
                views[old_views.len()..].contains(&retained),
                "old current descriptor lost"
            );
        }
        let mut restored = row.clone();
        restored["current"] = prior["current"].clone();
        restored["views"] = prior["views"].clone();
        if let Some(children) = prior.get("fragments") {
            restored["fragments"] = children.clone();
        } else {
            restored.as_object_mut().ok_or("row")?.remove("fragments");
        }
        require!(
            &restored == prior,
            "whole replacement row inverse/unknown fields"
        );
        rows[index] = row.clone();
    }
    let additions = page["additions"].as_array().ok_or("addition rows")?;
    let bindings = policy["additions"].as_array().ok_or("addition bindings")?;
    require!(additions.len() == bindings.len(), "missing addition");
    for (row, binding) in additions.iter().zip(bindings) {
        require!(keys(binding, &["source", "compiled"]), "addition binding");
        let source = binding["source"].as_str().ok_or("addition source")?;
        require!(
            normal(source)
                && !control(source)
                && row["source"] == source
                && !positions.contains_key(source)
                && seen.insert(source),
            "foreign/duplicate addition"
        );
        require!(
            binding["compiled"].is_boolean() && row["compiled"] == binding["compiled"],
            "addition classification"
        );
        pin(&row["current"])?;
        for view in row["views"].as_array().ok_or("addition views")? {
            pin(view)?;
        }
        rows.push(row.clone());
    }
    require!(
        rows.len()
            == old
                .len()
                .checked_add(additions.len())
                .ok_or("count overflow")?,
        "count"
    );
    let prior_edges = edges(&before)?;
    let current_edges = edges(&result)?;
    require!(
        current_edges
            .iter()
            .filter(|e| prior_edges.contains(e))
            .cloned()
            .collect::<Vec<_>>()
            == prior_edges,
        "old fragment edges lost/reordered"
    );
    let additions = current_edges
        .iter()
        .filter(|e| !prior_edges.contains(e))
        .map(|(parent, child)| serde_json::json!({"parent":parent,"child":child}))
        .collect::<Vec<_>>();
    require!(
        policy["new_fragment_edges"] == Value::Array(additions),
        "undeclared fragment edges"
    );
    let mut restored = result.clone();
    restored["rows"] = before["rows"].clone();
    require!(restored == before, "ENTIRE predecessor object inverse");
    Ok(result)
}
pub(super) fn compose(before: Value) -> Value {
    let (policy, page) = load().unwrap();
    compose_values(before, &policy, &page).unwrap()
}
fn affected(policy: &Value, source: &str) -> bool {
    policy["replacements"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p == source)
        || policy["additions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["source"] == source)
}
fn current_identity(
    row: &Value,
    physical: &str,
    compiled: Option<&str>,
    archive: &str,
) -> Result<(), &'static str> {
    if row["compiled"] == true {
        require!(
            compiled == Some(physical),
            "complete current compiled source"
        );
    }
    require!(archive == physical, "complete current archive");
    Ok(())
}
fn current_tree(
    row: &Value,
    read_source: &impl Fn(&Value) -> Result<String, &'static str>,
    compiled: &impl Fn(&str) -> Option<&'static str>,
    read_archive: &impl Fn(&Value) -> Result<String, &'static str>,
) -> Result<String, &'static str> {
    let raw = read_source(row)?;
    let source = row["source"].as_str().ok_or("source")?;
    current_identity(row, &raw, compiled(source), &read_archive(&row["current"])?)?;
    for child in row
        .get("fragments")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        current_tree(child, read_source, compiled, read_archive)?;
    }
    Ok(raw)
}
fn current(row: &Value) -> Result<String, &'static str> {
    current_tree(
        row,
        &|row| {
            let (length, sha, _) = pin(&row["current"])?;
            held_4a6eb(
                &Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join(row["source"].as_str().ok_or("source")?),
                length,
                sha,
            )
        },
        &super::compiled,
        &image,
    )
}
fn affected_row(source: &str) -> Option<Value> {
    if !REPLACEMENTS.contains(&source) && !ADDITIONS.contains(&source) {
        return None;
    }
    let (policy, page) = load().unwrap();
    if !affected(&policy, source) {
        return None;
    }
    let packet = compose_values(super::packet_before_overlay(), &policy, &page).unwrap();
    Some(
        packet["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["source"] == source)
            .unwrap()
            .clone(),
    )
}
pub(super) fn held_source(
    path: &Path,
    length: usize,
    expected: &str,
) -> Option<Result<String, &'static str>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let relative = path.strip_prefix(root).ok()?.to_str()?;
    if !normal(relative) {
        return None;
    }
    let row = affected_row(relative)?;
    Some((|| {
        let actual = current(&row)?;
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
pub(super) fn held_source_with_packet(
    authenticated: &Value,
    path: &Path,
    length: usize,
    expected: &str,
) -> Option<Result<String, &'static str>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let relative = path.strip_prefix(root).ok()?.to_str()?;
    if !normal(relative) {
        return None;
    } // compose() authenticated policy membership against these exact constants.
    if !REPLACEMENTS.contains(&relative) && !ADDITIONS.contains(&relative) {
        return None;
    }
    let rows = authenticated["rows"].as_array().unwrap();
    let row = rows.iter().find(|row| row["source"] == relative).unwrap();
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
fn stage(row: &Value) -> Option<Value> {
    let source = row["source"].as_str().unwrap();
    let current_row = affected_row(source)?;
    let canonical = super::clippy_stage::rows();
    assert_eq!(
        canonical["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["source"] == source),
        Some(row),
        "closed original stage row"
    );
    current(&current_row).unwrap();
    Some(current_row)
}
pub(super) fn stage_current(row: &Value) -> Option<String> {
    stage(row)?;
    Some(image(&row["current"]).unwrap())
}
pub(super) fn stage_normalize(row: &Value, supplied: &str) -> Option<String> {
    let current_row = stage(row)?;
    let raw = current(&current_row).unwrap();
    if supplied == raw {
        Some(image(&row["before"]).unwrap())
    } else {
        None
    }
}
pub(super) fn stage_requested(
    row: &Value,
    expected: &str,
    supplied: &str,
) -> Option<Option<String>> {
    let current_row = stage(row)?;
    let raw = current(&current_row).unwrap();
    let selected = [&row["before"], &row["current"]]
        .into_iter()
        .chain(
            row.get("views")
                .and_then(Value::as_array)
                .into_iter()
                .flatten(),
        )
        .find(|p| p["sha256"] == expected);
    Some(selected.and_then(|p| {
        let prior = image(p).unwrap();
        if supplied == raw || supplied == prior {
            Some(prior)
        } else {
            None
        }
    }))
}
#[path = "main_e177_overlay_compiled.rs"]
#[rustfmt::skip]
mod compiled_sources;
pub(super) fn compiled(source: &str) -> Option<&'static str> {
    compiled_sources::compiled(source)
}
pub(super) fn value_direct(source: &str, expected: &str, supplied: &str) -> String {
    // A whole logical-table view is selected only after its actual current physical, compiled and archive channels have authenticated. Preserve the complete historical stage fallback below for every other input.
    let direct = super::value_direct_4a6eb(source, expected, supplied);
    if digest(direct.as_bytes()) == expected {
        return direct;
    }
    if let Some(value) = super::clippy_stage::requested(source, expected, supplied) {
        let _ = super::packet();
        return value;
    }
    let normalized = super::clippy_stage::normalize(source, supplied);
    let result = super::value_direct_4a6eb(source, expected, normalized.as_ref());
    if normalized.as_ref() != supplied
        && result == normalized.as_ref()
        && digest(result.as_bytes()) != expected
    {
        supplied.to_owned()
    } else {
        result
    }
}
// Closed policy membership only; current source/archive bytes are never retained.
pub(super) fn declares_source(source: &str) -> bool {
    REPLACEMENTS.contains(&source) || ADDITIONS.contains(&source)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn full_before_canonical_serializer(value: &Value) -> Vec<u8> {
        fn write(v: &Value, out: &mut String) {
            match v {
                Value::Object(map) => {
                    out.push('{');
                    for (i, (key, value)) in
                        map.iter().collect::<BTreeMap<_, _>>().iter().enumerate()
                    {
                        if i != 0 {
                            out.push(',');
                        }
                        out.push_str(&serde_json::to_string(key).unwrap());
                        out.push(':');
                        write(value, out);
                    }
                    out.push('}');
                }
                Value::Array(array) => {
                    out.push('[');
                    for (i, value) in array.iter().enumerate() {
                        if i != 0 {
                            out.push(',');
                        }
                        write(value, out);
                    }
                    out.push(']');
                }
                _ => out.push_str(&serde_json::to_string(v).unwrap()),
            }
        }
        let mut result = String::new();
        write(value, &mut result);
        result.push('\n');
        result.into_bytes()
    }

    #[test]
    fn canonical_writer_matches_full_before_for_all_json_types_and_unknown_fields() {
        let special = serde_json::json!({
            "all_ascii": (0u8..=127).map(char::from).collect::<String>(),
            "unicode": "é😀\u{2028}",
            "escaped": "\0\n\r\t\"\\",
            "numbers": [-0.0, 1.25e90, i64::MIN as f64],
            "booleans": [true, false],
            "nothing": null,
        });
        for width in 0..=128 {
            for reverse in [false, true] {
                let mut fields = serde_json::Map::new();
                for offset in 0..width {
                    let index = if reverse { width - offset - 1 } else { offset };
                    fields.insert(format!("unknown-{index:04}"), special.clone());
                }
                let value = Value::Object(fields);
                let actual = canonical(&value);
                assert_eq!(actual, full_before_canonical_serializer(&value));
                assert_eq!(serde_json::from_slice::<Value>(&actual).unwrap(), value);
            }
        }
        let mut exotic_keys = serde_json::Map::new();
        for key in ["", "\0\n\r\t\"\\", "é😀\u{2028}"] {
            exotic_keys.insert(key.to_owned(), special.clone());
        }
        let exotic_keys = Value::Object(exotic_keys);
        assert_eq!(
            canonical(&exotic_keys),
            full_before_canonical_serializer(&exotic_keys)
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&canonical(&exotic_keys)).unwrap(),
            exotic_keys
        );
        let mut nested = special;
        for _ in 0..64 {
            nested = serde_json::json!([nested]);
        }
        assert_eq!(
            canonical(&nested),
            full_before_canonical_serializer(&nested)
        );
    }
    #[test]
    fn canonical_writer_preserves_every_field_of_complete_current_metadata_files() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        for path in [
            "docs/evidence/finite-source-overlay-v2-20261018/page.json",
            "docs/evidence/finite-source-overlay-v2-20261018/policy.json",
            "docs/evidence/a3-e47-final-custody-20261016/hir.json",
            "docs/evidence/a3-e47-final-custody-20261016/projection.json",
        ] {
            let raw = std::fs::read(root.join(path)).unwrap();
            let value: Value = serde_json::from_slice(&raw).unwrap();
            let actual = canonical(&value);
            assert_eq!(actual, full_before_canonical_serializer(&value), "{path}");
            assert_eq!(
                serde_json::from_slice::<Value>(&actual).unwrap(),
                value,
                "{path}"
            );
        }
    }

    #[test]
    fn logical_current_view_precedes_stage_normalization_and_preserves_foreign_refusals() {
        let source = "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs";
        let expected = "700318b054f35e3dbf69cf25d7a83cd3bf65d04f5674327bcbbd5af02bbdec66";
        let current = super::super::compiled(source).unwrap();
        let previous = value_direct(source, expected, current);
        assert_eq!(digest(previous.as_bytes()), expected);
        assert_ne!(previous, current);
        assert_eq!(value_direct(source, expected, &previous), previous);
        for bad in [
            String::new(),
            format!("{current} "),
            current.get(..current.len() / 2).unwrap_or("").to_owned(),
        ] {
            assert_eq!(value_direct(source, expected, &bad), bad);
        }
    }
    fn record(source: &str, value: &str) -> Value {
        serde_json::json!({"source":source,"compiled":false,
            "current":{"length":value.len(),"sha256":digest(value.as_bytes()),"archive":format!("e/{}",digest(value.as_bytes()))},
            "views":[],"unknown_metadata":{"retained":[3,1,2]}})
    }
    fn fixture() -> (Value, Value, Value) {
        let before = serde_json::json!({"schema":"fixture","source_cap":SOURCE_CAP,
            "unknown_root":{"order":[4,2,1]},"rows":[record("src/one.rs","old"),record("src/two.rs","two")]});
        let predecessor = serde_json::json!({"extension":{"path":EXTENSION_PATH,"length":EXTENSION_LENGTH,"sha256":EXTENSION_SHA256},"rows":2,"logical_sha256":digest(&canonical(&before))});
        let mut row = before["rows"][0].clone();
        row["current"] = record("src/one.rs", "new")["current"].clone();
        let mut old = before["rows"][0]["current"].clone();
        old["name"] = "whole-pre-overlay-v2-current".into();
        row["views"].as_array_mut().unwrap().push(old);
        let page = serde_json::json!({"schema":"semaprax.finite-source-overlay-page.v2","index":1,"total_pages":1,"predecessor":predecessor,"replacements":[row],"additions":[record("src/three.rs","three")]});
        let policy = serde_json::json!({"schema":"semaprax.finite-source-overlay-policy.v2","predecessor":predecessor,"replacements":["src/one.rs"],"additions":[{"source":"src/three.rs","compiled":false}],"new_fragment_edges":[]});
        (before, policy, page)
    }
    #[test]
    fn finite_page_retains_the_entire_predecessor_object_and_unknown_fields() {
        let (before, policy, page) = fixture();
        let result = compose_values(before.clone(), &policy, &page).unwrap();
        assert_eq!(result["rows"][1], before["rows"][1]);
        assert_eq!(
            result["rows"][0]["unknown_metadata"],
            before["rows"][0]["unknown_metadata"]
        );
        assert_eq!(result["unknown_root"], before["unknown_root"]);
        assert_eq!(
            result["rows"][0]["views"][0]["archive"],
            before["rows"][0]["current"]["archive"]
        );
        let mut inverse = result;
        inverse["rows"] = before["rows"].clone();
        assert_eq!(inverse, before);
    }
    #[test]
    fn finite_page_refuses_wrong_schema_predecessor_scope_order_and_unknown_keys() {
        let (before, policy, page) = fixture();
        let mut bad = Vec::new();
        for (key, value) in [
            ("schema", serde_json::json!("foreign")),
            ("index", serde_json::json!(2)),
            ("total_pages", serde_json::json!(2)),
            ("unexpected", serde_json::json!(true)),
        ] {
            let mut p = page.clone();
            p[key] = value;
            bad.push(p);
        }
        let mut p = page.clone();
        p["predecessor"]["logical_sha256"] = "0".repeat(64).into();
        bad.push(p);
        let mut p = page.clone();
        p["replacements"] = serde_json::json!([]);
        bad.push(p);
        let mut p = page.clone();
        p["replacements"]
            .as_array_mut()
            .unwrap()
            .push(page["replacements"][0].clone());
        bad.push(p);
        for source in [
            "src/foreign.rs",
            "../src/one.rs",
            "/src/one.rs",
            "src//one.rs",
            PAGE_PATH,
        ] {
            let mut p = page.clone();
            p["replacements"][0]["source"] = source.into();
            bad.push(p);
        }
        for p in bad {
            assert!(compose_values(before.clone(), &policy, &p).is_err());
        }
        let mut reordered_policy = policy.clone();
        reordered_policy["replacements"] = serde_json::json!(["src/two.rs", "src/one.rs"]);
        let mut reordered = page;
        reordered["replacements"] = serde_json::json!([
            before["rows"][1].clone(),
            reordered["replacements"][0].clone()
        ]);
        assert!(compose_values(before, &reordered_policy, &reordered).is_err());
    }
    #[test]
    fn finite_page_refuses_lost_views_old_current_classification_and_unknown_fields() {
        let (before, policy, page) = fixture();
        let mut bad = Vec::new();
        let mut p = page.clone();
        p["replacements"][0]["views"] = serde_json::json!([]);
        bad.push(p);
        let mut p = page.clone();
        p["replacements"][0]["views"][0]["archive"] = "e/same-payload-different-channel".into();
        bad.push(p);
        let mut p = page.clone();
        p["replacements"][0]["compiled"] = true.into();
        bad.push(p);
        let mut p = page.clone();
        p["replacements"][0]
            .as_object_mut()
            .unwrap()
            .remove("unknown_metadata");
        bad.push(p);
        let mut p = page.clone();
        p["replacements"][0]["unknown_metadata"]["retained"] = serde_json::json!([1, 2, 3]);
        bad.push(p);
        let mut p = page.clone();
        p["additions"][0]["compiled"] = true.into();
        bad.push(p);
        for p in bad {
            assert!(compose_values(before.clone(), &policy, &p).is_err());
        }
        let mut old = before;
        old["rows"][0]["views"] = serde_json::json!([{"name":"first"},{"name":"second"}]);
        let mut p = page;
        p["replacements"][0]["views"]
            .as_array_mut()
            .unwrap()
            .insert(0, serde_json::json!({"name":"second"}));
        p["replacements"][0]["views"]
            .as_array_mut()
            .unwrap()
            .insert(1, serde_json::json!({"name":"first"}));
        let mut policy = policy;
        policy["predecessor"]["logical_sha256"] = digest(&canonical(&old)).into();
        p["predecessor"] = policy["predecessor"].clone();
        assert!(compose_values(old, &policy, &p).is_err());
    }
    #[test]
    fn finite_page_refuses_nonleaf_duplicate_forged_and_undeclared_fragments() {
        let (before, policy, page) = fixture();
        for child in [
            serde_json::json!({"source":"src/one.rs","compiled":true}),
            serde_json::json!({"source":"src/missing.rs","compiled":true}),
            page["additions"][0].clone(),
        ] {
            let mut p = page.clone();
            p["replacements"][0]["fragments"] = serde_json::json!([child]);
            assert!(compose_values(before.clone(), &policy, &p).is_err());
        } // A genuine declared leaf edge has to use the complete same-scope row.
        let mut p = page;
        p["additions"][0]["compiled"] = true.into();
        let child = p["additions"][0].clone();
        p["replacements"][0]["fragments"] = serde_json::json!([child]);
        let mut policy = policy;
        policy["additions"][0]["compiled"] = true.into();
        policy["new_fragment_edges"] =
            serde_json::json!([{"parent":"src/one.rs","child":"src/three.rs"}]);
        assert!(compose_values(before.clone(), &policy, &p).is_ok());
        p["replacements"][0]["fragments"][0]["fragments"] = serde_json::json!([]);
        assert!(compose_values(before, &policy, &p).is_err());
    }
    #[test]
    fn finite_page_exact_bound_compiled_physical_and_json_hostiles_are_independent() {
        let raw = "x".repeat(PAGE_CAP);
        let sha = digest(raw.as_bytes());
        assert!(page_bytes(&raw, &raw, PAGE_CAP, &sha).is_ok());
        assert!(page_bytes(&raw, &raw, PAGE_CAP + 1, &sha).is_err());
        assert!(page_bytes(&raw, &raw[..raw.len() - 1], PAGE_CAP, &sha).is_err());
        assert!(page_bytes("different", &raw, PAGE_CAP, &sha).is_err());
        assert!(page_bytes(&raw, &raw, PAGE_CAP, &"0".repeat(64)).is_err());
        assert!(json("{\"a\":1,\"a\":1}\n").is_err());
        assert!(json("{ \"a\":1}\n").is_err());
        assert!(json("{\"a\":1}\n ").is_err());
        assert!(json("{\"a\":1}\n").is_ok());
    }
    #[test]
    fn finite_page_source_cap_is_per_full_source_and_not_an_aggregate_budget() {
        let (before, policy, mut page) = fixture();
        page["replacements"][0]["current"]["length"] = (SOURCE_CAP + 1).into();
        assert!(compose_values(before.clone(), &policy, &page).is_err());
        page["replacements"][0]["current"]["length"] = SOURCE_CAP.into();
        page["additions"][0]["current"]["length"] = SOURCE_CAP.into();
        assert!(
            compose_values(before, &policy, &page).is_ok(),
            "two separately bounded sources do not share a new aggregate cap"
        );
    }
    #[test]
    fn finite_page_literal_policy_replays_the_complete_519_metadata_inverse() {
        let before = super::super::packet_before_overlay();
        let mut after = compose(before.clone());
        after["rows"] = before["rows"].clone();
        assert_eq!(after, before);
        let (policy, page) = load().unwrap();
        // Decode only the new wire envelope before the unchanged full predecessor checks.
        let page = if page["schema"] == fragments_v3::SCHEMA_V11 {
            fragments_v3::expand_v11(&before, &policy, &page).unwrap()
        } else {
            (*page).clone()
        };
        assert_eq!(page["predecessor"], policy["predecessor"]);
        assert_eq!(page["total_pages"], 1);
    }
    #[test]
    fn finite_page_current_compiled_and_archive_channels_are_required_before_history() {
        let mut row = record("src/one.rs", "new");
        row["compiled"] = true.into();
        assert!(current_identity(&row, "new", Some("new"), "new").is_ok());
        for (physical, compiled, archive) in [
            ("old", Some("new"), "new"),
            ("new", Some("old"), "new"),
            ("new", None, "new"),
            ("new", Some("new"), "old"),
        ] {
            assert!(current_identity(&row, physical, compiled, archive).is_err());
        }
        let mut root = record("src/root.rs", "root");
        root["compiled"] = true.into();
        let mut child = record("src/leaf.rs", "new");
        child["compiled"] = true.into();
        root["fragments"] = serde_json::json!([child]);
        for broken in ["physical", "compiled", "archive"] {
            let history = std::cell::Cell::new(false);
            let source = |r: &Value| {
                Ok(if r["source"] == "src/root.rs" {
                    "root".to_owned()
                } else if broken == "physical" {
                    "old".to_owned()
                } else {
                    "new".to_owned()
                })
            };
            let compiled = |path: &str| {
                Some(if path == "src/root.rs" {
                    "root"
                } else if broken == "compiled" {
                    "old"
                } else {
                    "new"
                })
            };
            let archive = |p: &Value| {
                Ok(if p["sha256"] == digest(b"root") {
                    "root".to_owned()
                } else if broken == "archive" {
                    "old".to_owned()
                } else {
                    "new".to_owned()
                })
            };
            let result = current_tree(&root, &source, &compiled, &archive).map(|_| {
                history.set(true);
                "historical"
            });
            assert!(result.is_err());
            assert!(
                !history.get(),
                "a changed leaf must refuse before historical selection"
            );
        }
    }
    #[test]
    fn finite_page_original_held_reader_keeps_nonregular_and_symlink_refusals() {
        let path = std::env::temp_dir().join(format!("semaprax-overlay-{}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        assert!(held_4a6eb(&path, 0, &digest(b"")).is_err());
        #[cfg(unix)]
        {
            let target = path.join("target");
            std::fs::write(&target, "actual").unwrap();
            let link = path.join("link");
            std::os::unix::fs::symlink(&target, &link).unwrap();
            assert!(held_4a6eb(&link, 6, &digest(b"actual")).is_err());
        }
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn finite_page_parsed_data_reuses_only_exact_complete_bytes_and_rechecks_canonical_form() {
        let mut data = PageData { page: None };
        let a = "{\"a\":1}\n";
        let b = "{\"a\":2}\n";
        let first = data.parse(a).unwrap();
        let same = data.parse(a).unwrap();
        assert!(Arc::ptr_eq(&first, &same));
        for hostile in [
            "{\"a\":1,\"a\":1}\n",
            "{ \"a\":1}\n",
            "{\"a\":1}\n ",
            "not JSON",
        ] {
            assert_eq!(data.parse(hostile).unwrap_err(), json(hostile).unwrap_err());
            assert!(Arc::ptr_eq(&first, &data.parse(a).unwrap()));
        }
        let changed = data.parse(b).unwrap();
        assert!(!Arc::ptr_eq(&first, &changed));
        assert_eq!(*changed, json(b).unwrap());
        assert_eq!(
            *first,
            json(a).unwrap(),
            "retained callers see immutable old data"
        );
        assert_eq!(data.page.as_ref().unwrap().0, b, "one retained entry only");
        let again = data.parse(a).unwrap();
        assert!(
            !Arc::ptr_eq(&first, &again),
            "evicted bytes really parse again"
        ); // Even a same-key forged DATA object cannot substitute a verdict.
        data.page = Some((a.to_owned(), Arc::new(serde_json::json!({"a":2}))));
        assert_eq!(data.parse(a).unwrap_err(), "noncanonical or duplicate JSON");
        let raw = format!(
            "{{\"a\":\"{}\"}}\n",
            "x".repeat(PAGE_CAP - b"{\"a\":\"\"}\n".len())
        );
        data.parse(&raw).unwrap();
        assert_eq!(data.parse(&(raw + " ")).unwrap_err(), "page parse bound");
    }
    #[test]
    fn finite_page_warm_data_never_skips_fresh_regular_read_eof_utf8_or_digest_refusal() {
        let path =
            std::env::temp_dir().join(format!("semaprax-overlay-data-{}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        let source = path.join("page.json");
        let raw = "{\"a\":1}\n";
        let sha = digest(raw.as_bytes());
        let read = |data: &mut PageData| -> Result<Arc<Value>, &'static str> {
            let physical = held_4a6eb(&source, raw.len(), &sha)?;
            page_bytes(raw, &physical, raw.len(), &sha)?;
            data.parse(&physical)
        };
        let mut data = PageData { page: None };
        std::fs::write(&source, raw).unwrap();
        let first = read(&mut data).unwrap();
        assert!(Arc::ptr_eq(&first, &read(&mut data).unwrap()));
        for bad in [
            b"{\"a\":2}\n".as_slice(),
            b"{\"a\":1}",
            b"{\"a\":1}\n ",
            &[255; 8],
        ] {
            std::fs::write(&source, bad).unwrap();
            let expected = held_4a6eb(&source, raw.len(), &sha).unwrap_err();
            assert_eq!(read(&mut data).unwrap_err(), expected);
        }
        std::fs::remove_file(&source).unwrap();
        assert!(
            read(&mut data).is_err(),
            "missing source never uses cached data"
        );
        std::fs::create_dir(&source).unwrap();
        assert!(
            read(&mut data).is_err(),
            "nonregular source never uses cached data"
        );
        std::fs::remove_dir(&source).unwrap();
        #[cfg(unix)]
        {
            let target = path.join("target");
            std::fs::write(&target, raw).unwrap();
            std::os::unix::fs::symlink(&target, &source).unwrap();
            assert!(read(&mut data).is_err(), "warm symlink remains refused");
            std::fs::remove_file(&source).unwrap();
        }
        std::fs::write(&source, raw).unwrap();
        assert!(Arc::ptr_eq(&first, &read(&mut data).unwrap())); // Matching physical bytes with foreign compiled bytes are still refused.
        assert!(page_bytes("{\"a\":2}\n", raw, raw.len(), &sha).is_err());
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn finite_page_complete_old_load_and_every_live_check_remain_exact() {
        let source = include_str!("main_e177_overlay_v2.rs");
        let old = source
            .split_once("fn load_before_page_data()")
            .unwrap()
            .1
            .split_once("fn edges(")
            .unwrap()
            .0;
        let restored = format!("fn load(){old}");
        assert_eq!(
            digest(restored.as_bytes()),
            "cbb41edc5fda3c84c595601c511bd69ccfbbc160a6793ff4cc4848d1f0e70c9c"
        );
        let current = source
            .split_once("fn load()")
            .unwrap()
            .1
            .split_once("#[cfg(test)]\nfn load_before_page_data()")
            .unwrap()
            .0;
        let inverse = format!("fn load(){current}")
            .replacen("Result<(Value, Arc<Value>)", "Result<(Value, Value)", 1)
            .replacen(
                "Ok((policy, page_data(&physical)?))",
                "Ok((policy, json(&physical)?))",
                1,
            );
        assert_eq!(
            inverse, restored,
            "all live I/O/config/policy/hash/version checks remain at their original frontiers"
        );
        let (old_policy, old_page) = load_before_page_data().unwrap();
        for _ in 0..3 {
            let (policy, page) = load().unwrap();
            assert_eq!(policy, old_policy);
            assert_eq!(*page, old_page);
        }
    }
}
// Nondelegating current/prior view front, followed by the entire old stage path.
