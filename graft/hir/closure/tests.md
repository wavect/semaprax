# hir/closure/tests.rs

- SOURCE · constant · L3-L13 — const SOURCE: &str = r#"
- resolved · function · L15-L18 — fn resolved() -> ResolvedProgram
- creation · function · L20-L33 — fn creation(program: &mut ResolvedProgram) -> &mut ResolvedExpr
- rejects · function · L35-L37 — fn rejects(program: &ResolvedProgram)
- closures_hir_replays_snapshot_inventory_and_private_body · function · L40-L60 — fn closures_hir_replays_snapshot_inventory_and_private_body()
- closures_hir_rejects_effectful_capture_creation · function · L63-L75 — fn closures_hir_rejects_effectful_capture_creation()
- closures_hir_rejects_capture_type_and_order_forgery · function · L78-L96 — fn closures_hir_rejects_capture_type_and_order_forgery()
- closures_hir_rejects_private_binding_and_body_identity_forgery · function · L99-L120 — fn closures_hir_rejects_private_binding_and_body_identity_forgery()
- closures_hir_rejects_authored_private_identity_collision · function · L123-L132 — fn closures_hir_rejects_authored_private_identity_collision()
- generic_resolved · function · L134-L146 — fn generic_resolved() -> ResolvedProgram
- generic_closure_materialization_uses_distinct_execution_identities · function · L149-L162 — fn generic_closure_materialization_uses_distinct_execution_identities()
- generic_closure_template_rejects_forged_capture_scope_and_shape · function · L165-L199 — fn generic_closure_template_rejects_forged_capture_scope_and_shape()
- generic_closure_unused_template_still_checks_every_scalar_substitution · function · L202-L219 — fn generic_closure_unused_template_still_checks_every_scalar_substitution()
