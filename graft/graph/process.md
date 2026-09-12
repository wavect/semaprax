# graph/process.rs

- function_requires · function · L4-L12 — pub(super) fn function_requires(function: &ResolvedFunction) -> bool
- requires · function · L13-L15 — pub(super) fn requires(program: &ResolvedProgram) -> bool
- graph_schema · function · L16-L23 — pub(crate) fn graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic>
- graph_schema_from_parts_and_instances · function · L24-L45 — pub(crate) fn graph_schema_from_parts_and_instances(
- graph_json · function · L46-L111 — pub(super) fn graph_json(
- tests · module · L114-L232 — mod tests
- SOURCE · constant · L119-L132 — const SOURCE: &str = r#"module process.graph;
- process_graph_preserves_environment_and_rejects_forged_contracts · function · L134-L200 — fn process_graph_preserves_environment_and_rejects_forged_contracts()
- process_hir_rejects_forged_argument_and_result_ownership · function · L202-L231 — fn process_hir_rejects_forged_argument_and_result_ownership()
