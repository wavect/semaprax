"""One literal page, composed after predecessor metadata and before raw admission."""
import copy, hashlib, json
PAGE_CAP = 262144
SOURCE_CAP = 9753094
POLICY_PATH = 'docs/evidence/finite-source-overlay-v2-20261018/policy.json'
PAGE_PATH = 'docs/evidence/finite-source-overlay-v2-20261018/page.json'
POLICY_LENGTH = 110224
POLICY_SHA256 = 'f1b1645ceb581521cb96786e7c9f8bf4ab45c5e2cf2ee422fbb4c60a55d945ea'
EXTENSION_PATH = 'docs/evidence/fmt-local-441-20261018/extension.json'
EXTENSION_LENGTH = 262076
EXTENSION_SHA256 = '2af7299253e5cfb027186ba4fc151011a937214ff381c9267bfc11ef7d0003b6'
REPLACEMENTS = ('CHANGELOG.md', 'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'docs/ARCHITECTURE.md', 'docs/SUMMARY.md', 'src/cleanup_plan/replay.rs', 'src/cleanup_plan/replay/branch_clone/direct.rs', 'src/cleanup_plan/replay/branch_clone/direct/before.rs', 'src/cleanup_plan/replay/branch_clone/direct/tests.rs', 'src/cleanup_plan/replay/expression_facts/receiving/directory.rs', 'src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix.rs', 'src/cleanup_plan/replay/factored/canonical_shared/before_selected_labels.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests.rs', 'src/cleanup_plan/replay/skeleton_work.rs', 'src/source_reflows.rs', 'src/source_reflows/main_f247_controllers.rs', 'src/source_reflows_merge.rs', 'src/source_reflows_merge/main67107_inputs.rs', 'src/source_reflows_merge/main99f_inputs.rs', 'src/source_reflows_merge/main_e177_inputs.rs', 'src/source_reflows_merge/next_big_batch_inputs.rs', 'tests/hir_module_boundaries.rs', 'tests/wasm/scalar_browser_ci_contract/before_3e66.rs', 'tests/wasm/scalar_browser_ci_contract/incoming_main3e66.rs', 'tests/workspace/semantic_graph.rs', 'tests/workspace/semantic_graph/source_inventory.rs', 'src/cleanup_plan/replay/branch_clone/tests.rs', 'src/cleanup_plan/replay/expression_facts/receiving/retained.rs', 'src/cleanup_plan/replay/factored/canonical_shared.rs', 'src/cleanup_plan/replay/factored/canonical_shared/arena_tests.rs', 'src/cleanup_plan/replay/factored/canonical_shared/before_selected_route.rs', 'src/cleanup_plan/replay/factored/canonical_shared/borrowed_labels.rs', 'src/cleanup_plan/replay/factored/canonical_shared/paid_many/tests.rs', 'src/cleanup_plan/replay/factored/continuation_receiver.rs', 'src/cleanup_plan/replay/factored/oracle_hook.rs', 'src/source_reflows/semantic_receivers.rs', 'src/cleanup_plan/replay/skeleton_work/enumerated_prefix/tests.rs', 'platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs', 'src/cleanup_plan.rs', 'src/cleanup_plan/replay/factored.rs', 'src/cleanup_plan/replay/factored/canonical_shared/before_lexical_successors.rs', 'src/cleanup_plan/replay/factored/canonical_shared/lexical_successors/census.rs', 'src/cleanup_plan/replay/ordinary_plan_history.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor.rs', 'src/cleanup_plan/replay/owned_suffix.rs', 'src/workspace_graph/expected_projection/held_projection.rs', 'src/cleanup_plan/replay/expression_facts/arena/prefix/tests.rs', 'src/cleanup_plan/replay/expression_facts/arena/retained.rs', 'src/cleanup_plan/replay/expression_facts/arena/retained/tests.rs', 'src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_tests.rs', 'src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/retained_lookup.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_census.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_tests.rs', 'src/source_reflows_merge/main_c883_inputs.rs', 'src/workspace_graph/tests.rs', 'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity.rs', 'src/bounded_output.rs', 'src/cleanup/schema_summary/tests.rs', 'src/cleanup_plan/replay/branch_clone.rs', 'src/cleanup_plan/replay/branch_clone/inline_tests.rs', 'src/cleanup_plan/replay/branch_clone/production_capacity.rs', 'src/cleanup_plan/replay/branch_clone/production_capacity/tests.rs', 'src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix/tests.rs', 'src/cleanup_plan/replay/expression_facts/receiving/regions_entry/tests.rs', 'src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/counter_tests.rs', 'src/cleanup_plan/replay/expression_facts/receiving/retained/case_proof/tests.rs', 'src/cleanup_plan/replay/factored/canonical_shared/paid_many/source.rs', 'src/cleanup_plan/replay/factored/moderate_tests.rs', 'src/cleanup_plan/replay/factored/strategy.rs', 'src/cleanup_plan/replay/final_output/current_observation_tests.rs', 'src/cleanup_plan/replay/final_output/tests.rs', 'src/cleanup_plan/replay/live_oracle/source_custody/a.rs', 'src/cleanup_plan/replay/live_oracle/source_custody/b.rs', 'src/cleanup_plan/replay/path_array/main44/mod.rs', 'src/cleanup_plan/replay/path_array/main44/source_domain.rs', 'src/cleanup_plan/replay/path_join.rs', 'src/cleanup_plan/replay/path_join/before_singleton.rs', 'src/cleanup_plan/replay/physical_budget.rs', 'src/cleanup_plan/replay/selected_entry/tests/session_errors.rs', 'src/cleanup_plan/replay/semantic_refusal.rs', 'src/cleanup_plan/replay/semantic_refusal/tests.rs', 'src/cleanup_plan/replay/skeleton_work/enumerated_prefix/before.rs', 'src/cleanup_plan/replay/skeleton_work/prefix_fusion.rs', 'src/cleanup_plan/replay/structural_control/calls.rs', 'src/diagnostic/before_writer.rs', 'src/hir/declaration_index/identity_payer.rs', 'src/hir/expr_nodes.rs', 'src/hir/ids/fixed_operation_tests.rs', 'src/hir/lexical_exit.rs', 'src/hir/resolve_program.rs', 'src/hir/resolve_program_methods.rs', 'src/hir/validation.rs', 'src/loan_plan/paid_live.rs', 'src/loop_calls/byte_writer/resolved.rs', 'src/project/json_codec.rs', 'src/project/mod.rs', 'src/source_reflows/factored_sources.rs', 'src/source_reflows/incoming090_semantic_receivers.rs', 'src/source_reflows/incoming21_semantic_receivers.rs', 'src/source_reflows/incoming67107_semantic_receivers.rs', 'src/source_reflows/main043.rs', 'src/source_reflows/main043/audit_3.rs', 'src/source_reflows/semantic_receivers_mainb80.rs', 'src/source_reflows_merge/current344.rs', 'src/source_reflows_merge/final_ready_inputs.rs', 'src/source_reflows_merge/main44_custody.rs', 'src/source_reflows_merge/main67107_inputs_tail.rs', 'src/source_reflows_merge/main7f8_source_inputs.rs', 'src/source_reflows_merge/main8a_historical.rs', 'src/source_reflows_merge/main_eb2_inputs.rs', 'src/source_reflows_merge/ordinary_plan_inputs.rs', 'src/source_reflows_merge/runtime_json.rs', 'src/source_verify/declaration.rs', 'src/workspace_graph.rs', 'src/workspace_graph/constructor_failure/receiving/typed/tests.rs', 'src/workspace_graph/expected_projection/borrowed_imports/typed/alias/owned_growth_tests.rs', 'src/workspace_graph/expected_projection/type_rewrite/receiving.rs', 'src/workspace_graph/function_alias/before_core.rs', 'src/workspace_graph/function_alias/before_core/tests.rs', 'src/workspace_graph/source_schema/nominal_context/main8a.rs', 'src/workspace_graph/source_schema/static_text/tests.rs', 'src/workspace_graph/test_replay_context.rs', 'tests/doctor_production_provisioner_v1.rs', 'tests/project/standard_library/application_json.rs', 'tests/quality_routing.rs', 'tests/source_locked_contracts.rs', 'tests/support/production_source_inventory.rs', 'tests/wasm/scalar_browser_ci_contract.rs', 'tests/wasm/scalar_browser_ci_contract/browser_custody.rs', 'tests/wasm/scalar_browser_ci_contract/historical_builder.rs', 'tests/wasm/scalar_browser_ci_contract/historical_main_7f8.rs', 'docs/evidence/main-e177-final-source-binding-20261017/current-projection-inputs.tsv', 'src/bounded_output/stage_debits/route_debits.rs', 'src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified.rs', 'src/cleanup_plan/replay/factored/canonical_shared/matched_receiving_layers/successor_targets.rs', 'src/hir/ids/declaration_tag.rs', 'src/hir/ids/declaration_tag/before.rs', 'src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_cross_phase.rs', 'src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_cross_phase_before.rs', 'src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_cross_phase_tests.rs', 'src/cleanup_plan/replay/inventory_coverage/paid/queries.rs', 'src/cleanup_plan/replay/inventory_coverage/paid_before.rs', 'src/cleanup_plan/replay/inventory_coverage/typed_tests/selected_family.rs', 'src/cleanup_plan/replay/path_join/selected.rs', 'src/cleanup_plan/replay/path_join/selected/joins.rs', 'src/cleanup_plan/replay/path_join/selected/lexical_flow.rs', 'src/cleanup_plan/replay/path_join/selected/queries.rs', 'src/cleanup_plan/replay/path_join/selected/renewal.rs', 'src/cleanup_plan/replay/path_join/selected/strings.rs', 'src/cleanup_plan/replay/path_join/selected/tests/history.rs', 'src/cleanup_plan/replay/path_join/selected/tests/lookup.rs', 'src/cleanup_plan/replay/path_join/selected/walk.rs', 'src/cleanup_plan/replay/schema/selected_paid.rs', 'src/cleanup_plan/replay/selected_entry/tests/paid/selected_family.rs', 'src/cleanup_plan/replay/structural_control/selected.rs', 'src/cleanup_plan/replay/structural_control/selected/blocks.rs', 'src/cleanup_plan/replay/structural_control/selected/calls.rs', 'src/cleanup_plan/replay/structural_control/selected/helpers.rs', 'src/cleanup_plan/replay/structural_control/selected/place_before_formals.rs', 'src/cleanup_plan/replay/structural_control/selected/tests/boundary.rs', 'src/cleanup_plan/replay/inventory_coverage.rs', 'src/cleanup_plan/replay/inventory_coverage/paid.rs', 'src/cleanup_plan/replay/lexical_replay.rs', 'src/cleanup_plan/replay/scalar_replay.rs', 'src/iterator_ops.rs')
ADDITIONS = ('src/cleanup_plan/renewal_profile.rs', 'src/cleanup_plan/renewal_profile/funded.rs', 'src/cleanup_plan/renewal_profile/funded_receiving.rs', 'src/cleanup_plan/replay/expression_facts/arena/retained/into_tests.rs', 'src/cleanup_plan/replay/expression_facts/arena/retained_into.rs', 'src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_tests/receiving.rs', 'src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/retained_lookup_before_receiving.rs', 'src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_cross_phase_before_receiving.rs', 'src/cleanup_plan/replay/ordinary_receiving.rs', 'src/cleanup_plan/replay/ordinary_receiving/caller_before.rs', 'src/cleanup_plan/replay/ordinary_receiving/tests.rs', 'src/cleanup_plan/replay/skeleton_work/dag_storage.rs', 'src/cleanup_plan/replay/skeleton_work/receiving.rs', 'src/hir/lexical_exit/owner_funded.rs', 'src/hir/lexical_exit/owner_funded_receiving.rs', 'src/literal_format.rs', 'src/literal_format/funded.rs', 'src/literal_format/funded_receiving.rs', 'src/string_ops/conditions.rs', 'src/string_ops/conditions/funded.rs', 'src/string_ops/conditions/funded_receiving.rs', 'src/cleanup_plan/replay/expression_facts/errors.rs', 'docs/ORDINARY-REPLAY-RECEIVING-V1.md', 'src/hir/ids/declaration_backing_tests.rs', 'src/cleanup_plan/replay/factored/before_inline_singleton.rs', 'src/cleanup_plan/replay/factored/canonical_shared/inline_singleton.rs', 'src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/inline_exit.rs', 'src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/inline_exit/census.rs', 'src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors.rs', 'src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop.rs', 'src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/borrowed_single.rs', 'src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/terminal_leaf.rs', 'src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/tests.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_paired.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value.rs', 'src/source_reflows_merge/runtime_json/sealed_io.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_flat_view.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_fused_flat.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/combined_pair_scope_tests.rs', 'src/ast/program_source/function_alias/scalar_signature.rs', 'src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_block_before.rs', 'src/workspace_graph/constructor_failure/before_scalar_dispatch.rs', 'src/workspace_graph/expected_projection/borrowed_imports/typed/before_scalar_signature.rs', 'src/workspace_graph/expected_projection/borrowed_imports/typed/scalar_signature_tests.rs', 'src/ast/program_source/function_alias/before_string.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_retained_tail.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_before_tail.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/matched_tail_tests.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value_before_tail.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/source_format.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source.rs', 'src/source_reflows_merge/main_e177_historical_before_data.rs', 'src/source_reflows_merge/main_e177_historical_data.rs', 'src/source_reflows_merge/main_e177_historical_data_tests.rs', 'src/source_reflows_merge/main_e177_overlay_fragments_v3.rs', 'src/workspace_graph/expected_projection/borrowed_imports/typed/before_string_signature.rs', 'src/workspace_graph/expected_projection/borrowed_imports/typed/string_signature_tests.rs', 'src/workspace_graph/expected_projection/call_carriers.rs', 'src/workspace_graph/expected_projection/call_carriers_historical_tests.rs', 'src/workspace_graph/expected_projection/call_carriers_inverse.rs', 'src/workspace_graph/expected_projection/call_carriers_original.rs', 'src/workspace_graph/expected_projection/call_carriers_tests.rs', 'src/ast/program_source/decl_slices.rs', 'src/workspace_graph/expected_projection/borrowed_imports/typed.rs', 'src/workspace_graph/expected_projection/borrowed_imports/typed/alias.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_prepared_pairing.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_before_prepared_links.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/prepared_pairing_tests.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links.rs', 'src/workspace_graph/expected_projection/call_carriers_before_directory.rs', 'src/workspace_graph/expected_projection/call_carriers_directory.rs', 'src/workspace_graph/expected_projection/call_carriers_directory_tests.rs', 'src/workspace_graph/expected_projection.rs', 'src/workspace_graph/expected_projection/prelude_forecast_tests.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_identity_prefix.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_before_identity.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value_before_identity.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_tests.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_reachable_primitives.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_reachable_before.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/reachable_pair_scope_tests.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_event_prefix.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_shared_steps.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_shared_steps.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/main99_pair_scope_tests.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value_before_event.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_before_event.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_before_event_tests.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_event_tests.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps.rs', 'src/cleanup_plan/replay/lexical_replay/operand_tests/source.rs', 'src/workspace_graph/expected_projection/private_layout/capacity_tests/source.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_certified_heads.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_shared_runs.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_heads.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_heads_tests.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_before_certified.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_shared_runs.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/main7e_pair_scope_tests.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value_before_certified.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_before_certified.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps/shared_boundaries.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps/shared_boundaries/shared_runs.rs', 'src/workspace_graph/expected_projection/private_layout/registry/scratch_tests.rs', 'src/workspace_graph/cold_canonical/tests.rs', 'src/workspace_graph/expected_projection/cost.rs', 'src/bounded_output/stage_debits/route_debits/json_codec_probe.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_main001.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_main487.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_main_b692.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_pair_links.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_paired_tail.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/borrowed_reader_composition_tests.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_heads_pair_links.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_heads_paired_tail.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_main001.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_main487.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_main_b692.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_pair_links.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_paired_tail.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/main001_pair_scope_tests.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/main487_pair_scope_tests.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/main_b692_pair_scope_tests.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/main_b692_paired_tails.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_link_composition_tests.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_tail_composition_tests.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value_pair_links.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value_paired_tail.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_pair_links.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_paired_tail.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/constructor.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/fused_readers.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/json_ledger.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/paired_tails.rs', 'src/source_reflows_merge/main_e177_overlay_compiled.rs', 'src/source_reflows_merge/main_e177_packet_lookup.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_empty_selector.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_heads_before_empty.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/empty_selector_tests.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_before_empty.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value_before_empty.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_before_empty.rs', 'src/test_allocation_observer/backing.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_final_affine.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_heads_before_final_affine.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/final_affine.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/final_affine_tests.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_before_final_affine.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value_before_final_affine.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_before_final_affine.rs', 'src/bounded_output/exclusive_debits.rs', 'src/hir/ids/lowering_identity_tests.rs', 'src/hir/owned_collection_record/identity_before.rs', 'src/hir/resolve_expr/sort_keys/identity_before.rs', 'src/hir/resolve_expr/sort_keys/identity_tests.rs', 'src/hir/resolve_for/identity_before.rs', 'src/hir/resolve_for_own/identity_before.rs', 'src/source_reflows_merge/main_e177_overlay_pins_v9.rs', 'src/cleanup_plan/replay/ordinary_plan_history/frontier_tests.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/retained_main_289/fixture_helpers.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/retained_main_289/lexical_cursor.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/retained_main_289/mod.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/retained_main_289/pair_scope_census.rs', 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/retained_main_289/pair_scope_tests.rs', 'src/bounded_output/stage_debits/route_debits/retained_main_289.rs', 'tests/workspace.rs', 'src/cleanup_plan/replay/expression_facts.rs', 'tests/wasm/scalar_browser_ci_contract/historical_main_c11.rs', 'tests/wasm/scalar_browser_ci_contract/before_524.rs', 'tests/wasm/scalar_browser_ci_contract/before_671.rs', 'tests/wasm/scalar_browser_ci_contract/before_7d34.rs', 'tests/wasm/scalar_browser_ci_contract/before_21abc.rs', 'src/bounded_output/classifier_string.rs', 'src/cleanup_plan/replay/skeleton_work/enumerated_prefix.rs', 'src/cleanup_plan/replay/sequence_vector/funded_tests.rs', 'src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/compact.rs', 'src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/compact/compact_targets.rs', 'src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/compact/payments.rs', 'src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/tests/compact_results.rs', 'src/cleanup_plan/replay/final_output.rs', 'src/cleanup_plan/replay/final_output/compact.rs', 'src/cleanup_plan/replay/final_output/tests/compact_results.rs', 'src/cleanup_plan/replay/path_join/boxed_states.rs', 'src/cleanup_plan/replay/path_join/boxed_states/receiving_tests.rs', 'src/cleanup_plan/replay/sequence_vector.rs', 'src/cleanup_plan/replay/skeleton_work/enumerated_prefix/flat_probe.rs', 'src/source_reflows_merge/final_ready_inputs/projection_publication.rs', 'src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/compact/active.rs', 'src/source_reflows_merge/main_e177_overlay_pins_v10.rs', 'src/cleanup_plan/replay/branch_clone/production_capacity/before_short.rs', 'src/cleanup_plan/replay/branch_clone/short_plan.rs', 'src/cleanup_plan/replay/nested_shape.rs', 'src/cleanup_plan/replay/nested_shape/scalar_tests.rs', 'src/cleanup_plan/replay/expression_facts/receiving/remaining/tests.rs', 'tests/wasm/scalar_browser_ci_contract/current_profiles573.rs', 'src/cleanup_plan/replay/expression_facts/receiving/helpers/tests.rs', 'src/cleanup_plan/replay/expression_facts/receiving/retained/exits/tests.rs', 'src/cleanup_plan/replay/workspace_oracle.rs', 'src/cleanup_plan/replay/factored/paired.rs', 'src/cleanup_plan/replay/factored/paired/tests.rs', 'src/cleanup_plan/replay/transition.rs', 'src/workspace_graph/initial_report/build.rs', 'docs/AGENT-SKILL-BUNDLE-V1.json', 'src/cleanup_plan/replay/path_join/main_single_groups/receiver_tests.rs')
CONTROL = {POLICY_PATH, PAGE_PATH, EXTENSION_PATH,
 'src/source_reflows_merge/main_e177_final_inputs.rs',
 'src/source_reflows_merge/main_e177_overlay_v2.rs',
 'docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py',
 'docs/evidence/main-e177-final-source-binding-20261017/guard_payload.py',
 'docs/evidence/merge-2e92-browser-20261018/guard-config.json'}
H = lambda b: hashlib.sha256(b).hexdigest()
def canonical(value):
 return (json.dumps(value, sort_keys=True, ensure_ascii=False, allow_nan=False, separators=(',', ':'))+'\n').encode()
def parse(raw):
 def unique(pairs):
  result = {}
  for key,value in pairs:
   assert key not in result, 'duplicate JSON key'
   result[key] = value
  return result
 value = json.loads(raw, object_pairs_hook=unique)
 assert canonical(value) == raw, 'noncanonical JSON'
 return value
def keys(value, expected):
 return type(value) is dict and set(value) == set(expected)
def normal(path):
 return type(path) is str and path and not any(c in path for c in ('\\', ':', '\0')) and all(p and p not in ('.','..') for p in path.split('/'))
def pin(value):
 length,sha,path = value['length'],value['sha256'],value['archive']
 assert type(length) is int and 0 <= length <= SOURCE_CAP
 assert type(sha) is str and len(sha)==64 and all(c in '0123456789abcdef' for c in sha)
 assert normal(path)
 return length,sha,path
def edges(packet):
 rows = packet['rows']; by = {r['source']:r for r in rows}
 assert len(by)==len(rows) and all(type(p) is str and p for p in by)
 result=[]
 for row in rows:
  for child in row.get('fragments',[]):
   assert 'fragments' not in child and child['compiled'] is True
   assert by[child['source']]==child and row['source']!=child['source']
   edge=(row['source'],child['source']); assert edge not in result; result.append(edge)
 return result
V3_SCHEMA = 'semaprax.finite-source-overlay-page.v3'
V3_ROOTS = ('tests/hir_module_boundaries.rs',
 'tests/workspace/semantic_graph/source_inventory.rs',
 'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs')
def expand_fragment_refs(before, policy, page):
 assert page['schema']==V3_SCHEMA
 by={r['source']:r for r in before['rows']}; assert len(by)==len(before['rows'])
 seen=set()
 for group in ('replacements','additions'):
  for row in page[group]:
   source=row['source']; assert source not in seen; seen.add(source); by[source]=row
 allowed=set(edges(before))
 for edge in policy['new_fragment_edges']:
  assert keys(edge,('parent','child')); allowed.add((edge['parent'],edge['child']))
 result=copy.deepcopy(page); result['schema']='semaprax.finite-source-overlay-page.v2'
 for group in ('replacements','additions'):
  for row in result[group]:
   parent=row['source']; seen=set()
   for i,fragment in enumerate(row.get('fragments',[])):
    if 'canonical_source' not in fragment:continue
    assert parent in V3_ROOTS and keys(fragment,('canonical_source',))
    child=fragment['canonical_source']; assert type(child) is str
    edge=(parent,child); assert parent!=child and edge in allowed and edge not in seen; seen.add(edge)
    target=by[child]; assert target['compiled'] is True and 'fragments' not in target
    row['fragments'][i]=copy.deepcopy(target)
 return result

V4_SCHEMA = 'semaprax.finite-source-overlay-page.v4'
V4_OLD519 = (
 ('src/source_reflows_merge.rs','3ef2bb440f23c573007567c98b4bfc608b555bd2cc8f2591d365476b850bd46a'),
 ('src/source_reflows/incoming67107_semantic_receivers.rs','383eff4edb39826861991d6dcd84dc0c489471e90e66ed214b4d5ad46a907ac1'))
def expand_old519_refs(before, policy, page):
 assert page['schema']==V4_SCHEMA and len(before['rows'])==519
 result=copy.deepcopy(page); result['schema']=V3_SCHEMA; seen=set()
 for i,row in enumerate(result['replacements']):
  if 'old519' not in row:
   assert row['source'] not in {p for p,_ in V4_OLD519}, 'missing literal inheritance'
   continue
  assert keys(row,('old519',)) and type(row['old519']) is list and len(row['old519'])==4
  index,length,sha,extra=row['old519']
  assert type(index) is int and 0<=index<len(V4_OLD519) and index not in seen
  seen.add(index); source,old_sha=V4_OLD519[index]
  matches=[r for r in before['rows'] if r['source']==source]
  assert len(matches)==1 and H(canonical(matches[0]))==old_sha, 'complete old row drift'
  assert type(sha) is str
  current=dict(length=length,sha256=sha,archive='e/'+sha); pin(current)
  prior=matches[0]; assert current!=prior['current'] and type(extra) is list
  for view in extra:pin(view)
  full=copy.deepcopy(prior); full['current']=current
  full['views'].append(dict(prior['current'],name='whole-pre-overlay-v2-current'))
  full['views'].extend(copy.deepcopy(extra)); result['replacements'][i]=full
 assert seen==set(range(len(V4_OLD519))), 'missing literal reference'
 assert all('old519' not in row for row in result['additions'])
 return expand_fragment_refs(before,policy,result)

V5_SCHEMA = 'semaprax.finite-source-overlay-page.v5'
V5_EXTRA519 = (
 ('src/cleanup_plan/replay/expression_facts/arena/retained/tests.rs','e9f1208c4daeeb93e1200a127c9dee49e57c3ee370c10c5c0305c8c4266d1443'),
 ('src/cleanup_plan/replay.rs','85efc15e0205f0f2c4070c9d80b6f30b50f73bc6e4070854484bdd75e8fb9a8a'))
def expand_extra519_refs(before, policy, page):
 assert page['schema']==V5_SCHEMA and len(before['rows'])==519
 result=copy.deepcopy(page); result['schema']=V4_SCHEMA; seen=set()
 for i,row in enumerate(result['replacements']):
  if 'old519' not in row:
   assert row['source'] not in {p for p,_ in V5_EXTRA519}, 'missing literal inheritance'
   continue
  assert keys(row,('old519',)) and type(row['old519']) is list and len(row['old519'])==4
  index,length,sha,extra=row['old519']
  assert type(index) is int and 0<=index<4
  if index<2:continue
  assert index not in seen; seen.add(index); source,old_sha=V5_EXTRA519[index-2]
  matches=[r for r in before['rows'] if r['source']==source]
  assert len(matches)==1 and H(canonical(matches[0]))==old_sha, 'complete old row drift'
  assert type(sha) is str
  current=dict(length=length,sha256=sha,archive='e/'+sha); pin(current)
  prior=matches[0]; assert current!=prior['current'] and type(extra) is list
  for view in extra:pin(view)
  full=copy.deepcopy(prior); full['current']=current
  full['views'].append(dict(prior['current'],name='whole-pre-overlay-v2-current'))
  full['views'].extend(copy.deepcopy(extra)); result['replacements'][i]=full
 assert seen=={2,3}, 'missing literal reference'
 assert all('old519' not in row for row in result['additions'])
 return expand_old519_refs(before,policy,result)

V6_SCHEMA = 'semaprax.finite-source-overlay-page.v6'
def expand_old519_indices(before, policy, page):
 assert page['schema']==V6_SCHEMA and len(before['rows'])==519
 assert type(policy['predecessor']['rows']) is int and policy['predecessor']['rows']==519
 assert type(before['source_cap']) is int and before['source_cap']==SOURCE_CAP
 assert H(canonical(before))==policy['predecessor']['logical_sha256'], 'v6 authenticated old519'
 result=copy.deepcopy(page); result['schema']=V3_SCHEMA; seen=set()
 for i,row in enumerate(result['replacements']):
  assert 'old519' not in row, 'v6 legacy reference'
  if 'old519_index' not in row:continue
  assert keys(row,('old519_index',)) and type(row['old519_index']) is list and len(row['old519_index'])==4
  index,current,suffix,fragments=row['old519_index']
  assert type(index) is int and 0<=index<519 and index not in seen
  seen.add(index); prior=before['rows'][index]; pin(current)
  assert type(suffix) is list
  for view in suffix:pin(view)
  full=copy.deepcopy(prior); full['current']=copy.deepcopy(current); full['views'].extend(copy.deepcopy(suffix))
  if fragments is not None:
   assert prior['source'] in V3_ROOTS and type(fragments) is list, 'v6 fragment override root or shape'
   full['fragments']=copy.deepcopy(fragments)
  result['replacements'][i]=full
 assert all('old519_index' not in row and 'old519' not in row for row in result['additions'])
 return expand_fragment_refs(before,policy,result)

V7_SCHEMA = 'semaprax.finite-source-overlay-page.v7'
def expand_canonical_indices(before, policy, page):
 assert page['schema']==V7_SCHEMA and len(before['rows'])==519
 # The same authenticated predecessor and closed additions determine every index.
 assert H(canonical(before))==policy['predecessor']['logical_sha256']
 sources=[r['source'] for r in before['rows']]+[r['source'] for r in page['additions']]
 assert len(set(sources))==len(sources), 'v7 duplicate canonical source'
 result=copy.deepcopy(page); result['schema']=V6_SCHEMA
 for group in ('replacements','additions'):
  for row in result[group]:
   if 'old519_index' in row:
    reference=row['old519_index']
    assert keys(row,('old519_index',)) and type(reference) is list and len(reference)==4
    index=reference[0]; assert type(index) is int and 0<=index<519
    parent=sources[index]; fragments=reference[3]
   else:parent=row['source']; fragments=row.get('fragments')
   if fragments is None:continue
   assert type(fragments) is list
   for fragment in fragments:
    if 'canonical_source' not in fragment:continue
    value=fragment['canonical_source']
    if type(value) is str:continue
    assert parent in V3_ROOTS and keys(fragment,('canonical_source',))
    assert type(value) is int and 519<=value<len(sources), 'v7 canonical addition index'
    fragment['canonical_source']=sources[value]
 return expand_old519_indices(before,policy,result)

V8_SCHEMA = 'semaprax.finite-source-overlay-page.v8'
V8_CARRIERS = (
 (38, 'src/source_reflows_merge/main67107_inputs.rs',
  'ae571c1bf1f461c0485d0e5f4c5b231af8d7f5889e99e81d04e55ef86bd561d8',
  366, 'src/source_reflows_merge/main67107_inputs_tail.rs'),
 (314, 'src/hir/resolve_program.rs',
  '4bc3d8948fadbac1745748fa65dec109e2e2f2aad21147bdc41b9b4fb0b76913',
  316, 'src/hir/resolve_program_methods.rs'))
def expand_v8_carriers(before, policy, page):
 assert page['schema']==V8_SCHEMA and len(before['rows'])==519
 assert H(canonical(before))==policy['predecessor']['logical_sha256'], 'v8 authenticated old519'
 result=copy.deepcopy(page); result['schema']=V7_SCHEMA; seen=set()
 for position,row in enumerate(result['replacements']):
  if 'old519_index' not in row:continue
  reference=row['old519_index']
  assert keys(row,('old519_index',)) and type(reference) is list and len(reference)==4
  index=reference[0]; assert type(index) is int and 0<=index<519
  binding=next((b for b in V8_CARRIERS if b[0]==index),None)
  if binding is None or reference[3] is None:continue
  _,source,old_sha,child_index,child=binding
  assert index not in seen, 'v8 duplicate carrier'; seen.add(index)
  old=before['rows'][index]
  assert old['source']==source and H(canonical(old))==old_sha, 'v8 complete old carrier'
  assert before['rows'][child_index]['source']==child, 'v8 old child identity'
  fragments=reference[3]
  assert type(fragments) is list and fragments==[{'canonical_source':child}], 'v8 exact single leaf reference'
  assert len(old.get('fragments',[]))==1 and old['fragments'][0]['source']==child
  targets=[]
  for candidate in page['replacements']:
   if 'old519_index' in candidate:
    target=candidate['old519_index']
    assert keys(candidate,('old519_index',)) and type(target) is list and len(target)==4
    target_index=target[0]; assert type(target_index) is int and 0<=target_index<519
    if target_index!=child_index:continue
    assert target[3] is None, 'v8 leaf override'
    target_row=copy.deepcopy(before['rows'][child_index]); target_row['current']=copy.deepcopy(target[1])
    assert type(target[2]) is list
    target_row['views'].extend(copy.deepcopy(target[2])); targets.append(target_row)
   elif candidate['source']==child:targets.append(copy.deepcopy(candidate))
  assert len(targets)==1, 'v8 unique present canonical leaf'
  target=targets[0]
  assert target['source']==child and target['compiled'] is True and 'fragments' not in target, 'v8 compiled canonical leaf'
  pin(target['current'])
  for view in target['views']:pin(view)
  pin(reference[1]); assert type(reference[2]) is list
  for view in reference[2]:pin(view)
  full=copy.deepcopy(old); full['current']=copy.deepcopy(reference[1])
  full['views'].extend(copy.deepcopy(reference[2])); full['fragments']=[target]
  result['replacements'][position]=full
 return expand_canonical_indices(before,policy,result)

V9_SCHEMA = 'semaprax.finite-source-overlay-page.v9'
def v9_pin(value, named, encode=False):
 # Only documented slots call this function. Extra fields keep object form.
 if encode:
  expected=('name','length','sha256','archive') if named else ('length','sha256','archive')
  if not keys(value,expected):return copy.deepcopy(value)
  pin(value)
  if named and type(value['name']) is not str:return copy.deepcopy(value)
  result=([value['name']] if named else [])+[value['length'],value['sha256']]
  if value['archive']!='e/'+value['sha256']:result.append(value['archive'])
  return result
 if type(value) is not list:return copy.deepcopy(value)
 count=3 if named else 2
 assert len(value) in (count,count+1), 'v9 pin arity'
 offset=1 if named else 0
 if named:assert type(value[0]) is str, 'v9 view name'
 length,sha=value[offset:offset+2]
 assert type(sha) is str, 'v9 digest type'
 result=dict(length=length,sha256=sha,archive=value[-1] if len(value)==count+1 else 'e/'+sha)
 if named:result['name']=value[0]
 pin(result)
 return result

def transform_v9_pin_slots(page, encode=False):
 # Deliberately one row/one fragment level, never recursive metadata matching.
 result=copy.deepcopy(page)
 def row_pins(row):
  assert type(row) is dict, 'v9 literal row'
  if 'current' in row:row['current']=v9_pin(row['current'],False,encode)
  if 'views' in row:
   assert type(row['views']) is list, 'v9 views'
   row['views']=[v9_pin(view,True,encode) for view in row['views']]
 def fragment_pins(fragments):
  if fragments is None:return
  assert type(fragments) is list, 'v9 fragments'
  for fragment in fragments:
   assert type(fragment) is dict, 'v9 fragment row'
   if 'canonical_source' not in fragment:row_pins(fragment)
 for group in ('replacements','additions'):
  assert type(result[group]) is list, 'v9 rows'
  for row in result[group]:
   assert type(row) is dict, 'v9 row'
   if 'old519_index' in row:
    assert group=='replacements' and keys(row,('old519_index',)), 'v9 indexed row'
    reference=row['old519_index']
    assert type(reference) is list and len(reference)==4, 'v9 old519 arity'
    reference[1]=v9_pin(reference[1],False,encode)
    assert type(reference[2]) is list, 'v9 appended views'
    reference[2]=[v9_pin(view,True,encode) for view in reference[2]]
    fragment_pins(reference[3])
   else:
    row_pins(row)
    if 'fragments' in row:fragment_pins(row['fragments'])
 return result

def expand_v9_pins(page):
 assert page['schema']==V9_SCHEMA
 result=transform_v9_pin_slots(page);result['schema']=V8_SCHEMA
 return result

def encode_v9_pins(page):
 assert page['schema']==V8_SCHEMA
 result=transform_v9_pin_slots(page,True);result['schema']=V9_SCHEMA
 assert canonical(expand_v9_pins(result))==canonical(page), 'v9 ENTIRE V8 page inverse'
 assert len(canonical(result))<=PAGE_CAP, 'v9 same one-page cap'
 return result

V10_SCHEMA = 'semaprax.finite-source-overlay-page.v10'
def v10_base(before,policy):
 assert len(before['rows'])==519 and type(policy['predecessor']['rows']) is int and policy['predecessor']['rows']==519, 'v10 old rows'
 assert type(before['source_cap']) is int and before['source_cap']==SOURCE_CAP, 'v10 source cap'
 assert H(canonical(before))==policy['predecessor']['logical_sha256'], 'v10 authenticated old519'
def v10_pin(value,named,before,encode=False):
 if type(value) is not list:return copy.deepcopy(value)
 if encode:
  expanded=v9_pin(value,named)
  if v9_pin(expanded,named,True)!=value:return copy.deepcopy(value)
  name=expanded.pop('name') if named else None
  index=next((i for i,row in enumerate(before['rows']) if row['current']==expanded),None)
  return copy.deepcopy(value) if index is None else ([name,index] if named else [index])
 offset=int(named)
 if len(value)!=offset+1:return copy.deepcopy(value)
 if named:assert type(value[0]) is str, 'v10 view name'
 index=value[offset];assert type(index) is int and 0<=index<519, 'v10 pin index'
 current=copy.deepcopy(before['rows'][index]['current']);assert keys(current,('length','sha256','archive')), 'v10 exact current pin'
 pin(current)
 if named:current['name']=value[0]
 return v9_pin(current,named,True)
def transform_v10_pin_slots(page, before, policy, encode=False):
 v10_base(before,policy)
 def convert(value,named,encode=False):return v10_pin(value,named,before,encode)
 # Deliberately one row/one fragment level, never recursive metadata matching.
 result=copy.deepcopy(page)
 def row_pins(row):
  assert type(row) is dict, 'v9 literal row'
  if 'current' in row:row['current']=convert(row['current'],False,encode)
  if 'views' in row:
   assert type(row['views']) is list, 'v9 views'
   row['views']=[convert(view,True,encode) for view in row['views']]
 def fragment_pins(fragments):
  if fragments is None:return
  assert type(fragments) is list, 'v9 fragments'
  for fragment in fragments:
   assert type(fragment) is dict, 'v9 fragment row'
   if 'canonical_source' not in fragment:row_pins(fragment)
 for group in ('replacements','additions'):
  assert type(result[group]) is list, 'v9 rows'
  for row in result[group]:
   assert type(row) is dict, 'v9 row'
   if 'old519_index' in row:
    assert group=='replacements' and keys(row,('old519_index',)), 'v9 indexed row'
    reference=row['old519_index']
    assert type(reference) is list and len(reference)==4, 'v9 old519 arity'
    reference[1]=convert(reference[1],False,encode)
    assert type(reference[2]) is list, 'v9 appended views'
    reference[2]=[convert(view,True,encode) for view in reference[2]]
    fragment_pins(reference[3])
   else:
    row_pins(row)
    if 'fragments' in row:fragment_pins(row['fragments'])
 return result

def expand_v10_pins(before,policy,page):
 assert page['schema']==V10_SCHEMA
 result=transform_v10_pin_slots(page,before,policy);result['schema']=V9_SCHEMA
 return result
def encode_v10_pins(before,policy,page):
 assert page['schema']==V9_SCHEMA
 result=transform_v10_pin_slots(page,before,policy,True);result['schema']=V10_SCHEMA
 assert canonical(expand_v10_pins(before,policy,result))==canonical(page), 'v10 ENTIRE V9 page inverse'
 assert len(canonical(result))<=PAGE_CAP, 'v10 same one-page cap'
 return result

def compose_values(before, policy, page):
 if page['schema']==V10_SCHEMA:page=expand_v10_pins(before,policy,page)
 if page['schema']==V9_SCHEMA:page=expand_v9_pins(page)
 if page['schema']==V8_SCHEMA:page=expand_v8_carriers(before,policy,page)
 elif page['schema']==V7_SCHEMA:page=expand_canonical_indices(before,policy,page)
 elif page['schema']==V6_SCHEMA:page=expand_old519_indices(before,policy,page)
 elif page['schema']==V5_SCHEMA:page=expand_extra519_refs(before,policy,page)
 elif page['schema']==V4_SCHEMA:page=expand_old519_refs(before,policy,page)
 elif page['schema']==V3_SCHEMA:page=expand_fragment_refs(before,policy,page)
 return compose_values_v2(before,policy,page)

def compose_values_v2(before, policy, page):
 assert keys(page, ('schema','index','total_pages','predecessor','replacements','additions'))
 assert page['schema']=='semaprax.finite-source-overlay-page.v2' and type(page['index']) is int and page['index']==1 and type(page['total_pages']) is int and page['total_pages']==1
 assert page['predecessor']==policy['predecessor']
 assert H(canonical(before))==policy['predecessor']['logical_sha256']
 old=before['rows']; assert len(old)==policy['predecessor']['rows'] and before['source_cap']==SOURCE_CAP
 positions={r['source']:i for i,r in enumerate(old)}; assert len(positions)==len(old)
 result=copy.deepcopy(before); rows=result['rows']; seen=set(); previous=-1
 assert len(page['replacements'])==len(policy['replacements'])
 for row,source in zip(page['replacements'],policy['replacements']):
  assert normal(source) and source not in CONTROL and row['source']==source and source not in seen; seen.add(source)
  index=positions[source]; assert previous < index; previous=index; prior=old[index]
  assert type(row['compiled']) is bool and row['compiled']==prior['compiled']
  views=row['views']; old_views=prior['views']; assert views[:len(old_views)]==old_views and len(views)>=len(old_views)
  pin(row['current'])
  for view in views[len(old_views):]:pin(view)
  if row['current']!=prior['current']:assert dict(name='whole-pre-overlay-v2-current',**prior['current']) in views[len(old_views):]
  restored=copy.deepcopy(row); restored['current']=prior['current']; restored['views']=old_views
  if 'fragments' in prior:restored['fragments']=prior['fragments']
  else:restored.pop('fragments',None)
  assert restored==prior, 'whole row inverse/unknown fields'; rows[index]=copy.deepcopy(row)
 assert len(page['additions'])==len(policy['additions'])
 for row,binding in zip(page['additions'],policy['additions']):
  assert keys(binding, ('source','compiled')); source=binding['source']
  assert normal(source) and source not in CONTROL and row['source']==source and source not in positions and source not in seen; seen.add(source)
  assert type(binding['compiled']) is bool and type(row['compiled']) is bool and row['compiled']==binding['compiled']
  pin(row['current'])
  for view in row['views']:pin(view)
  rows.append(copy.deepcopy(row))
 assert len(rows)==len(old)+len(page['additions'])
 old_edges=edges(before); new_edges=edges(result)
 assert [e for e in new_edges if e in old_edges]==old_edges
 assert [dict(parent=p,child=c) for p,c in new_edges if (p,c) not in old_edges]==policy['new_fragment_edges']
 restored=copy.deepcopy(result); restored['rows']=copy.deepcopy(old); assert restored==before, 'ENTIRE predecessor object inverse'
 return result

class Overlay:
 def __init__(self, root, bounded, read, configuration_current, predecessor, stage_rows):
  self.root=root; self.bounded=bounded; self.read=read; self.configuration_current=configuration_current
  self.predecessor=predecessor; self.stage_rows=stage_rows
 def load(self):
  self.configuration_current()
  assert POLICY_LENGTH<=PAGE_CAP
  policy=parse(self.bounded(self.root/POLICY_PATH,POLICY_LENGTH,POLICY_SHA256))
  assert keys(policy, ('schema','page','predecessor','replacements','additions','new_fragment_edges'))
  assert policy['schema']=='semaprax.finite-source-overlay-policy.v2'
  p=policy['page']; assert keys(p,('path','length','sha256')) and p['path']==PAGE_PATH
  pred=policy['predecessor']; assert keys(pred,('extension','rows','logical_sha256')) and pred['rows']==519
  assert pred['extension']==dict(path=EXTENSION_PATH,length=EXTENSION_LENGTH,sha256=EXTENSION_SHA256)
  self.bounded(self.root/EXTENSION_PATH,EXTENSION_LENGTH,EXTENSION_SHA256)
  assert policy['replacements']==list(REPLACEMENTS) and [r['source'] for r in policy['additions']]==list(ADDITIONS)
  assert type(p['length']) is int and 0<=p['length']<=PAGE_CAP
  page=parse(self.bounded(self.root/PAGE_PATH,p['length'],p['sha256']))
  return policy,page
 def compose(self, before):
  policy,page=self.load(); return compose_values(before,policy,page)
 def affected_row(self, source):
  if source not in REPLACEMENTS and source not in ADDITIONS:return None
  packet=self.compose(self.predecessor())
  return next(r for r in packet['rows'] if r['source']==source)
 def current(self,row):
  length,sha,_=pin(row['current'])
  # Keep bounded()'s original Python per-read cap; no aggregate bound or
  # oversized-source splitting is introduced by the metadata page.
  actual=self.bounded(self.root/row['source'],length,sha)
  assert actual==self.read(row['current'])
  for child in row.get('fragments',[]):self.current(child)
  return actual
 def stage_source(self, source, requested):
  row=self.affected_row(source)
  if row is None:return None
  actual=self.current(row)
  if (requested['length'],requested['sha256'])==(row['current']['length'],row['current']['sha256']):return actual
  # Only literal authenticated predecessor stage metadata can supply an
  # additional historical pin. No caller archive/path is used as authority.
  original=next((r for r in self.stage_rows() if r['source']==source),None)
  choices=[row['current'],*row['views']]
  if original is not None:choices.extend([original['before'],original['current'],*original.get('views',[])])
  matches=[p for p in choices if (p['length'],p['sha256'])==(requested['length'],requested['sha256'])]
  assert matches, 'foreign overlay tuple'
  prior=self.read(matches[0]); assert all(self.read(p)==prior for p in matches)
  return prior
