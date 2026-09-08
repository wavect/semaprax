//! Exact conditional same-owner Vec renewal inside an authenticated traversal.
use super::*;
use std::collections::BTreeMap;

pub(crate) fn function_requires_renewal(function: &ResolvedFunction) -> bool {
    !bindings(&function.body, None).is_empty()
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
    function: &'a ResolvedFunction,
    at: &ExpressionId,
) -> Option<&'a ResolvedBinding> {
    bindings(&function.body, None).remove(at)
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
