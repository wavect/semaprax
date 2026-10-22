pub(super) fn compiled(source: &str) -> Option<&'static str> {
    match source {
        "src/bounded_output/exclusive_debits.rs" => Some(include_str!("../../src/bounded_output/exclusive_debits.rs")),
        "src/hir/ids/lowering_identity_tests.rs" => Some(include_str!("../../src/hir/ids/lowering_identity_tests.rs")),
        "src/hir/owned_collection_record/identity_before.rs" => Some(include_str!("../../src/hir/owned_collection_record/identity_before.rs")),
        "src/hir/resolve_expr/sort_keys/identity_before.rs" => Some(include_str!("../../src/hir/resolve_expr/sort_keys/identity_before.rs")),
        "src/hir/resolve_expr/sort_keys/identity_tests.rs" => Some(include_str!("../../src/hir/resolve_expr/sort_keys/identity_tests.rs")),
        "src/hir/resolve_for/identity_before.rs" => Some(include_str!("../../src/hir/resolve_for/identity_before.rs")),
        "src/hir/resolve_for_own/identity_before.rs" => Some(include_str!("../../src/hir/resolve_for_own/identity_before.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_final_affine.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_final_affine.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_heads_before_final_affine.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_heads_before_final_affine.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/final_affine.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/final_affine.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/final_affine_tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/final_affine_tests.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_before_final_affine.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_before_final_affine.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value_before_final_affine.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value_before_final_affine.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_before_final_affine.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_before_final_affine.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_empty_selector.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_empty_selector.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_heads_before_empty.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_heads_before_empty.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/empty_selector_tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/empty_selector_tests.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_before_empty.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_before_empty.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value_before_empty.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value_before_empty.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_before_empty.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_before_empty.rs")),
        "src/test_allocation_observer/backing.rs" => Some(include_str!("../../src/test_allocation_observer/backing.rs")),
        "src/bounded_output/stage_debits/route_debits/json_codec_probe.rs" => Some(include_str!("../../src/bounded_output/stage_debits/route_debits/json_codec_probe.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_main001.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_main001.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_main487.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_main487.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_main_b692.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_main_b692.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_pair_links.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_pair_links.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_paired_tail.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_paired_tail.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/borrowed_reader_composition_tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/borrowed_reader_composition_tests.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_heads_pair_links.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_heads_pair_links.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_heads_paired_tail.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_heads_paired_tail.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_main001.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_main001.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_main487.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_main487.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_main_b692.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_main_b692.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_pair_links.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_pair_links.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_paired_tail.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_paired_tail.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/main001_pair_scope_tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/main001_pair_scope_tests.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/main487_pair_scope_tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/main487_pair_scope_tests.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/main_b692_pair_scope_tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/main_b692_pair_scope_tests.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/main_b692_paired_tails.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/main_b692_paired_tails.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_link_composition_tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_link_composition_tests.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_tail_composition_tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_tail_composition_tests.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value_pair_links.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value_pair_links.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value_paired_tail.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value_paired_tail.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_pair_links.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_pair_links.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_paired_tail.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_paired_tail.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/constructor.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/constructor.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/fused_readers.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/fused_readers.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/json_ledger.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/json_ledger.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/paired_tails.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/paired_tails.rs")),
        "src/source_reflows_merge/main_e177_overlay_compiled.rs" => Some(include_str!("../../src/source_reflows_merge/main_e177_overlay_compiled.rs")),
        "src/source_reflows_merge/main_e177_packet_lookup.rs" => Some(include_str!("../../src/source_reflows_merge/main_e177_packet_lookup.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_certified_heads.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_certified_heads.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_shared_runs.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_shared_runs.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_heads.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_heads.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_heads_tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_heads_tests.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_before_certified.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_before_certified.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_shared_runs.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_shared_runs.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/main7e_pair_scope_tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/main7e_pair_scope_tests.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value_before_certified.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value_before_certified.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_before_certified.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_before_certified.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps/shared_boundaries.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps/shared_boundaries.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps/shared_boundaries/shared_runs.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps/shared_boundaries/shared_runs.rs")),
        "src/workspace_graph/expected_projection/private_layout/registry/scratch_tests.rs" => Some(include_str!("../../src/workspace_graph/expected_projection/private_layout/registry/scratch_tests.rs")),
        "src/workspace_graph/cold_canonical/tests.rs" => Some(include_str!("../../src/workspace_graph/cold_canonical/tests.rs")),
        "src/workspace_graph/expected_projection/cost.rs" => Some(include_str!("../../src/workspace_graph/expected_projection/cost.rs")),
        "src/cleanup_plan/replay/lexical_replay/operand_tests/source.rs" => Some(include_str!("../../src/cleanup_plan/replay/lexical_replay/operand_tests/source.rs")),
        "src/workspace_graph/expected_projection/private_layout/capacity_tests/source.rs" => Some(include_str!("../../src/workspace_graph/expected_projection/private_layout/capacity_tests/source.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_event_prefix.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_event_prefix.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_shared_steps.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_shared_steps.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_shared_steps.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_shared_steps.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/main99_pair_scope_tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/main99_pair_scope_tests.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value_before_event.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value_before_event.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_before_event.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_before_event.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_before_event_tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_before_event_tests.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_event_tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_event_tests.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_reachable_primitives.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_reachable_primitives.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_reachable_before.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_reachable_before.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/reachable_pair_scope_tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/reachable_pair_scope_tests.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_identity_prefix.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_identity_prefix.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_before_identity.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_before_identity.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value_before_identity.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value_before_identity.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_tests.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_prepared_pairing.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_prepared_pairing.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_before_prepared_links.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_before_prepared_links.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/prepared_pairing_tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/prepared_pairing_tests.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links.rs")),
        "src/workspace_graph/expected_projection/call_carriers_before_directory.rs" => Some(include_str!("../../src/workspace_graph/expected_projection/call_carriers_before_directory.rs")),
        "src/workspace_graph/expected_projection/call_carriers_directory.rs" => Some(include_str!("../../src/workspace_graph/expected_projection/call_carriers_directory.rs")),
        "src/workspace_graph/expected_projection/call_carriers_directory_tests.rs" => Some(include_str!("../../src/workspace_graph/expected_projection/call_carriers_directory_tests.rs")),
        "src/workspace_graph/expected_projection.rs" => Some(include_str!("../../src/workspace_graph/expected_projection.rs")),
        "src/workspace_graph/expected_projection/prelude_forecast_tests.rs" => Some(include_str!("../../src/workspace_graph/expected_projection/prelude_forecast_tests.rs")),
        "src/ast/program_source/function_alias/before_string.rs" => Some(include_str!("../../src/ast/program_source/function_alias/before_string.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_retained_tail.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_retained_tail.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_before_tail.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor_before_tail.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/matched_tail_tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/matched_tail_tests.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value_before_tail.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value_before_tail.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/source_format.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/source_format.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source.rs")),
        "src/source_reflows_merge/main_e177_historical_before_data.rs" => Some(include_str!("../../src/source_reflows_merge/main_e177_historical_before_data.rs")),
        "src/source_reflows_merge/main_e177_historical_data.rs" => Some(include_str!("../../src/source_reflows_merge/main_e177_historical_data.rs")),
        "src/source_reflows_merge/main_e177_historical_data_tests.rs" => Some(include_str!("../../src/source_reflows_merge/main_e177_historical_data_tests.rs")),
        "src/source_reflows_merge/main_e177_overlay_fragments_v3.rs" => Some(include_str!("../../src/source_reflows_merge/main_e177_overlay_fragments_v3.rs")),
        "src/source_reflows_merge/main_e177_overlay_pins_v9.rs" => Some(include_str!("../../src/source_reflows_merge/main_e177_overlay_pins_v9.rs")),
        "src/workspace_graph/expected_projection/borrowed_imports/typed/before_string_signature.rs" => Some(include_str!("../../src/workspace_graph/expected_projection/borrowed_imports/typed/before_string_signature.rs")),
        "src/workspace_graph/expected_projection/borrowed_imports/typed/string_signature_tests.rs" => Some(include_str!("../../src/workspace_graph/expected_projection/borrowed_imports/typed/string_signature_tests.rs")),
        "src/workspace_graph/expected_projection/call_carriers.rs" => Some(include_str!("../../src/workspace_graph/expected_projection/call_carriers.rs")),
        "src/workspace_graph/expected_projection/call_carriers_historical_tests.rs" => Some(include_str!("../../src/workspace_graph/expected_projection/call_carriers_historical_tests.rs")),
        "src/workspace_graph/expected_projection/call_carriers_inverse.rs" => Some(include_str!("../../src/workspace_graph/expected_projection/call_carriers_inverse.rs")),
        "src/workspace_graph/expected_projection/call_carriers_original.rs" => Some(include_str!("../../src/workspace_graph/expected_projection/call_carriers_original.rs")),
        "src/workspace_graph/expected_projection/call_carriers_tests.rs" => Some(include_str!("../../src/workspace_graph/expected_projection/call_carriers_tests.rs")),
        "src/ast/program_source/decl_slices.rs" => Some(include_str!("../../src/ast/program_source/decl_slices.rs")),
        "src/workspace_graph/expected_projection/borrowed_imports/typed.rs" => Some(include_str!("../../src/workspace_graph/expected_projection/borrowed_imports/typed.rs")),
        "src/workspace_graph/expected_projection/borrowed_imports/typed/alias.rs" => Some(include_str!("../../src/workspace_graph/expected_projection/borrowed_imports/typed/alias.rs")),
        "src/ast/program_source/function_alias/scalar_signature.rs" => Some(include_str!("../../src/ast/program_source/function_alias/scalar_signature.rs")),
        "src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_block_before.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_block_before.rs")),
        "src/workspace_graph/constructor_failure/before_scalar_dispatch.rs" => Some(include_str!("../../src/workspace_graph/constructor_failure/before_scalar_dispatch.rs")),
        "src/workspace_graph/expected_projection/borrowed_imports/typed/before_scalar_signature.rs" => Some(include_str!("../../src/workspace_graph/expected_projection/borrowed_imports/typed/before_scalar_signature.rs")),
        "src/workspace_graph/expected_projection/borrowed_imports/typed/scalar_signature_tests.rs" => Some(include_str!("../../src/workspace_graph/expected_projection/borrowed_imports/typed/scalar_signature_tests.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_flat_view.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_flat_view.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_fused_flat.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_fused_flat.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/combined_pair_scope_tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/combined_pair_scope_tests.rs")),
        "src/source_reflows_merge/runtime_json/sealed_io.rs" => Some(include_str!("../../src/source_reflows_merge/runtime_json/sealed_io.rs")),

        "src/cleanup_plan/replay/factored/before_inline_singleton.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/before_inline_singleton.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/inline_singleton.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/inline_exit.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/inline_exit.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/inline_exit/census.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/inline_exit/census.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/borrowed_single.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/borrowed_single.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/terminal_leaf.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/terminal_leaf.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/tests.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_paired.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_paired.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/paired_value.rs")),

        "src/source_reflows_merge/main_e177_overlay_v2.rs" => {
            Some(include_str!("main_e177_overlay_v2.rs"))
        }
        "src/cleanup_plan/replay/expression_facts/errors.rs" => Some(include_str!(
            "../../src/cleanup_plan/replay/expression_facts/errors.rs"
        )),
        "src/cleanup_plan/renewal_profile.rs" => Some(include_str!("../../src/cleanup_plan/renewal_profile.rs")),
        "src/cleanup_plan/renewal_profile/funded.rs" => Some(include_str!("../../src/cleanup_plan/renewal_profile/funded.rs")),
        "src/cleanup_plan/renewal_profile/funded_receiving.rs" => Some(include_str!("../../src/cleanup_plan/renewal_profile/funded_receiving.rs")),
        "src/cleanup_plan/replay/expression_facts/arena/retained/into_tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/arena/retained/into_tests.rs")),
        "src/cleanup_plan/replay/expression_facts/arena/retained_into.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/arena/retained_into.rs")),
        "src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_tests/receiving.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_tests/receiving.rs")),
        "src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/retained_lookup_before_receiving.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/retained_lookup_before_receiving.rs")),
        "src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_cross_phase_before_receiving.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_cross_phase_before_receiving.rs")),
        "src/cleanup_plan/replay/ordinary_receiving.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_receiving.rs")),
        "src/cleanup_plan/replay/ordinary_receiving/caller_before.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_receiving/caller_before.rs")),
        "src/cleanup_plan/replay/ordinary_receiving/tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_receiving/tests.rs")),
        "src/cleanup_plan/replay/skeleton_work/dag_storage.rs" => Some(include_str!("../../src/cleanup_plan/replay/skeleton_work/dag_storage.rs")),
        "src/cleanup_plan/replay/skeleton_work/receiving.rs" => Some(include_str!("../../src/cleanup_plan/replay/skeleton_work/receiving.rs")),
        "src/hir/lexical_exit/owner_funded.rs" => Some(include_str!("../../src/hir/lexical_exit/owner_funded.rs")),
        "src/hir/lexical_exit/owner_funded_receiving.rs" => Some(include_str!("../../src/hir/lexical_exit/owner_funded_receiving.rs")),
        "src/literal_format.rs" => Some(include_str!("../../src/literal_format.rs")),
        "src/literal_format/funded.rs" => Some(include_str!("../../src/literal_format/funded.rs")),
        "src/literal_format/funded_receiving.rs" => Some(include_str!("../../src/literal_format/funded_receiving.rs")),
        "src/string_ops/conditions.rs" => Some(include_str!("../../src/string_ops/conditions.rs")),
        "src/string_ops/conditions/funded.rs" => Some(include_str!("../../src/string_ops/conditions/funded.rs")),
        "src/string_ops/conditions/funded_receiving.rs" => Some(include_str!("../../src/string_ops/conditions/funded_receiving.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/retained_main_289/fixture_helpers.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/retained_main_289/fixture_helpers.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/retained_main_289/lexical_cursor.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/retained_main_289/lexical_cursor.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/retained_main_289/mod.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/retained_main_289/mod.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/retained_main_289/pair_scope_census.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/retained_main_289/pair_scope_census.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/retained_main_289/pair_scope_tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/retained_main_289/pair_scope_tests.rs")),
        "src/bounded_output/stage_debits/route_debits/retained_main_289.rs" => Some(include_str!("../../src/bounded_output/stage_debits/route_debits/retained_main_289.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/frontier_tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/frontier_tests.rs")),
        _ => None,
    }
}

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

    fn restore_whole_before(root: &str, child: &str) -> String {
        // These arguments must be the entire independently compiled current
        // source images, before any metadata or whole-source normalization.
        assert_eq!(root, ROOT);
        assert_eq!(child, CHILD);
        // Undo only this exact later literal scope and line-budget projection.
        // Every original fixed span and complete predecessor assertion follows.
        let mut checkpoint_root = root.to_owned();
        for source in [
            "src/source_reflows_merge/main_e177_inputs.rs",
            "src/source_reflows_merge/main_c883_inputs.rs",
            "src/source_reflows/main043/audit_3.rs",
        ] {
            let literal = format!("    {source:?},\n");
            assert_eq!(checkpoint_root.matches(&literal).count(), 1);
            checkpoint_root = checkpoint_root.replacen(&literal, "", 1);
        }
        for (current, previous) in [
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
        let root = ordered_root.as_str();
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
}
