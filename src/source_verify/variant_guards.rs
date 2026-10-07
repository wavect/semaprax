//! Source twin of the authenticated Copy Variant Guards v1 profile.
use super::{binding::Binding, diagnostics::is_scalar_source_type, type_table::TypeTable};
use crate::ast::{Expr, ExprKind, MatchMode, MatchPattern, ParamMode, Type};
use std::collections::HashMap;

pub(super) fn copy_variant(types: &TypeTable<'_>, ty: &Type) -> bool {
    let (Type::Named { name, arguments }, Some(cases)) = (ty, types.variant_cases(ty)) else {
        return false;
    };
    let Some(declaration) = types.declaration(name) else {
        return false;
    };
    cases.iter().all(|case| {
        case.fields.iter().all(|field| {
            TypeTable::substitute_variant_type(declaration, arguments, &field.ty)
                .is_some_and(|ty| is_scalar_source_type(&ty))
        })
    })
}

pub(super) fn guard_shape(guard: &Expr, bindings: &HashMap<String, Binding>) -> bool {
    let mut pending = vec![guard];
    while let Some(expression) = pending.pop() {
        match &expression.kind {
            ExprKind::Int(_)
            | ExprKind::Int32(_)
            | ExprKind::Char(_)
            | ExprKind::Uint8(_)
            | ExprKind::Usize(_)
            | ExprKind::Float32(_)
            | ExprKind::Float64(_)
            | ExprKind::Bool(_) => {}
            ExprKind::Var(name)
                if bindings.get(name).is_some_and(|binding| {
                    binding.mode == ParamMode::Value && is_scalar_source_type(&binding.ty)
                }) => {}
            ExprKind::Unary { value, .. } => pending.push(value),
            ExprKind::Binary { left, right, .. } => {
                pending.push(right);
                pending.push(left);
            }
            _ => return false,
        }
    }
    true
}

pub(super) fn admission(
    types: &TypeTable<'_>,
    ty: &Type,
    mode: MatchMode,
    pattern: &MatchPattern,
) -> bool {
    mode == MatchMode::Value
        && copy_variant(types, ty)
        && matches!(pattern, MatchPattern::Variant { .. })
}
