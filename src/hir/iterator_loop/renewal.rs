//! Exact same-owner renewal inside authenticated bounded loops.
use super::*;
use std::collections::BTreeMap;

pub(crate) fn function_requires_renewal(function: &ResolvedFunction) -> bool {
    !bindings(&function.body, None).is_empty()
}
pub(crate) fn function_requires_record_renewal(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> bool {
    !record_bindings(program, &function.body).is_empty()
}
pub(crate) fn function_has_record_renewal_outside_loop(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> bool {
    let mut pending = vec![(&function.body, false)];
    while let Some((expression, in_loop)) = pending.pop() {
        if let ResolvedExprKind::Block { statements, tail } = &expression.kind {
            pending.push((tail, in_loop));
            for statement in statements {
                match statement {
                    ResolvedStatement::Let { value, .. } => pending.push((value, in_loop)),
                    ResolvedStatement::Assign {
                        binding,
                        field: None,
                        value,
                        ..
                    } => {
                        if !in_loop && is_record_owner_renewal(program, binding, value) {
                            return true;
                        }
                        pending.push((value, in_loop));
                    }
                    ResolvedStatement::Assign { value, .. } => pending.push((value, in_loop)),
                    ResolvedStatement::Unsafe { body, .. } => pending.push((body, in_loop)),
                    ResolvedStatement::While {
                        condition, body, ..
                    } => {
                        pending.push((condition, in_loop));
                        pending.push((body, true));
                    }
                }
            }
        } else {
            let mut children = Vec::new();
            push_resolved_expression_children_in_authored_order(expression, &mut children);
            pending.extend(children.into_iter().map(|child| (child, in_loop)));
        }
    }
    false
}
pub(crate) fn function_may_require_record_renewal(function: &ResolvedFunction) -> bool {
    has_record_renewal_candidate(&function.body)
}
pub(crate) fn template_requires_renewal(template: &ResolvedFunctionTemplate) -> bool {
    generic_collection::profile(template)
        && !bindings(
            &template.body,
            Some((&template.id, template.type_parameters.len())),
        )
        .is_empty()
}
pub(crate) fn renewal_binding<'a>(
    program: &'a ResolvedProgram,
    function: &'a ResolvedFunction,
    at: &ExpressionId,
) -> Option<&'a ResolvedBinding> {
    bindings(&function.body, None)
        .remove(at)
        .or_else(|| record_bindings(program, &function.body).remove(at))
}

pub(crate) fn renewal_bindings<'a>(
    program: &'a ResolvedProgram,
    function: &'a ResolvedFunction,
) -> BTreeMap<ExpressionId, &'a ResolvedBinding> {
    let mut found = record_bindings(program, &function.body);
    // Preserve renewal_binding's historical default-profile precedence when
    // malformed HIR reuses one expression identity across both collectors.
    found.extend(bindings(&function.body, None));
    found
}

fn has_record_renewal_candidate(body: &ResolvedExpr) -> bool {
    let mut pending = vec![body];
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Block { statements, .. } = &expression.kind {
            for statement in statements {
                if let ResolvedStatement::While { body, .. } = statement {
                    let mut loop_nodes = vec![body.as_ref()];
                    while let Some(node) = loop_nodes.pop() {
                        if let ResolvedExprKind::Block { statements, .. } = &node.kind {
                            if statements.iter().any(|statement| matches!(statement,
                                ResolvedStatement::Assign { binding, field: None, value, .. }
                                    if binding.ownership == OwnershipMode::Own
                                        && value.ownership == OwnershipMode::Own
                                        && binding.ty == value.ty
                                        && matches!(&binding.ty,
                                            ResolvedType::Nominal { arguments, .. }
                                                if arguments.is_empty())
                                        && matches!(&value.kind, ResolvedExprKind::Call { args, .. }
                                            if args.iter().any(|argument| place(argument, &binding.id)))))
                            {
                                return true;
                            }
                        }
                        push_resolved_expression_children_in_authored_order(node, &mut loop_nodes);
                    }
                }
            }
        }
        push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    false
}

fn record_bindings<'a>(
    program: &'a ResolvedProgram,
    body: &'a ResolvedExpr,
) -> BTreeMap<ExpressionId, &'a ResolvedBinding> {
    let mut found = BTreeMap::new();
    let mut pending = vec![body];
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Block { statements, .. } = &expression.kind {
            for statement in statements {
                if let ResolvedStatement::While { body, .. } = statement {
                    collect_record_bindings(program, body, &mut found);
                }
            }
        }
        push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    found
}

fn collect_record_bindings<'a>(
    program: &'a ResolvedProgram,
    body: &'a ResolvedExpr,
    found: &mut BTreeMap<ExpressionId, &'a ResolvedBinding>,
) {
    let mut pending = vec![body];
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Block { statements, .. } = &expression.kind {
            for statement in statements {
                if let ResolvedStatement::Assign {
                    binding,
                    field: None,
                    value,
                    ..
                } = statement
                {
                    if is_record_owner_renewal(program, binding, value) {
                        found.insert(value.id.clone(), binding);
                    }
                }
            }
        }
        push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
}

pub(crate) fn is_owner_renewal_record(declarations: &DeclarationIndex, ty: &ResolvedType) -> bool {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return false;
    };
    if !arguments.is_empty()
        || declarations.declaration(declaration).is_none_or(|item| {
            item.kind != DeclarationKind::Record || item.identity_origin != IdentityOrigin::Explicit
        })
        || declarations
            .type_parameters(declaration)
            .is_none_or(|parameters| !parameters.is_empty())
    {
        return false;
    }
    matches!(declarations.record_fields(declaration), Some(fields)
        if fields.len() == 2
            && fields.iter().filter(|field| field.ty == ResolvedType::Bytes).count() == 1
            && fields.iter().filter(|field| field.ty == ResolvedType::Usize).count() == 1
            && fields.iter().all(|field| declarations.declaration(&field.id)
                .is_some_and(|item| item.identity_origin == IdentityOrigin::Explicit)))
}

pub(crate) fn is_record_owner_renewal(
    program: &ResolvedProgram,
    binding: &ResolvedBinding,
    value: &ResolvedExpr,
) -> bool {
    if binding.ownership != OwnershipMode::Own
        || value.ownership != OwnershipMode::Own
        || binding.ty != value.ty
        || !is_owner_renewal_record(&program.declarations, &binding.ty)
    {
        return false;
    }
    let ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance: None,
        args,
    } = &value.kind
    else {
        return false;
    };
    let Some(target) = program.resolve_call_target(callee, None) else {
        return false;
    };
    target.effects.is_empty()
        && type_arguments.is_empty()
        && target.return_type == binding.ty
        && target.params.len() == args.len()
        && target
            .params
            .iter()
            .zip(args)
            .filter(|(parameter, argument)| {
                parameter.ownership == OwnershipMode::Own
                    && parameter.ty == binding.ty
                    && place(argument, &binding.id)
            })
            .count()
            == 1
        && target.params.iter().zip(args).all(|(parameter, argument)| {
            parameter.ty == argument.ty
                && match parameter.ownership {
                    OwnershipMode::Own => {
                        argument.ownership == OwnershipMode::Own
                            && parameter.ty == binding.ty
                            && place(argument, &binding.id)
                    }
                    OwnershipMode::Borrow => {
                        matches!(
                            argument.ownership,
                            OwnershipMode::Own | OwnershipMode::Borrow
                        ) && matches!(&argument.kind, ResolvedExprKind::Place(place)
                                if place.projections.is_empty())
                            && (is_owner_renewal_record(&program.declarations, &parameter.ty)
                                || (argument.ownership == OwnershipMode::Borrow
                                    && crate::loop_calls::resolved_renewal_view(&parameter.ty)))
                    }
                    OwnershipMode::Value => argument.ownership == OwnershipMode::Value,
                    OwnershipMode::Shared => false,
                }
        })
}
fn bindings<'a>(
    body: &'a ResolvedExpr,
    owner: Option<(&DeclarationId, usize)>,
) -> BTreeMap<ExpressionId, &'a ResolvedBinding> {
    let mut found = BTreeMap::new();
    let mut pending = vec![body];
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Block { statements, .. } = &expression.kind {
            for statement in statements {
                if let ResolvedStatement::While {
                    condition, body, ..
                } = statement
                {
                    if let Some(protocol) = recognize_scoped(condition, body, owner) {
                        conditional_bindings(protocol.authored_body, owner, &mut found);
                    }
                }
            }
        }
        push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    found
}
fn conditional_bindings<'a>(
    body: &'a ResolvedExpr,
    owner: Option<(&DeclarationId, usize)>,
    found: &mut BTreeMap<ExpressionId, &'a ResolvedBinding>,
) {
    let mut pending = vec![(body, false)];
    while let Some((expression, conditional)) = pending.pop() {
        if let ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } = &expression.kind
        {
            pending.push((condition, conditional));
            pending.push((then_branch, true));
            pending.push((else_branch, true));
            continue;
        }
        if conditional {
            if let ResolvedExprKind::Block { statements, .. } = &expression.kind {
                for statement in statements {
                    if let ResolvedStatement::Assign {
                        binding,
                        field: None,
                        value,
                        ..
                    } = statement
                    {
                        if same_owner(binding, value, owner) {
                            found.insert(value.id.clone(), binding);
                        }
                    }
                }
            }
        }
        let mut children = Vec::new();
        push_resolved_expression_children_in_authored_order(expression, &mut children);
        pending.extend(children.into_iter().map(|child| (child, conditional)));
    }
}
fn same_owner(
    binding: &ResolvedBinding,
    value: &ResolvedExpr,
    owner: Option<(&DeclarationId, usize)>,
) -> bool {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = &binding.ty
    else {
        return false;
    };
    let [element] = arguments.as_slice() else {
        return false;
    };
    if declaration.as_str() != crate::prelude::VEC_ID
        || binding.ownership != OwnershipMode::Own
        || value.ty != binding.ty
        || value.ownership != OwnershipMode::Own
        || !(generic_collection::scalar(element)
            || owner
                .is_some_and(|(owner, count)| generic_collection::parameter(element, owner, count)))
    {
        return false;
    }
    matches!(&value.kind, ResolvedExprKind::Call { callee, type_arguments, instance: None, args }
        if crate::vec_ops::by_id(callee.as_str()).is_some_and(|op| op.reopens_same_owner() && args.len() == op.arity())
        && type_arguments.as_slice() == [element.clone()]
        && args.first().is_some_and(|argument| place(argument, &binding.id) && argument.ty == binding.ty && argument.ownership == OwnershipMode::Own))
}
