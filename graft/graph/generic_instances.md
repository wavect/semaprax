# graph/generic_instances.rs

- to_legacy_json · function · L9-L27 — pub fn to_legacy_json(program: &Program) -> Result<String, Vec<Diagnostic>>
- legacy_context_json · function · L30-L39 — pub fn legacy_context_json(
- to_legacy_hir_json · function · L42-L61 — pub(crate) fn to_legacy_hir_json(
- verify_json · function · L65-L74 — pub fn verify_json(program: &Program, submitted: &str) -> Result<(), Vec<Diagnostic>>
- legacy_graph_json · function · L76-L107 — pub(super) fn legacy_graph_json(
- pre_filesystem_graph_json · function · L109-L208 — pub(super) fn pre_filesystem_graph_json(
- digest · function · L210-L219 — fn digest(domain: &str, parts: &[&str]) -> String
- identity · function · L221-L229 — fn identity(revision: &str, template: &DeclarationId, args: &[ResolvedType]) -> String
- leaves · function · L231-L256 — fn leaves(shape: &FieldLivenessShape, path: &mut Vec<String>, output: &mut Vec<Value>)
- signature · function · L258-L287 — fn signature(
- instance_json · function · L289-L438 — pub(super) fn instance_json(
- type_facts_json · function · L440-L527 — pub(super) fn type_facts_json(
