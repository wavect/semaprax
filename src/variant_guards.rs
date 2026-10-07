//! Copy Variant Guards v1: structural admission, never an authority shortcut.
use crate::hir::{
    self, DeclarationIndex, OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedMatchMode,
    ResolvedMatchPattern, ResolvedType,
};

pub(crate) fn copy_variant(declarations: &DeclarationIndex, ty: &ResolvedType) -> bool {
    matches!(ty, ResolvedType::Nominal { .. })
        && crate::loop_calls::resolved_match_scrutinee_admitted(declarations, ty)
}

/// Operators and exact scalar place reads allocate and consume no owners.
/// The ordinary validator must still authenticate every expression and place.
pub(crate) fn guard_shape(guard: &ResolvedExpr) -> bool {
    let mut pending = vec![guard];
    while let Some(expression) = pending.pop() {
        if expression.ownership != OwnershipMode::Value
            || !hir::is_scalar_resolved_type(&expression.ty)
        {
            return false;
        }
        match &expression.kind {
            ResolvedExprKind::Int(_)
            | ResolvedExprKind::Int32(_)
            | ResolvedExprKind::Char(_)
            | ResolvedExprKind::Uint8(_)
            | ResolvedExprKind::Usize(_)
            | ResolvedExprKind::Float32(_)
            | ResolvedExprKind::Float64(_)
            | ResolvedExprKind::Bool(_) => {}
            ResolvedExprKind::Place(place) if place.projections.is_empty() => {}
            ResolvedExprKind::Unary { value, .. } => pending.push(value),
            ResolvedExprKind::Binary { left, right, .. } => {
                pending.push(right);
                pending.push(left);
            }
            _ => return false,
        }
    }
    true
}

pub(crate) fn admitted(
    declarations: &DeclarationIndex,
    ty: &ResolvedType,
    mode: ResolvedMatchMode,
    pattern: &ResolvedMatchPattern,
    guard: &ResolvedExpr,
) -> bool {
    mode == ResolvedMatchMode::Value
        && copy_variant(declarations, ty)
        && matches!(pattern, ResolvedMatchPattern::Variant { .. })
        && guard_shape(guard)
}
