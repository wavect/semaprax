//! Structural authentication of the consuming iterator loop's retained lowering.
use super::*;
mod renewal;
pub(crate) use renewal::{function_requires_renewal, renewal_binding, template_requires_renewal};

pub(crate) struct IteratorLoop<'a> {
    pub(crate) step: &'a ResolvedBinding,
    pub(crate) authored_body: &'a ResolvedExpr,
    pub(crate) owned_item: Option<&'a ResolvedBinding>,
}
fn zero(value: &ResolvedExpr) -> bool {
    value.ownership == OwnershipMode::Value
        && matches!(
            value.kind,
            ResolvedExprKind::Int(0) | ResolvedExprKind::Usize(0)
        )
}
fn place(value: &ResolvedExpr, id: &ValueId) -> bool {
    matches!(&value.kind, ResolvedExprKind::Place(place) if &place.root == id && place.projections.is_empty())
}
fn cases(arms: &[ResolvedMatchArm]) -> bool {
    matches!(arms, [done, yielded]
        if done.guard.is_none() && yielded.guard.is_none()
        && matches!(&done.pattern, ResolvedMatchPattern::Variant { variant, case, fields }
            if variant.as_str() == crate::iterator_ops::STEP_ID && case.as_str() == crate::iterator_ops::DONE_ID && fields.is_empty())
        && matches!(&yielded.pattern, ResolvedMatchPattern::Variant { variant, case, fields }
            if variant.as_str() == crate::iterator_ops::STEP_ID && case.as_str() == crate::iterator_ops::YIELD_ID && fields.len() == 2
            && fields[0].field.as_str() == crate::iterator_ops::ITEM_ID && fields[1].field.as_str() == crate::iterator_ops::REST_ID))
}
fn replacement<'a>(
    value: &'a ResolvedExpr,
    binding: &ValueId,
    owner: Option<(&DeclarationId, usize)>,
) -> Option<&'a ResolvedExpr> {
    let ResolvedExprKind::Match {
        mode: ResolvedMatchMode::Own,
        scrutinee,
        arms,
    } = &value.kind
    else {
        return None;
    };
    if step_element(&value.ty, owner).is_none()
        || value.ownership != OwnershipMode::Own
        || scrutinee.ty != value.ty
        || !place(scrutinee, binding)
        || !cases(arms)
    {
        return None;
    }
    if !matches!(&arms[0].value.kind, ResolvedExprKind::ConstructVariant { variant, case, fields }
        if variant.as_str() == crate::iterator_ops::STEP_ID && case.as_str() == crate::iterator_ops::DONE_ID && fields.is_empty())
        || arms[0].value.ty != value.ty
    {
        return None;
    }
    let ResolvedMatchPattern::Variant { fields, .. } = &arms[1].pattern else {
        return None;
    };
    let element = step_element(&value.ty, owner)?;
    if fields[0].binding.ty != *element
        || fields[0].binding.ownership != crate::iterator_ops::item_ownership(element, false)
        || fields[1].binding.ty != crate::iterator_ops::resolved_iter(element.clone())
        || fields[1].binding.ownership != OwnershipMode::Own
    {
        return None;
    }
    let ResolvedExprKind::Block { statements, tail } = &arms[1].value.kind else {
        return None;
    };
    let [ResolvedStatement::Let {
        binding: discard,
        mutable: false,
        value: authored_body,
        ..
    }] = statements.as_slice()
    else {
        return None;
    };
    if discard.ownership != OwnershipMode::Value
        || !(is_scalar_resolved_type(&discard.ty)
            || owner.is_some_and(|(owner, count)| {
                generic_collection::parameter(&discard.ty, owner, count)
            }))
        || discard.ty != authored_body.ty
        || authored_body.ownership != OwnershipMode::Value
    {
        return None;
    }
    if !matches!(&tail.kind, ResolvedExprKind::Call { callee, type_arguments, instance: None, args }
        if callee.as_str() == crate::iterator_ops::NEXT_ID && type_arguments.as_slice() == [element.clone()]
        && matches!(args.as_slice(), [argument] if place(argument, &fields[1].binding.id) && argument.ty == fields[1].binding.ty && argument.ownership == OwnershipMode::Own))
        || tail.ty != value.ty
        || tail.ownership != OwnershipMode::Own
    {
        return None;
    }
    Some(authored_body)
}
pub(crate) fn is_step_reassignment(value: &ResolvedExpr, binding: &ValueId) -> bool {
    replacement(value, binding, None).is_some()
}
pub(crate) fn recognize<'a>(
    condition: &'a ResolvedExpr,
    body: &'a ResolvedExpr,
) -> Option<IteratorLoop<'a>> {
    recognize_scoped(condition, body, None)
}
fn recognize_scoped<'a>(
    condition: &'a ResolvedExpr,
    body: &'a ResolvedExpr,
    owner: Option<(&DeclarationId, usize)>,
) -> Option<IteratorLoop<'a>> {
    let ResolvedExprKind::Block { statements, tail } = &body.kind else {
        return None;
    };
    let [ResolvedStatement::Assign {
        binding: step,
        field: None,
        value,
        ..
    }] = statements.as_slice()
    else {
        return None;
    };
    if !zero(tail) || step.ownership != OwnershipMode::Own || step.ty != value.ty {
        return None;
    }
    let authored_body = replacement(value, &step.id, owner)?;
    let ResolvedExprKind::Match {
        mode: ResolvedMatchMode::Borrow,
        scrutinee,
        arms,
    } = &condition.kind
    else {
        return None;
    };
    if !place(scrutinee, &step.id)
        || scrutinee.ty != step.ty
        || !cases(arms)
        || condition.ty != ResolvedType::Bool
        || condition.ownership != OwnershipMode::Value
        || !matches!(arms[0].value.kind, ResolvedExprKind::Bool(false))
        || !matches!(arms[1].value.kind, ResolvedExprKind::Bool(true))
    {
        return None;
    }
    let element = step_element(&step.ty, owner)?;
    let ResolvedMatchPattern::Variant { fields, .. } = &arms[1].pattern else {
        return None;
    };
    if fields[0].binding.ty != *element
        || fields[0].binding.ownership != crate::iterator_ops::item_ownership(element, true)
        || fields[1].binding.ty != crate::iterator_ops::resolved_iter(element.clone())
        || fields[1].binding.ownership != OwnershipMode::Borrow
    {
        return None;
    }
    let owned_item = if *element == ResolvedType::Bytes {
        let ResolvedExprKind::Match { arms, .. } = &value.kind else {
            return None;
        };
        let ResolvedMatchPattern::Variant { fields, .. } = &arms[1].pattern else {
            return None;
        };
        Some(&fields[0].binding)
    } else {
        None
    };
    Some(IteratorLoop {
        step,
        authored_body,
        owned_item,
    })
}
pub(crate) fn function_contains(function: &ResolvedFunction) -> bool {
    let mut pending = vec![&function.body];
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Block { statements, .. } = &expression.kind {
            if statements.iter().any(|statement| matches!(statement, ResolvedStatement::While { condition, body, .. } if recognize(condition, body).is_some())) { return true; }
        }
        push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    false
}

/// Authenticate the owning seed and mutable slot independently of display names.
pub(crate) fn validate_function(
    function: &ResolvedFunction,
) -> Result<(), crate::diagnostic::Diagnostic> {
    let mut pending = vec![&function.body];
    let mut authorized = BTreeSet::new();
    let mut replacements = Vec::new();
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Block { statements, tail } = &expression.kind {
            for statement in statements {
                if let ResolvedStatement::Assign { binding, value, .. } = statement {
                    if crate::iterator_ops::is_step(&binding.ty) {
                        replacements.push(value.id.clone());
                    }
                }
                if let ResolvedStatement::While {
                    condition, body, ..
                } = statement
                {
                    if let Some(protocol) = recognize(condition, body) {
                        let [ResolvedStatement::Let {
                            binding,
                            mutable: true,
                            value: seed,
                            ..
                        }, ResolvedStatement::While { .. }] = statements.as_slice()
                        else {
                            return Err(crate::diagnostic::Diagnostic::io(
                                "SPX-H006",
                                "iterator loop must retain one mutable initialized Step slot",
                            ));
                        };
                        if binding != protocol.step
                            || !zero(tail)
                            || !matches!(&seed.kind, ResolvedExprKind::Call { callee, type_arguments, instance: None, args }
                                if callee.as_str() == crate::iterator_ops::NEXT_ID
                                && matches!(args.as_slice(), [source] if source.ownership == OwnershipMode::Own && source.ty == crate::iterator_ops::resolved_iter(type_arguments.first().cloned().unwrap_or(ResolvedType::Unit))))
                            || seed.ty != binding.ty
                            || seed.ownership != OwnershipMode::Own
                        {
                            return Err(crate::diagnostic::Diagnostic::io(
                                "SPX-H006",
                                "iterator loop seed, slot, or wrapper is inconsistent",
                            ));
                        }
                        let ResolvedExprKind::Block { statements, .. } = &body.kind else {
                            unreachable!()
                        };
                        let ResolvedStatement::Assign { value, .. } = &statements[0] else {
                            unreachable!()
                        };
                        authorized.insert(value.id.clone());
                    }
                }
            }
        }
        push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    if replacements.iter().any(|id| !authorized.contains(id)) {
        return Err(crate::diagnostic::Diagnostic::io(
            "SPX-H006",
            "owning Step assignment is outside an authenticated consuming iterator loop",
        ));
    }
    Ok(())
}

fn step_element<'a>(
    ty: &'a ResolvedType,
    owner: Option<(&DeclarationId, usize)>,
) -> Option<&'a ResolvedType> {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return None;
    };
    let [element] = arguments.as_slice() else {
        return None;
    };
    (declaration.as_str() == crate::iterator_ops::STEP_ID
        && (crate::iterator_ops::resolved_element_is_admitted(element)
            || owner.is_some_and(|(owner, count)| {
                generic_collection::parameter(element, owner, count)
            })))
    .then_some(element)
}
/// Symbolic templates have no executable cleanup plan. Their exact scoped
/// protocol still belongs to the additive graph grammar before instantiation.
pub(crate) fn template_contains(template: &ResolvedFunctionTemplate) -> bool {
    if !(1..=2).contains(&template.type_parameters.len()) || !generic_collection::profile(template)
    {
        return false;
    }
    let mut pending = vec![&template.body];
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Block { statements, tail } = &expression.kind {
            if let [ResolvedStatement::Let {
                binding,
                mutable: true,
                value: seed,
                ..
            }, ResolvedStatement::While {
                condition, body, ..
            }] = statements.as_slice()
            {
                if let Some(protocol) = recognize_scoped(
                    condition,
                    body,
                    Some((&template.id, template.type_parameters.len())),
                ) {
                    if protocol.step == binding
                        && zero(tail)
                        && seed.ty == binding.ty
                        && seed.ownership == OwnershipMode::Own
                        && matches!(&seed.kind, ResolvedExprKind::Call { callee, type_arguments, instance: None, args }
                            if callee.as_str() == crate::iterator_ops::NEXT_ID
                            && matches!(type_arguments.as_slice(), [element] if generic_collection::parameter(element, &template.id, template.type_parameters.len())
                                && matches!(args.as_slice(), [source] if source.ty == crate::iterator_ops::resolved_iter(element.clone()) && source.ownership == OwnershipMode::Own)))
                    {
                        return true;
                    }
                }
            }
        }
        push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    false
}
