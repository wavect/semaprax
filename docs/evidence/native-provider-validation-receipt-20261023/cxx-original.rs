#![allow(dead_code)]
#[path = "/Volumes/Semaprax/worktrees/ci-canonical-54e/tests/support/owned_mixed_arity_product.rs"] mod owned_mixed_arity_product;
#[path = "/Volumes/Semaprax/worktrees/ci-canonical-54e/tests/support/owned_result_product.rs"] mod owned_result_product;
#[path = "/Volumes/Semaprax/worktrees/ci-canonical-54e/tests/support/owned_tuple_product.rs"] mod owned_tuple_product;
#[path = "/Volumes/Semaprax/worktrees/ci-canonical-54e/tests/project/cxx_owned_data_package.rs"] mod cxx_owned_data_package;

#[test]
fn diagnostic_genuine_current_ir_sizes() {
    eprintln!("ACTUAL_IR_LAYOUT {}", serde_json::json!({
        "resolved_expr":std::mem::size_of::<semaprax::hir::ResolvedExpr>(),
        "resolved_expr_kind":std::mem::size_of::<semaprax::hir::ResolvedExprKind>(),
        "resolved_type":std::mem::size_of::<semaprax::hir::ResolvedType>(),
        "expression_id":std::mem::size_of::<semaprax::hir::ExpressionId>(),
        "declaration_id":std::mem::size_of::<semaprax::hir::DeclarationId>(),
        "native_rust_call":std::mem::size_of::<semaprax::hir::ResolvedNativeRustImportCall>(),
        "host_command_call":std::mem::size_of::<semaprax::hir::ResolvedHostCommandCall>(),
        "statement":std::mem::size_of::<semaprax::hir::ResolvedStatement>(),
        "binding":std::mem::size_of::<semaprax::hir::ResolvedBinding>(),
        "match_arm":std::mem::size_of::<semaprax::hir::ResolvedMatchArm>(),
        "match_pattern":std::mem::size_of::<semaprax::hir::ResolvedMatchPattern>(),
    }));
}
