use super::*;
use std::collections::BTreeSet;

pub(crate) fn validate_shape(
    program: &ResolvedProgram,
    expression: &ResolvedExpr,
) -> Result<(), Diagnostic> {
    validate_shape_scoped(program, expression, None)
}

pub(crate) fn validate_shape_scoped(
    program: &ResolvedProgram,
    expression: &ResolvedExpr,
    owner: Option<&DeclarationId>,
) -> Result<(), Diagnostic> {
    let scalar = |ty: &ResolvedType| {
        super::super::function_value::scalar(ty)
            || owner.is_some_and(|owner| super::super::generic_collection::parameter(ty, owner))
    };
    let ResolvedExprKind::Closure {
        parameters,
        captures,
        body,
    } = &expression.kind
    else {
        return Err(hir_error("expected closure"));
    };
    let ResolvedType::Function {
        parameters: types,
        result,
    } = &expression.ty
    else {
        return Err(hir_error("closure signature is missing"));
    };
    if expression.ownership != OwnershipMode::Value
        || !(super::super::function_value::is_signature(&expression.ty)
            || owner.is_some_and(|owner| {
                super::super::generic_collection::callback(&expression.ty, owner)
            }))
        || captures.len() > MAX_CAPTURES
        || parameters.len() != types.len()
        || body.ty != **result
    {
        return Err(hir_error("closure signature or capture bound is invalid"));
    }
    let target = closure_id(&expression.id);
    if program.declarations.declaration(&target).is_some() {
        return Err(hir_error("closure identity collides with a declaration"));
    }
    let execution = FunctionExecutionId::Monomorphic(target);
    let mut names = BTreeSet::new();
    let mut captured_roots = Vec::new();
    for (index, capture) in captures.iter().enumerate() {
        let ResolvedExprKind::Place(place) = &capture.value.kind else {
            return Err(hir_error("closure capture must snapshot one scalar place"));
        };
        if !place.projections.is_empty()
            || capture.binding.id != ValueId::parameter(&execution, index)
            || capture.binding.ty != capture.value.ty
            || capture.binding.ownership != OwnershipMode::Value
            || capture.value.ownership != OwnershipMode::Value
            || !scalar(&capture.value.ty)
            || !names.insert(capture.binding.name.as_str())
        {
            return Err(hir_error("closure capture binding is not canonical"));
        }
        captured_roots.push(&place.root);
    }
    if captured_roots.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(hir_error(
            "closure capture roots must be unique and identity ordered",
        ));
    }
    for (index, parameter) in parameters.iter().enumerate() {
        if parameter.id != ValueId::parameter(&execution, captures.len() + index)
            || parameter.ty != types[index]
            || parameter.ownership != OwnershipMode::Value
            || !names.insert(parameter.name.as_str())
        {
            return Err(hir_error("closure parameter binding is not canonical"));
        }
    }
    let capture_ids = captures
        .iter()
        .map(|capture| &capture.binding.id)
        .collect::<BTreeSet<_>>();
    let mut used = BTreeSet::new();
    let mut pending = vec![body.as_ref()];
    let mut count = 0usize;
    while let Some(node) = pending.pop() {
        count += 1;
        if count > 4096 || node.ownership != OwnershipMode::Value || !scalar(&node.ty) {
            return Err(hir_error(
                "closure body is outside the bounded scalar profile",
            ));
        }
        match &node.kind {
            ResolvedExprKind::Int(_)
            | ResolvedExprKind::Int32(_)
            | ResolvedExprKind::Uint8(_)
            | ResolvedExprKind::Usize(_)
            | ResolvedExprKind::Char(_)
            | ResolvedExprKind::Float32(_)
            | ResolvedExprKind::Float64(_)
            | ResolvedExprKind::Bool(_) => {}
            ResolvedExprKind::Place(place) => {
                if !place.projections.is_empty() {
                    return Err(hir_error("closure body cannot project captured places"));
                }
                if capture_ids.contains(&place.root) {
                    used.insert(&place.root);
                }
            }
            ResolvedExprKind::Unary { value, .. } => pending.push(value),
            ResolvedExprKind::Binary { left, right, .. } => {
                pending.push(right);
                pending.push(left);
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                pending.push(else_branch);
                pending.push(then_branch);
                pending.push(condition);
            }
            ResolvedExprKind::Call {
                callee,
                instance,
                type_arguments,
                args,
            } => {
                if instance.is_some()
                    || !type_arguments.is_empty()
                    || program
                        .functions
                        .iter()
                        .find(|f| f.id == *callee)
                        .and_then(super::super::function_value::signature)
                        .is_none()
                {
                    return Err(hir_error(
                        "closure body call is not an ordinary scalar target",
                    ));
                }
                pending.extend(args.iter().rev());
            }
            ResolvedExprKind::Block { statements, tail } => {
                pending.push(tail);
                for statement in statements.iter().rev() {
                    match statement {
                        ResolvedStatement::Let { binding, value, .. } => {
                            if !scalar(&binding.ty) || binding.ownership != OwnershipMode::Value {
                                return Err(hir_error("closure locals must be scalar"));
                            }
                            pending.push(value);
                        }
                        ResolvedStatement::Assign {
                            binding,
                            field,
                            value,
                            ..
                        } => {
                            if field.is_some() || capture_ids.contains(&binding.id) {
                                return Err(hir_error("closure snapshots cannot be mutated"));
                            }
                            pending.push(value);
                        }
                        ResolvedStatement::While {
                            condition, body, ..
                        } => {
                            pending.push(body);
                            pending.push(condition);
                        }
                        _ => return Err(hir_error("closure body contains unsupported statements")),
                    }
                }
            }
            _ => {
                return Err(hir_error(
                    "closure body contains unsupported or nested closure expression",
                ))
            }
        }
    }
    if used != capture_ids {
        return Err(hir_error("closure inventory contains an unused capture"));
    }
    Ok(())
}
