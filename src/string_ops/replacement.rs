//! General whole String replacement sites, derived from typed HIR rather than plans.
use crate::hir::{
    ExpressionId, OwnershipMode, ResolvedBinding, ResolvedExpr, ResolvedExprKind, ResolvedFunction,
    ResolvedStatement, ResolvedType,
};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn admitted(binding: &ResolvedBinding, value: &ResolvedExpr) -> bool {
    binding.ty == ResolvedType::String
        && binding.ownership == OwnershipMode::Own
        && value.ty == ResolvedType::String
        && value.ownership == OwnershipMode::Own
}
pub(crate) fn bindings(function: &ResolvedFunction) -> BTreeMap<ExpressionId, &ResolvedBinding> {
    let mut mutable = BTreeSet::new();
    let mut assignments = Vec::new();
    let mut pending = vec![&function.body];
    while let Some(expression) = pending.pop() {
        if matches!(expression.kind, ResolvedExprKind::Closure { .. }) {
            continue;
        }
        if let ResolvedExprKind::Block { statements, .. } = &expression.kind {
            for statement in statements {
                match statement {
                    ResolvedStatement::Let {
                        binding,
                        mutable: true,
                        ..
                    } => {
                        mutable.insert(binding.id.clone());
                    }
                    ResolvedStatement::Assign {
                        binding,
                        field: None,
                        value,
                        ..
                    } if admitted(binding, value)
                        && !super::is_same_owner_concat_hir(value, &binding.id) =>
                    {
                        assignments.push((value.id.clone(), binding));
                    }
                    _ => {}
                }
            }
        }
        crate::hir::push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    assignments
        .into_iter()
        .filter(|(_, binding)| mutable.contains(&binding.id))
        .collect()
}
pub(crate) fn binding<'a>(
    function: &'a ResolvedFunction,
    at: &ExpressionId,
) -> Option<&'a ResolvedBinding> {
    bindings(function).remove(at)
}
pub(crate) fn requires(function: &ResolvedFunction) -> bool {
    !bindings(function).is_empty()
}
