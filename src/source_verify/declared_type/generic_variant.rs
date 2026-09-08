//! Authored variant signature and expression profile.
use super::*;

pub(in crate::source_verify) fn slot(
    function: &Function,
    ty: &Type,
    types: &TypeTable<'_>,
) -> bool {
    let [parameter] = function.type_parameters.as_slice() else {
        return false;
    };
    let Type::Named { name, arguments } = ty else {
        return false;
    };
    if matches!(name.as_str(), "Option" | "Result")
        || !matches!(arguments.as_slice(), [Type::Bytes, Type::Named { name, arguments }] if name == &parameter.name && arguments.is_empty())
    {
        return false;
    }
    owned_record_function_substitutions(1)
        .iter()
        .all(|replacement| {
            types.is_flat_owned_byte_variant(&Type::Named {
                name: name.clone(),
                arguments: vec![Type::Bytes, replacement[0].clone()],
            })
        })
}
pub(in crate::source_verify) fn profile(function: &Function, types: &TypeTable<'_>) -> bool {
    let [parameter] = function.type_parameters.as_slice() else {
        return false;
    };
    let scalar = |ty: &Type| {
        crate::vec_ops::ast_element_is_admitted(ty)
            || matches!(ty, Type::Named { name, arguments } if name == &parameter.name && arguments.is_empty())
    };
    let owned = function
        .params
        .iter()
        .filter(|p| p.mode == ParamMode::Own)
        .collect::<Vec<_>>();
    function.effects.is_empty()
        && owned.len() == 1
        && slot(function, &owned[0].ty, types)
        && (slot(function, &function.return_type, types) || scalar(&function.return_type))
        && function.params.iter().all(|p| {
            (p.mode == ParamMode::Own && slot(function, &p.ty, types))
                || (p.mode == ParamMode::Value && scalar(&p.ty))
        })
}
pub(in crate::source_verify) fn body(
    function: &Function,
    types: &TypeTable<'_>,
    expression: &Expr,
) -> bool {
    if !profile(function, types) {
        return false;
    }
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        if generic_function_expression_is_direct_scalar(expression) {
            continue;
        }
        match &expression.kind {
            ExprKind::ConstructVariant {
                type_name,
                type_arguments,
                fields,
                ..
            } => {
                if !slot(
                    function,
                    &Type::Named {
                        name: type_name.clone(),
                        arguments: type_arguments.clone(),
                    },
                    types,
                ) {
                    return false;
                }
                pending.extend(fields.iter().map(|field| &field.value));
            }
            ExprKind::Match {
                scrutinee, arms, ..
            } => {
                pending.push(scrutinee);
                for arm in arms {
                    if arm.guard.is_some()
                        || !matches!(arm.pattern, crate::ast::MatchPattern::Variant { .. })
                    {
                        return false;
                    }
                    pending.push(&arm.value);
                }
            }
            ExprKind::Call { args, .. } => pending.extend(args),
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => pending.extend([
                condition.as_ref(),
                then_branch.as_ref(),
                else_branch.as_ref(),
            ]),
            ExprKind::Block { statements, tail } => {
                pending.push(tail);
                for statement in statements {
                    match statement {
                        crate::ast::Statement::Let { value, .. } => pending.push(value),
                        _ => return false,
                    }
                }
            }
            _ => return false,
        }
    }
    true
}

pub(in crate::source_verify) fn parameter_slot(
    ty: &Type,
    parameters: &HashSet<&str>,
    types: &TypeTable<'_>,
) -> bool {
    let Type::Named { name, arguments } = ty else {
        return false;
    };
    parameters.len() == 1
        && !matches!(name.as_str(), "Option" | "Result")
        && matches!(arguments.as_slice(),[Type::Bytes,Type::Named{name,arguments}] if arguments.is_empty()&&parameters.contains(name.as_str()))
        && owned_record_function_substitutions(1).iter().all(|args| {
            types.is_flat_owned_byte_variant(&Type::Named {
                name: name.clone(),
                arguments: vec![Type::Bytes, args[0].clone()],
            })
        })
}
pub(in crate::source_verify) fn match_result(
    template: Option<&Function>,
    types: &TypeTable<'_>,
    mode: crate::ast::MatchMode,
    ty: &Type,
    ownership: ParamMode,
) -> bool {
    if mode == crate::ast::MatchMode::Own
        && ownership == ParamMode::Own
        && matches!(ty, Type::Named { name, arguments } if name == "IterStep"
            && matches!(arguments.as_slice(), [element] if crate::iterator_ops::ast_element_is_admitted(element)))
        && types.is_flat_owned_byte_variant(ty)
    {
        return true;
    }
    template.is_some_and(|function| profile(function, types))
        && ((ownership == ParamMode::Value && crate::vec_ops::ast_element_is_admitted(ty))
            || (mode == crate::ast::MatchMode::Own
                && ownership == ParamMode::Own
                && types.is_flat_owned_byte_variant(ty)
                && !matches!(ty,Type::Named{name,..} if matches!(name.as_str(),"Option"|"Result"))))
}
