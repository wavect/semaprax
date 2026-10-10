//! Exact same-cell admitted Vec renewal in ordinary while bodies (not iterator lowering).
use super::*;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn requires(function: &ResolvedFunction) -> bool {
    !bindings(function).is_empty()
}
pub(crate) fn bindings(function: &ResolvedFunction) -> BTreeMap<ExpressionId, &ResolvedBinding> {
    collect(function, None)
}
/// Existing Copy and owned-element collection admission composes the same-cell
/// protocol. Element shape and call ownership are re-derived from declarations.
pub(crate) fn bindings_in<'a>(
    program: &ResolvedProgram,
    function: &'a ResolvedFunction,
) -> BTreeMap<ExpressionId, &'a ResolvedBinding> {
    collect(
        function,
        Some(&|op, element| element_mode(&program.declarations, op, element)),
    )
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
type ElementAdmission<'a> =
    dyn Fn(crate::vec_ops::VecOp, &ResolvedType) -> Option<OwnershipMode> + 'a;

/// Borrowed source-authenticated declaration authority for graph selection.
/// This only selects metadata; cleanup construction and replay authenticate
/// their own resolved declaration index independently.
pub(crate) fn requires_with_admission(
    function: &ResolvedFunction,
    admitted: impl Fn(crate::vec_ops::VecOp, &ResolvedType) -> Option<OwnershipMode>,
) -> bool {
    !collect(function, Some(&admitted)).is_empty()
}

fn element_mode(
    index: &DeclarationIndex,
    op: crate::vec_ops::VecOp,
    element: &ResolvedType,
) -> Option<OwnershipMode> {
    if !super::owned_leaf_collection::vec_operation_admitted(index, op, element) {
        return None;
    }
    Some(
        if *element == ResolvedType::Bytes
            || super::owned_leaf_collection::runtime_element(index, element)
        {
            OwnershipMode::Own
        } else {
            OwnershipMode::Value
        },
    )
}

fn collect<'a>(
    function: &'a ResolvedFunction,
    admission: Option<&ElementAdmission>,
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
                        && same_cell(binding, value, admission) =>
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
    admission: Option<&ElementAdmission>,
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
    // Admission remains operation-specific; ownership comes from the exact
    // independently authenticated element family, never a cached Copy bit.
    let mode = if super::generic_collection::scalar(element) {
        Some(OwnershipMode::Value)
    } else {
        admission.and_then(|admit| admit(op, element))
    };
    matches!(mode, Some(OwnershipMode::Value | OwnershipMode::Own))
        && type_arguments.as_slice() == std::slice::from_ref(element)
        && args.len() == op.arity()
        && args.first().is_some_and(|arg| {
            arg.ty == binding.ty
                && arg.ownership == OwnershipMode::Own
                && matches!(&arg.kind, ResolvedExprKind::Place(place)
                    if place.root == binding.id && place.projections.is_empty())
        })
        && args.iter().enumerate().all(|(index, arg)| {
            op.accepts_resolved(index, &arg.ty, element)
                && arg.ownership
                    == if mode == Some(OwnershipMode::Own) {
                        op.param_ownership_for(index, element)
                    } else {
                        op.param_ownership(index)
                    }
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

#[cfg(test)]
#[path = "vec_loop_renewal/tests.rs"]
mod tests;
