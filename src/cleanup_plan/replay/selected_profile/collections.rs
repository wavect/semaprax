//! Actual monomorphic Vec and field-read facts, independent of plan metadata.
use super::*;
use crate::vec_ops::VecOp;

pub(in crate::cleanup_plan::replay) fn call(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    Ok(call_facts(program, function, expression, budget)?.is_some())
}
pub(in crate::cleanup_plan::replay) fn call_facts(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    budget: &mut ReplayBudget,
) -> Result<Option<bool>, Diagnostic> {
    let ResolvedExprKind::Call {
        callee,
        instance,
        type_arguments,
        args,
    } = &expression.kind
    else {
        return Ok(None);
    };
    budget.charge(
        function,
        callee
            .as_str()
            .len()
            .min(64)
            .checked_add(1)
            .and_then(|n| n.checked_mul(32))
            .ok_or_else(|| replay_error(function, "selected Vec callee work overflow"))?,
        "selected Vec callee byte comparisons",
    )?;
    let Some(op) = crate::vec_ops::by_id(callee.as_str()) else {
        return Ok(None);
    };
    if instance.is_some()
        || matches!(
            op,
            VecOp::SortBy | VecOp::StrAt | VecOp::UniqueBy | VecOp::UpdateField
        )
    {
        return Err(replay_error(
            function,
            "selected Vec call requires a separately authenticated operation",
        ));
    }
    let [element] = type_arguments.as_slice() else {
        return Err(replay_error(
            function,
            "selected Vec call has invalid type arguments",
        ));
    };
    prepay(program, function, element_bytes(element)?, budget)?;
    let vector = if op == VecOp::WithCapacity {
        &expression.ty
    } else {
        &args
            .first()
            .ok_or_else(|| replay_error(function, "selected Vec source is absent"))?
            .ty
    };
    if crate::cleanup::selected_vector_element(program, vector)? != Some(element) {
        return Err(replay_error(
            function,
            "selected Vec element and root disagree",
        ));
    }
    let owned = *element == ResolvedType::Bytes
        || *element == ResolvedType::String
        || crate::hir::owned_leaf_collection::runtime_element(program.declarations(), element);
    let permitted = crate::vec_ops::resolved_operation_element_is_admitted(op, element)
        || crate::hir::owned_leaf_collection::vec_operation_admitted(
            program.declarations(),
            op,
            element,
        );
    if !permitted || args.len() != op.arity() {
        return Err(replay_error(
            function,
            "selected Vec operation or arity is outside the exact element family",
        ));
    }
    budget.charge(function, args.len() + 9, "selected Vec argument facts")?;
    for (i, arg) in args.iter().enumerate() {
        let expected = if owned {
            op.param_ownership_for(i, element)
        } else {
            op.param_ownership(i)
        };
        let valid_type = match (op, i) {
            (VecOp::WithCapacity, 0) => arg.ty == ResolvedType::Usize,
            (_, 0) => crate::cleanup::selected_vector_element(program, &arg.ty)? == Some(element),
            (VecOp::Push, 1) | (VecOp::Set | VecOp::Replace, 2) => &arg.ty == element,
            _ => arg.ty == ResolvedType::Usize,
        };
        if !valid_type
            || (expected != arg.ownership
                && !(expected == OwnershipMode::Borrow
                    && matches!(arg.ownership, OwnershipMode::Own | OwnershipMode::Borrow)))
        {
            return Err(replay_error(
                function,
                "selected Vec argument type or mode disagrees",
            ));
        }
    }
    let (valid_type, mode) = match op {
        VecOp::Len | VecOp::Capacity => {
            (expression.ty == ResolvedType::Usize, OwnershipMode::Value)
        }
        VecOp::Get | VecOp::CloneAt => (
            &expression.ty == element,
            if owned {
                OwnershipMode::Own
            } else {
                OwnershipMode::Value
            },
        ),
        _ => (
            crate::cleanup::selected_vector_element(program, &expression.ty)? == Some(element),
            OwnershipMode::Own,
        ),
    };
    if !valid_type || expression.ownership != mode {
        return Err(replay_error(
            function,
            "selected Vec result type or mode disagrees",
        ));
    }
    Ok(Some(owned))
}

pub(in crate::cleanup_plan::replay) fn field(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let ResolvedExprKind::VecFieldRead {
        origin: _,
        element,
        field,
        bytes,
        args,
    } = &expression.kind
    else {
        return Ok(());
    };
    let [owner, index] = args.as_slice() else {
        return Err(replay_error(
            function,
            "selected Vec field read has invalid arity",
        ));
    };
    prepay(
        program,
        function,
        element_bytes(element)?
            .checked_add(field.as_str().len())
            .ok_or_else(|| replay_error(function, "selected field key overflow"))?,
        budget,
    )?;
    let Some((_, fields)) = crate::cleanup::selected_flat_record_fields(program, element)? else {
        return Err(replay_error(
            function,
            "selected Vec field element is not current flat record",
        ));
    };
    budget.charge(function, fields.len() + 9, "selected Vec field facts")?;
    let declared = fields
        .iter()
        .find(|f| &f.id == field)
        .ok_or_else(|| replay_error(function, "selected Vec field identity is foreign"))?;
    let (ty, mode) = match &declared.ty {
        ResolvedType::String => (
            if *bytes {
                &ResolvedType::SliceU8
            } else {
                &ResolvedType::Str
            },
            OwnershipMode::Borrow,
        ),
        ResolvedType::Bytes if !bytes => (&ResolvedType::SliceU8, OwnershipMode::Borrow),
        ty if crate::hir::is_scalar_resolved_type(ty) && !*bytes => (ty, OwnershipMode::Value),
        _ => {
            return Err(replay_error(
                function,
                "selected Vec field mode is unsupported",
            ))
        }
    };
    if crate::cleanup::selected_vector_element(program, &owner.ty)? != Some(element)
        || !matches!(&owner.kind,ResolvedExprKind::Place(p) if p.projections.is_empty())
        || !matches!(owner.ownership, OwnershipMode::Own | OwnershipMode::Borrow)
        || index.ty != ResolvedType::Usize
        || index.ownership != OwnershipMode::Value
        || &expression.ty != ty
        || expression.ownership != mode
    {
        return Err(replay_error(
            function,
            "selected Vec field root, index, result type or mode disagrees",
        ));
    }
    Ok(())
}

pub(in crate::cleanup_plan::replay) fn append(
    function: &ResolvedFunction,
    binding: &crate::hir::ResolvedBinding,
    value: &ResolvedExpr,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    if budget.refusal_spent {
        return Err(budget.budget_refusal(function, "selected replay budget already failed", ""));
    }
    // Debit the allowance `ReplayBudget::charge` would use. A factored
    // comparison lends the whole free allowance to its skeleton constructors,
    // so `remaining` is then zero and only the lent allowance may pay.
    let ReplayBudget {
        remaining,
        skeleton_remaining,
        skeleton_spent,
        lent_skeleton,
        physical,
        ..
    } = budget;
    let lent = *lent_skeleton;
    let physical = physical
        .as_deref_mut()
        .ok_or_else(|| replay_error(function, "selected append lost reservation"))?;
    let mut exhausted = false;
    let answer = crate::cleanup_plan::renewal_profile::selected_string_append(
        binding,
        value,
        physical,
        &mut |units| {
            let debited = if lent {
                skeleton_remaining
                    .checked_sub(units)
                    .zip(skeleton_spent.checked_add(units))
                    .map(|(next, spent)| {
                        *skeleton_remaining = next;
                        *skeleton_spent = spent;
                    })
            } else {
                remaining.checked_sub(units).map(|next| *remaining = next)
            };
            debited.ok_or_else(|| {
                exhausted = true;
                // This marker is consumed below, never published. The outer
                // refusal owns the sole prepaid diagnostic String backing.
                Diagnostic::io("SPX-H006", String::new())
            })
        },
    );
    if exhausted {
        // Same sticky refusal as every other selected debit: no later member
        // or retry can spend what this exhausted allowance did not have.
        let error = budget.budget_refusal(
            function,
            "cleanup replay work budget exhausted during ",
            "selected String append",
        );
        exhaust(budget);
        return Err(error);
    }
    answer
}

pub(in crate::cleanup_plan::replay) fn prepay(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    query_bytes: usize,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let answer =
        crate::cleanup::prepay_selected_collection_probes(program, query_bytes, &mut |units| {
            budget.charge(function, units, "selected collection key/probe work")
        });
    if answer.is_err() {
        // A refused prepayment is sticky; a one-short receiver cannot retry
        // the census with the unspent remainder of its exact allowance.
        exhaust(budget);
    }
    answer
}

/// Zero the active work allowance after a selected refusal.
fn exhaust(budget: &mut ReplayBudget) {
    if budget.lent_skeleton {
        budget.skeleton_remaining = 0;
    } else {
        budget.remaining = 0;
    }
}
fn element_bytes(element: &ResolvedType) -> Result<usize, Diagnostic> {
    match element {
        ResolvedType::Nominal {
            declaration,
            arguments,
        } if arguments.is_empty() => Ok(declaration.as_str().len()),
        ResolvedType::Nominal { .. }
        | ResolvedType::Function { .. }
        | ResolvedType::TypeParameter { .. } => Err(Diagnostic::io(
            "SPX-H006",
            "selected collection element is not a bounded leaf",
        )),
        _ => Ok(1),
    }
}

pub(in crate::cleanup_plan::replay) fn type_key_bytes(
    ty: &ResolvedType,
) -> Result<usize, Diagnostic> {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(1);
    };
    let nested = match arguments.as_slice() {
        [] => 0,
        [element] => element_bytes(element)?,
        _ => {
            return Err(Diagnostic::io(
                "SPX-H006",
                "selected type has unsupported argument count",
            ))
        }
    };
    declaration
        .as_str()
        .len()
        .checked_add(nested)
        .ok_or_else(|| Diagnostic::io("SPX-H006", "selected type key length overflow"))
}
