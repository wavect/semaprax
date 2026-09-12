# workspace_graph/expected_projection/identity_slots.rs

- ast_program_identity_slots · function · L10-L26 — pub(super) fn ast_program_identity_slots(program: &Program) -> Result<usize, Vec<Diagnostic>>
- ast_type_declaration_identity_slots · function · L28-L56 — pub(super) fn ast_type_declaration_identity_slots(
- ast_function_identity_slots · function · L58-L72 — pub(super) fn ast_function_identity_slots(function: &Function) -> Result<usize, Vec<Diagnostic>>
- ast_type_identity_slots · function · L74-L83 — pub(super) fn ast_type_identity_slots(ty: &Type) -> Result<usize, Vec<Diagnostic>>
- fixed_expression_identity_slots · function · L92-L105 — fn fixed_expression_identity_slots(kind: &ExprKind) -> usize
- BASE · constant · L93-L93 — const BASE: usize = 3;
- ast_expr_identity_slots · function · L107-L243 — fn ast_expr_identity_slots(expression: &Expr) -> Result<usize, Vec<Diagnostic>>
- ast_pattern_identity_slots · function · L245-L277 — fn ast_pattern_identity_slots(
- record_pattern_identity_slots · function · L279-L290 — fn record_pattern_identity_slots(
- scalar_expression_identity_discount · function · L298-L323 — pub(super) fn scalar_expression_identity_discount(kind: &ExprKind) -> usize
- scalar_result · function · L328-L354 — fn scalar_result(expression: &Expr, depth: usize) -> bool
- scalar_result_tests · module · L357-L377 — mod scalar_result_tests
- identity_prebound_control_flow_requires_every_result_to_be_scalar · function · L359-L376 — fn identity_prebound_control_flow_requires_every_result_to_be_scalar()
