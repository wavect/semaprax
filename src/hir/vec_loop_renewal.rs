//! Exact same-cell scalar Vec renewal in ordinary while bodies (not iterator lowering).
use super::*;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn requires(function: &ResolvedFunction) -> bool {
    !bindings(function).is_empty()
}
pub(crate) fn bindings(function: &ResolvedFunction) -> BTreeMap<ExpressionId, &ResolvedBinding> {
    collect(function, None)
}
/// Copy-record collection admission composes the existing same-cell protocol;
/// it never authorizes an owned element or trusts a cached Copy flag.
pub(crate) fn bindings_in<'a>(
    program: &ResolvedProgram,
    function: &'a ResolvedFunction,
) -> BTreeMap<ExpressionId, &'a ResolvedBinding> {
    collect(function, Some(&program.declarations))
}
pub(crate) fn requires_in(program: &ResolvedProgram, function: &ResolvedFunction) -> bool {
    !bindings_in(program, function).is_empty()
}
pub(crate) fn binding_in<'a>(
    program: &ResolvedProgram,
    function: &'a ResolvedFunction,
    at: &ExpressionId,
) -> Option<&'a ResolvedBinding> {
    bindings_in(program, function).remove(at)
}
fn collect<'a>(
    function: &'a ResolvedFunction,
    index: Option<&DeclarationIndex>,
) -> BTreeMap<ExpressionId, &'a ResolvedBinding> {
    let mut mutable = BTreeSet::new();
    let mut pending = vec![&function.body];
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Block { statements, .. } = &expression.kind {
            for statement in statements {
                if let ResolvedStatement::Let {
                    binding,
                    mutable: true,
                    ..
                } = statement
                {
                    mutable.insert(binding.id.clone());
                }
            }
        }
        push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    let mut found = BTreeMap::new();
    let mut pending = vec![(&function.body, false)];
    while let Some((expression, in_loop)) = pending.pop() {
        if matches!(expression.kind, ResolvedExprKind::Closure { .. }) {
            continue;
        }
        if let ResolvedExprKind::Block { statements, tail } = &expression.kind {
            pending.push((tail, in_loop));
            for statement in statements {
                match statement {
                    ResolvedStatement::Assign {
                        binding,
                        field: None,
                        value,
                        ..
                    } if in_loop
                        && mutable.contains(&binding.id)
                        && same_cell(binding, value, index) =>
                    {
                        found.insert(value.id.clone(), binding);
                        pending.push((value, in_loop));
                    }
                    ResolvedStatement::While {
                        condition, body, ..
                    } => {
                        // Source for-own lowers to a while with an authenticated protocol.
                        // Its v12 conditional renewal remains a separate frozen profile.
                        let ordinary = super::iterator_loop::recognize(condition, body).is_none();
                        pending.push((body, ordinary));
                        pending.push((condition, false));
                    }
                    ResolvedStatement::Let { value, .. }
                    | ResolvedStatement::Assign { value, .. } => pending.push((value, in_loop)),
                    _ => {}
                }
            }
        } else {
            let mut children = Vec::new();
            push_resolved_expression_children_in_authored_order(expression, &mut children);
            pending.extend(children.into_iter().map(|child| (child, in_loop)));
        }
    }
    found
}
fn same_cell(
    binding: &ResolvedBinding,
    value: &ResolvedExpr,
    index: Option<&DeclarationIndex>,
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
        || !(super::generic_collection::scalar(element)
            || index.is_some_and(|index| super::copy_record_collection::admitted(index, element)))
        || binding.ownership != OwnershipMode::Own
        || value.ownership != OwnershipMode::Own
        || value.ty != binding.ty
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
    let Some(op) = crate::vec_ops::by_id(callee.as_str()).filter(|op| op.reopens_same_owner())
    else {
        return false;
    };
    type_arguments.as_slice() == std::slice::from_ref(element)
        && args.len() == op.arity()
        && args.first().is_some_and(|arg| {
            arg.ty == binding.ty
                && arg.ownership == OwnershipMode::Own
                && matches!(&arg.kind, ResolvedExprKind::Place(place)
                    if place.root == binding.id && place.projections.is_empty())
        })
        && args.iter().enumerate().all(|(index, arg)| {
            op.accepts_resolved(index, &arg.ty, element)
                && (index == 0 || arg.ownership == OwnershipMode::Value)
        })
}

/// Forge cached field drift for independent renewal-replay hostile controls.
#[cfg(test)]
pub(crate) fn forge_record_field_for_test(
    declarations: &mut DeclarationIndex,
    record: &DeclarationId,
    ty: ResolvedType,
) {
    declarations
        .record_fields
        .get_mut(record)
        .expect("fixture record")[0]
        .ty = ty;
}
