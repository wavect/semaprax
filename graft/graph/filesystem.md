# graph/filesystem.rs

- function_requires · function · L5-L13 — pub(super) fn function_requires(function: &ResolvedFunction) -> bool
- function_requires_v2 · function · L14-L22 — fn function_requires_v2(function: &ResolvedFunction) -> bool
- requires · function · L23-L29 — pub(super) fn requires(program: &ResolvedProgram) -> bool
- graph_schema · function · L30-L46 — pub(crate) fn graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic>
- graph_schema_from_parts_and_instances · function · L47-L74 — pub(crate) fn graph_schema_from_parts_and_instances(
- graph_json · function · L75-L140 — pub(super) fn graph_json(
- string_array · function · L141-L150 — pub(super) fn string_array(values: &[String]) -> String
- tests · module · L153-L257 — mod tests
- SOURCE · constant · L156-L165 — const SOURCE: &str = r#"
- filesystem_v2_graph_and_profile_are_additive_and_closed · function · L167-L199 — fn filesystem_v2_graph_and_profile_are_additive_and_closed()
- filesystem_graph_binds_checked_operations_and_rejects_remint · function · L201-L237 — fn filesystem_graph_binds_checked_operations_and_rejects_remint()
- filesystem_effect_and_contract_admission_remain_closed · function · L239-L256 — fn filesystem_effect_and_contract_admission_remain_closed()
