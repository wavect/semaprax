# hir/capacity_probe.rs

- reset_iterative_phase_capacity_high_water · function · L23-L25 — pub(crate) fn reset_iterative_phase_capacity_high_water()
- iterative_phase_capacity_high_water · function · L28-L30 — pub(crate) fn iterative_phase_capacity_high_water() -> [usize; 3]
- note_iterative_phase_capacity · function · L33-L39 — pub(super) fn note_iterative_phase_capacity(index: usize, bytes: usize)
- type_facts_outer_baseline · function · L42-L44 — pub(super) fn type_facts_outer_baseline() -> usize
- validation_scope_owned_capacity · function · L47-L82 — pub(super) fn validation_scope_owned_capacity(
- place_projection_owned_capacity · function · L85-L92 — pub(super) fn place_projection_owned_capacity(projection: &PlaceProjection) -> usize
- resolved_type_owned_capacity · function · L95-L132 — pub(super) fn resolved_type_owned_capacity(ty: &ResolvedType) -> usize
- resolved_place_owned_capacity · function · L135-L143 — pub(super) fn resolved_place_owned_capacity(place: &Place) -> usize
- resolved_expr_owned_capacity · function · L146-L354 — pub(super) fn resolved_expr_owned_capacity(expression: &ResolvedExpr) -> usize
- resolved_binding_owned_capacity · function · L357-L359 — pub(super) fn resolved_binding_owned_capacity(binding: &ResolvedBinding) -> usize
- resolved_record_pattern_field_owned_capacity · function · L362-L385 — pub(super) fn resolved_record_pattern_field_owned_capacity(
- resolved_match_pattern_owned_capacity · function · L388-L429 — pub(super) fn resolved_match_pattern_owned_capacity(pattern: &ResolvedMatchPattern) -> usize
- resolved_statement_owned_capacity · function · L432-L448 — pub(super) fn resolved_statement_owned_capacity(statement: &ResolvedStatement) -> usize
- resolved_field_initializer_owned_capacity · function · L451-L453 — pub(super) fn resolved_field_initializer_owned_capacity(field: &ResolvedFieldInitializer) -> usize
- resolved_match_arm_owned_capacity · function · L456-L463 — pub(super) fn resolved_match_arm_owned_capacity(arm: &ResolvedMatchArm) -> usize
- resolved_field_declaration_owned_capacity · function · L466-L468 — pub(super) fn resolved_field_declaration_owned_capacity(field: &ResolvedFieldDeclaration) -> usize
- resolved_variant_case_owned_capacity · function · L471-L480 — pub(super) fn resolved_variant_case_owned_capacity(case: &ResolvedVariantCaseDeclaration) -> usize
- resolver_scope_owned_capacity · function · L483-L500 — pub(super) fn resolver_scope_owned_capacity(scope: &BTreeMap<String, Binding>) -> usize
