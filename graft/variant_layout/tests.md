# variant_layout/tests.rs

- SOURCE · constant · L7-L22 — const SOURCE: &str = r#"
- GENERIC_SOURCE · constant · L24-L45 — const GENERIC_SOURCE: &str = r#"
- resolved · function · L47-L50 — fn resolved() -> hir::ResolvedProgram
- nominal · function · L52-L57 — fn nominal(id: &str, arguments: Vec<ResolvedType>) -> ResolvedType
- concrete_generic_and_prelude_instances_have_distinct_cached_layouts · function · L60-L163 — fn concrete_generic_and_prelude_instances_have_distinct_cached_layouts()
- native64_and_wasm32_layouts_freeze_tag_payload_and_bool_profiles · function · L166-L221 — fn native64_and_wasm32_layouts_freeze_tag_payload_and_bool_profiles()
- direct_owned_bytes_payload_uses_target_carrier_and_remains_non_copy · function · L224-L462 — fn direct_owned_bytes_payload_uses_target_carrier_and_remains_non_copy()
- hostile_layout_and_declaration_mutations_are_rejected_independently · function · L465-L520 — fn hostile_layout_and_declaration_mutations_are_rejected_independently()
- compiler_owned_two_owned_result_rejects_retained_declaration_drift · function · L523-L553 — fn compiler_owned_two_owned_result_rejects_retained_declaration_drift()
