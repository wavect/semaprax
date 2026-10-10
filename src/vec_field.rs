//! Checked literal field selection has no runtime selector or owning result.
pub(crate) const NAME: &str = "vec_field";
pub(crate) const ID: &str = "core.vec.field";

pub(crate) fn operation_id() -> &'static crate::hir::DeclarationId {
    static IDENTITY: std::sync::LazyLock<crate::hir::DeclarationId> =
        std::sync::LazyLock::new(|| crate::hir::DeclarationId::new(ID));
    &IDENTITY
}
pub(crate) fn resolved_params(args: &[crate::hir::ResolvedExpr]) -> Vec<crate::hir::ResolvedParam> {
    args.iter()
        .enumerate()
        .map(|(index, arg)| crate::hir::ResolvedParam {
            id: crate::hir::ValueId::intrinsic_parameter(ID, index),
            name: format!("value{index}"),
            ownership: if index == 0 {
                crate::hir::OwnershipMode::Borrow
            } else {
                crate::hir::OwnershipMode::Value
            },
            ty: arg.ty.clone(),
            span: arg.span,
        })
        .collect()
}

pub(crate) fn expression_uses(root: &crate::hir::ResolvedExpr) -> bool {
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        if matches!(
            expression.kind,
            crate::hir::ResolvedExprKind::VecFieldRead { .. }
        ) {
            return true;
        }
        crate::hir::push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    false
}
