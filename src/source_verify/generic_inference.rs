//! Bounded argument-directed inference. This pass observes type facts only;
//! ordinary argument checking remains the sole source ownership authority.
use super::binding::Binding;
use crate::ast::{Expr, ExprKind, Function, Type};
use std::collections::HashMap;

pub(super) fn arguments(
    current: &Function,
    target: &Function,
    args: &[Expr],
    bindings: &HashMap<String, Binding>,
) -> Option<Vec<Type>> {
    if !current.type_parameters.is_empty()
        || target.type_parameters.len() != 1
        || args.len() != target.params.len()
    {
        return None;
    }
    let parameter = &target.type_parameters[0].name;
    let mut inferred = None;
    for (formal, expression) in target.params.iter().zip(args) {
        let actual = evidence(expression, bindings)?;
        let mut pending = vec![(&formal.ty, &actual)];
        while let Some((formal, actual)) = pending.pop() {
            if matches!(formal, Type::Named { name, arguments } if name == parameter && arguments.is_empty())
            {
                if !copy_scalar(actual) || inferred.as_ref().is_some_and(|old| old != actual) {
                    return None;
                }
                inferred = Some(actual.clone());
            } else if let (
                Type::Named {
                    name: a,
                    arguments: aa,
                },
                Type::Named {
                    name: b,
                    arguments: ba,
                },
            ) = (formal, actual)
            {
                if a != b || aa.len() != ba.len() {
                    return None;
                }
                pending.extend(aa.iter().zip(ba));
            } else if formal != actual {
                return None;
            }
        }
    }
    inferred.map(|ty| vec![ty])
}

fn copy_scalar(ty: &Type) -> bool {
    matches!(
        ty,
        Type::I64
            | Type::I32
            | Type::U8
            | Type::Usize
            | Type::Char
            | Type::F32
            | Type::F64
            | Type::Bool
    )
}

fn evidence(expression: &Expr, bindings: &HashMap<String, Binding>) -> Option<Type> {
    Some(match &expression.kind {
        ExprKind::Int(_) => Type::I64,
        ExprKind::Int32(_) => Type::I32,
        ExprKind::Uint8(_) => Type::U8,
        ExprKind::Usize(_) => Type::Usize,
        ExprKind::Char(_) => Type::Char,
        ExprKind::Float32(_) => Type::F32,
        ExprKind::Float64(_) => Type::F64,
        ExprKind::Bool(_) => Type::Bool,
        ExprKind::Var(name) => bindings.get(name)?.ty.clone(),
        ExprKind::ConstructRecord {
            type_name,
            type_arguments,
            ..
        }
        | ExprKind::ConstructVariant {
            type_name,
            type_arguments,
            ..
        } => Type::Named {
            name: type_name.clone(),
            arguments: type_arguments.clone(),
        },
        _ => return None,
    })
}
