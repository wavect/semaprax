# hir/iterator_loop.rs

- renewal · module · L3-L3 — mod renewal;
- IteratorLoop · struct · L6-L10 — pub(crate) struct IteratorLoop<'a>
- zero · function · L11-L17 — fn zero(value: &ResolvedExpr) -> bool
- place · function · L18-L20 — fn place(value: &ResolvedExpr, id: &ValueId) -> bool
- cases · function · L21-L29 — fn cases(arms: &[ResolvedMatchArm]) -> bool
- replacement · function · L30-L99 — fn replacement<'a>(
- is_step_reassignment · function · L100-L102 — pub(crate) fn is_step_reassignment(value: &ResolvedExpr, binding: &ValueId) -> bool
- recognize · function · L103-L108 — pub(crate) fn recognize<'a>(
- recognize_scoped · function · L109-L175 — fn recognize_scoped<'a>(
- function_contains · function · L176-L185 — pub(crate) fn function_contains(function: &ResolvedFunction) -> bool
- validate_function · function · L188-L252 — pub(crate) fn validate_function(
- step_element · function · L254-L274 — fn step_element<'a>(
- template_contains · function · L277-L316 — pub(crate) fn template_contains(template: &ResolvedFunctionTemplate) -> bool
