# hir/closure.rs

- MAX_CAPTURES · constant · L4-L4 — pub const MAX_CAPTURES: usize = 8;
- closure_id · function · L6-L8 — pub fn closure_id(expression: &ExpressionId) -> DeclarationId
- closure_function · function · L13-L62 — pub fn closure_function(
- inventory · function · L66-L82 — pub fn inventory(program: &ResolvedProgram) -> Vec<&ResolvedExpr>
- requires_closures · function · L84-L86 — pub fn requires_closures(program: &ResolvedProgram) -> bool
- requires_closure_projection · function · L96-L98 — pub fn requires_closure_projection(program: &ResolvedProgram) -> bool
- template_has_closure · function · L100-L114 — pub fn template_has_closure(template: &super::ResolvedFunctionTemplate) -> bool
- resolve · module · L116-L116 — mod resolve;
- validation · module · L117-L117 — mod validation;
- tests · module · L122-L122 — mod tests;
- materialize · module · L124-L124 — mod materialize;
