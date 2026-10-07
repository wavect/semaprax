//! Internal collection bodies retain the public scalar-only ABI.
use super::*;

fn internal_type(program: &ResolvedProgram, ty: &ResolvedType, ownership: OwnershipMode) -> bool {
    if hir::is_scalar_resolved_type(ty)
        || crate::variant_guards::copy_variant(&program.declarations, ty)
    {
        ownership == OwnershipMode::Value
    } else if *ty == ResolvedType::String || crate::map_ops::is_collection(ty) {
        matches!(ownership, OwnershipMode::Own | OwnershipMode::Borrow)
    } else {
        *ty == ResolvedType::Str && ownership == OwnershipMode::Borrow
    }
}

pub(super) fn validate_expression(
    program: &ResolvedProgram,
    expression: &ResolvedExpr,
    function: &DeclarationId,
) -> Result<(), Diagnostic> {
    let refuse = || {
        admission(format!(
        "Public Scalar Export Profile v1 function `{function}` contains an unsupported collection body"
    ))
    };
    // HIR is independently validated before this profile check. Owning locals
    // settle through that exact replayed CleanupPlan; no owner crosses a wrapper.
    let mut pending = vec![expression];
    let mut nodes = 0usize;
    while let Some(expression) = pending.pop() {
        nodes += 1;
        if nodes > 65_536 {
            return Err(capacity(
                "Public Scalar Export Profile v1 collection body exceeds 65536 expression nodes",
            ));
        }
        if !internal_type(program, &expression.ty, expression.ownership) {
            return Err(refuse());
        }
        match &expression.kind {
            ResolvedExprKind::Int(_)
            | ResolvedExprKind::Int32(_)
            | ResolvedExprKind::Char(_)
            | ResolvedExprKind::Uint8(_)
            | ResolvedExprKind::Usize(_)
            | ResolvedExprKind::Float32(_)
            | ResolvedExprKind::Float64(_)
            | ResolvedExprKind::Bool(_)
            | ResolvedExprKind::String(_)
            | ResolvedExprKind::Unary { .. }
            | ResolvedExprKind::Binary { .. }
            | ResolvedExprKind::If { .. } => {}
            ResolvedExprKind::Place(place) if place.projections.is_empty() => {}
            ResolvedExprKind::BorrowPlace { operation, .. }
                if operation.as_str() == crate::byte_ops::STRING_AS_STR_ID => {}
            ResolvedExprKind::Call {
                instance,
                callee,
                type_arguments,
                args,
            } => {
                if instance.is_some() {
                    return Err(refuse());
                }
                let signature = if let Some(operation) = crate::map_ops::by_id(callee.as_str()) {
                    operation
                        .resolved_signature(type_arguments)
                        .map(|(params, result)| {
                            (
                                params.into_iter().map(|param| param.ty).collect::<Vec<_>>(),
                                result,
                            )
                        })
                } else if !type_arguments.is_empty() {
                    None
                } else if let Some(operation) = crate::string_ops::by_id(callee.as_str()) {
                    Some((operation.param_types().to_vec(), operation.return_type()))
                } else if let Some(operation) = crate::str_ops::by_id(callee.as_str()) {
                    Some((operation.param_types().to_vec(), operation.return_type()))
                } else {
                    program.resolve_call_target(callee, None).map(|target| {
                        (
                            target.params.iter().map(|param| param.ty.clone()).collect(),
                            target.return_type.clone(),
                        )
                    })
                };
                let Some((params, result)) = signature else {
                    return Err(refuse());
                };
                if expression.ty != result
                    || params.len() != args.len()
                    || params.iter().zip(args).any(|(ty, arg)| ty != &arg.ty)
                {
                    return Err(refuse());
                }
            }
            ResolvedExprKind::Block { statements, .. } => {
                for statement in statements {
                    match statement {
                        ResolvedStatement::Let { binding, .. }
                        | ResolvedStatement::Assign {
                            binding,
                            field: None,
                            ..
                        } if internal_type(program, &binding.ty, binding.ownership) => {}
                        ResolvedStatement::While { .. } => {}
                        _ => return Err(refuse()),
                    }
                }
            }
            ResolvedExprKind::ConstructVariant { .. }
                if crate::variant_guards::copy_variant(&program.declarations, &expression.ty) => {}
            ResolvedExprKind::Match {
                mode: hir::ResolvedMatchMode::Value,
                scrutinee,
                ..
            } if crate::loop_calls::resolved_match_scrutinee_admitted(
                &program.declarations,
                &scrutinee.ty,
            ) => {}
            _ => return Err(refuse()),
        }
        hir::push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    Ok(())
}
