# hir/generic_variant.rs

- substitutions · function · L7-L9 — pub(crate) fn substitutions() -> Vec<Vec<ResolvedType>>
- arguments · function · L10-L12 — pub(crate) fn arguments(arguments: &[ResolvedType]) -> bool
- concrete · function · L13-L17 — pub(crate) fn concrete(declarations: &DeclarationIndex, ty: &ResolvedType) -> bool
- slot · function · L18-L36 — pub(crate) fn slot(
- profile · function · L37-L74 — pub(crate) fn profile(program: &ResolvedProgram, template: &ResolvedFunctionTemplate) -> bool
- bounded_template · function · L78-L100 — pub(crate) fn bounded_template<'a>(
- match_result · function · L101-L117 — pub(crate) fn match_result(
- match_result_execution · function · L118-L145 — pub(crate) fn match_result_execution(
- symbolic_slot · function · L147-L155 — pub(crate) fn symbolic_slot(ty: &ResolvedType, owner: &DeclarationId, count: usize) -> bool
- concrete_signature · function · L159-L217 — pub(crate) fn concrete_signature(
- iterator_match_result · function · L220-L229 — fn iterator_match_result(
- template_substitutions · function · L231-L237 — fn template_substitutions(template: &ResolvedFunctionTemplate) -> Vec<Vec<ResolvedType>>
