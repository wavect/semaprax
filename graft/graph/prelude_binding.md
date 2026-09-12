# graph/prelude_binding.rs

- revision_from_source · function · L7-L21 — pub(super) fn revision_from_source(source: &str) -> String
- uses_vec · function · L23-L83 — pub(super) fn uses_vec(program: &ResolvedProgram) -> bool
- type_uses_vec · function · L24-L27 — fn type_uses_vec(ty: &ResolvedType) -> bool
- function_uses_vec · function · L29-L42 — fn function_uses_vec(function: &crate::hir::ResolvedFunction) -> bool
- uses_box · function · L85-L129 — pub(super) fn uses_box(program: &ResolvedProgram) -> bool
- type_uses_box · function · L86-L89 — fn type_uses_box(ty: &ResolvedType) -> bool
- function_uses_box · function · L90-L107 — fn function_uses_box(function: &crate::hir::ResolvedFunction) -> bool
- uses_iterator · function · L134-L180 — pub(super) fn uses_iterator(program: &ResolvedProgram) -> bool
- expression_uses_iterator · function · L135-L137 — fn expression_uses_iterator(expression: &crate::hir::ResolvedExpr) -> bool
- function_uses_iterator · function · L138-L147 — fn function_uses_iterator(function: &crate::hir::ResolvedFunction) -> bool
- uses_vec_v3 · function · L182-L220 — fn uses_vec_v3(program: &ResolvedProgram) -> bool
- is_v3_id · function · L183-L199 — fn is_v3_id(id: &crate::hir::DeclarationId) -> bool
- schema · function · L222-L240 — pub(super) fn schema(program: &ResolvedProgram) -> &'static str
- digest · function · L242-L260 — pub(super) fn digest(program: &ResolvedProgram) -> String
- tests · module · L263-L326 — mod tests
- box_calls_in_contracts_select_v4_without_body_or_type_reachability · function · L268-L293 — fn box_calls_in_contracts_select_v4_without_body_or_type_reachability()
- every_scalar_iterator_binding_selects_v7_and_rejects_a_downgraded_graph · function · L296-L325 — fn every_scalar_iterator_binding_selects_v7_and_rejects_a_downgraded_graph()
