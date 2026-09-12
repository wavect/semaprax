# hir/generic_collection.rs

- scalar · function · L3-L5 — pub(crate) fn scalar(ty: &ResolvedType) -> bool
- slot · function · L6-L11 — pub(crate) fn slot(ty: &ResolvedType, owner: &DeclarationId, count: usize) -> bool
- parameter · function · L12-L15 — pub(crate) fn parameter(ty: &ResolvedType, owner: &DeclarationId, count: usize) -> bool
- callback · function · L16-L19 — pub(crate) fn callback(ty: &ResolvedType, owner: &DeclarationId, count: usize) -> bool
- profile · function · L20-L39 — pub(crate) fn profile(template: &ResolvedFunctionTemplate) -> bool
- arguments · function · L40-L42 — pub(crate) fn arguments(arguments: &[ResolvedType]) -> bool
- concrete_signature · function · L43-L52 — pub(crate) fn concrete_signature(function: &super::ResolvedFunction) -> bool
- arguments_for_count · function · L54-L56 — pub(crate) fn arguments_for_count(arguments: &[ResolvedType], count: usize) -> bool
- substitutions · function · L57-L63 — pub(crate) fn substitutions(count: usize) -> Vec<Vec<ResolvedType>>
- source_parameter · function · L64-L80 — pub(super) fn source_parameter(
- source_count · function · L81-L87 — pub(super) fn source_count(program: &crate::ast::Program, owner: &DeclarationId) -> usize
- tests · module · L90-L146 — mod tests
- generic_iterator_operation_scope_and_cartesian_arguments_are_exact · function · L94-L145 — fn generic_iterator_operation_scope_and_cartesian_arguments_are_exact()
