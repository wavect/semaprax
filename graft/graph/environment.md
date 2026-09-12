# graph/environment.rs

- function_requires · function · L4-L12 — pub(super) fn function_requires(function: &ResolvedFunction) -> bool
- requires · function · L13-L24 — pub(super) fn requires(program: &ResolvedProgram) -> bool
- graph_schema · function · L25-L32 — pub(crate) fn graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic>
- graph_schema_from_parts_and_instances · function · L33-L54 — pub(crate) fn graph_schema_from_parts_and_instances(
- graph_json · function · L55-L104 — pub(super) fn graph_json(
- tests · module · L107-L212 — mod tests
- SOURCE · constant · L112-L123 — const SOURCE: &str = r#"module environment.graph;
- environment_graph_and_profile_are_additive_and_closed · function · L125-L211 — fn environment_graph_and_profile_are_additive_and_closed()
- loop_tests · module · L215-L243 — mod loop_tests
- environment_lookups_and_named_text_reads_repeat_inside_loop · function · L217-L242 — fn environment_lookups_and_named_text_reads_repeat_inside_loop()
