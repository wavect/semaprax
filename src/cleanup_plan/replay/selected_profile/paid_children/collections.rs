//! Exact selected collection twins with upfront finite-error ownership.
use super::{Deferred, Error, Permit};
use crate::cleanup::ReplayContext;
use crate::cleanup_plan::renewal_profile::{
    child_refusal::Refusal, profile_refusal::ProfileRefusal,
};

use crate::cleanup_plan::replay::ReplayBudget;
use crate::hir::{OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedFunction, ResolvedType};
use crate::vec_ops::VecOp;
pub(super) fn call_inner(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    budget: &mut ReplayBudget,
) -> Result<bool, Deferred> {
    Ok(call_facts_inner(program, function, expression, budget)?.is_some())
}
pub(in crate::cleanup_plan::replay) fn call(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    budget: &mut ReplayBudget,
) -> Result<bool, Error> {
    // Explicit legacy/non-owner entry; this result is never certified prepaid.
    if !budget.selected() || budget.physical.is_none() {
        return super::super::collections::call(program, function, expression, budget)
            .map_err(Error::Owned);
    }

    let permit = Permit::profile::<bool>(function, budget)?;
    let outcome = call_inner(program, function, expression, budget);
    permit.resolve(outcome)
}
pub(super) fn call_facts_inner(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    budget: &mut ReplayBudget,
) -> Result<Option<bool>, Deferred> {
    let ResolvedExprKind::Call {
        callee,
        instance,
        type_arguments,
        args,
    } = &expression.kind
    else {
        return Ok(None);
    };
    budget.child_charge(
        function,
        callee
            .as_str()
            .len()
            .min(64)
            .checked_add(1)
            .and_then(|n| n.checked_mul(32))
            .ok_or_else(|| {
                Deferred::Finite(Refusal::Profile(
                    ProfileRefusal::SelectedVecCalleeWorkOverflow,
                ))
            })?,
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
        return Err(Deferred::Finite(Refusal::Profile(
            ProfileRefusal::SelectedVecCallRequiresASeparatelyAuthenticatedOperation,
        )));
    }
    let [element] = type_arguments.as_slice() else {
        return Err(Deferred::Finite(Refusal::Profile(
            ProfileRefusal::SelectedVecCallHasInvalidTypeArguments,
        )));
    };
    prepay_inner(program, function, element_bytes_inner(element)?, budget)?;
    let vector = if op == VecOp::WithCapacity {
        &expression.ty
    } else {
        &args
            .first()
            .ok_or_else(|| {
                Deferred::Finite(Refusal::Profile(ProfileRefusal::SelectedVecSourceIsAbsent))
            })?
            .ty
    };
    if crate::cleanup::selected_inline::selected_vector_element_inline(program, vector)?
        != Some(element)
    {
        return Err(Deferred::Finite(Refusal::Profile(
            ProfileRefusal::SelectedVecElementAndRootDisagree,
        )));
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
        return Err(Deferred::Finite(Refusal::Profile(
            ProfileRefusal::SelectedVecOperationOrArityIsOutsideTheExactElementFamily,
        )));
    }
    budget.child_charge(function, args.len() + 9, "selected Vec argument facts")?;
    for (i, arg) in args.iter().enumerate() {
        let expected = if owned {
            op.param_ownership_for(i, element)
        } else {
            op.param_ownership(i)
        };
        let valid_type = match (op, i) {
            (VecOp::WithCapacity, 0) => arg.ty == ResolvedType::Usize,
            (_, 0) => {
                crate::cleanup::selected_inline::selected_vector_element_inline(program, &arg.ty)?
                    == Some(element)
            }
            (VecOp::Push, 1) | (VecOp::Set | VecOp::Replace, 2) => &arg.ty == element,
            _ => arg.ty == ResolvedType::Usize,
        };
        if !valid_type
            || (expected != arg.ownership
                && !(expected == OwnershipMode::Borrow
                    && matches!(arg.ownership, OwnershipMode::Own | OwnershipMode::Borrow)))
        {
            return Err(Deferred::Finite(Refusal::Profile(
                ProfileRefusal::SelectedVecArgumentTypeOrModeDisagrees,
            )));
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
            crate::cleanup::selected_inline::selected_vector_element_inline(
                program,
                &expression.ty,
            )? == Some(element),
            OwnershipMode::Own,
        ),
    };
    if !valid_type || expression.ownership != mode {
        return Err(Deferred::Finite(Refusal::Profile(
            ProfileRefusal::SelectedVecResultTypeOrModeDisagrees,
        )));
    }
    Ok(Some(owned))
}
pub(in crate::cleanup_plan::replay) fn call_facts(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    budget: &mut ReplayBudget,
) -> Result<Option<bool>, Error> {
    // Explicit legacy/non-owner entry; this result is never certified prepaid.
    if !budget.selected() || budget.physical.is_none() {
        return super::super::collections::call_facts(program, function, expression, budget)
            .map_err(Error::Owned);
    }

    let permit = Permit::profile::<Option<bool>>(function, budget)?;
    let outcome = call_facts_inner(program, function, expression, budget);
    permit.resolve(outcome)
}
pub(super) fn field_inner(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    budget: &mut ReplayBudget,
) -> Result<(), Deferred> {
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
        return Err(Deferred::Finite(Refusal::Profile(
            ProfileRefusal::SelectedVecFieldReadHasInvalidArity,
        )));
    };
    prepay_inner(
        program,
        function,
        element_bytes_inner(element)?
            .checked_add(field.as_str().len())
            .ok_or_else(|| {
                Deferred::Finite(Refusal::Profile(ProfileRefusal::SelectedFieldKeyOverflow))
            })?,
        budget,
    )?;
    let Some((_, fields)) =
        crate::cleanup::selected_inline::selected_flat_record_fields_inline(program, element)?
    else {
        return Err(Deferred::Finite(Refusal::Profile(
            ProfileRefusal::SelectedVecFieldElementIsNotCurrentFlatRecord,
        )));
    };
    budget.child_charge(function, fields.len() + 9, "selected Vec field facts")?;
    let declared = fields.iter().find(|f| &f.id == field).ok_or_else(|| {
        Deferred::Finite(Refusal::Profile(
            ProfileRefusal::SelectedVecFieldIdentityIsForeign,
        ))
    })?;
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
            return Err(Deferred::Finite(Refusal::Profile(
                ProfileRefusal::SelectedVecFieldModeIsUnsupported,
            )));
        }
    };
    if crate::cleanup::selected_inline::selected_vector_element_inline(program, &owner.ty)?
        != Some(element)
        || !matches!(&owner.kind,ResolvedExprKind::Place(p) if p.projections.is_empty())
        || !matches!(owner.ownership, OwnershipMode::Own | OwnershipMode::Borrow)
        || index.ty != ResolvedType::Usize
        || index.ownership != OwnershipMode::Value
        || &expression.ty != ty
        || expression.ownership != mode
    {
        return Err(Deferred::Finite(Refusal::Profile(
            ProfileRefusal::SelectedVecFieldRootIndexResultTypeOrModeDisagrees,
        )));
    }
    Ok(())
}
pub(in crate::cleanup_plan::replay) fn field(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    // Explicit legacy/non-owner entry; this result is never certified prepaid.
    if !budget.selected() || budget.physical.is_none() {
        return super::super::collections::field(program, function, expression, budget)
            .map_err(Error::Owned);
    }

    let permit = Permit::profile::<()>(function, budget)?;
    let outcome = field_inner(program, function, expression, budget);
    permit.resolve(outcome)
}
pub(super) fn append_inner(
    function: &ResolvedFunction,
    binding: &crate::hir::ResolvedBinding,
    value: &ResolvedExpr,
    budget: &mut ReplayBudget,
) -> Result<bool, Deferred> {
    if budget.refusal_spent {
        return Err(budget.child_budget_refusal(
            function,
            "selected replay budget already failed",
            "",
        ));
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
    let physical = physical.as_deref_mut().ok_or_else(|| {
        Deferred::Finite(Refusal::Profile(
            ProfileRefusal::SelectedAppendLostReservation,
        ))
    })?;
    let mut exhausted = false;
    let answer = crate::cleanup_plan::renewal_profile::selected_string_append_inline(
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
                Refusal::RenewalWork
            })
        },
    );
    if exhausted {
        // Same sticky refusal as every other selected debit: no later member
        // or retry can spend what this exhausted allowance did not have.
        let error = budget.child_budget_refusal(
            function,
            "cleanup replay work budget exhausted during ",
            "selected String append",
        );
        exhaust_inner(budget);
        return Err(error);
    }
    answer.map_err(Deferred::Finite)
}
pub(in crate::cleanup_plan::replay) fn append(
    function: &ResolvedFunction,
    binding: &crate::hir::ResolvedBinding,
    value: &ResolvedExpr,
    budget: &mut ReplayBudget,
) -> Result<bool, Error> {
    if budget.refusal_spent {
        return Err(budget
            .budget_refusal(function, "selected replay budget already failed", "")
            .into());
    }
    if !budget.selected() || budget.physical.is_none() {
        return append_unfunded_lost_owner(function, binding, value, budget);
    }
    let permit = Permit::profile::<bool>(function, budget)?;
    let outcome = append_inner(function, binding, value, budget);
    permit.resolve(outcome)
}
pub(super) fn prepay_inner(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    query_bytes: usize,
    budget: &mut ReplayBudget,
) -> Result<(), Deferred> {
    let answer = crate::cleanup::selected_inline::prepay_selected_collection_probes_inline(
        program,
        query_bytes,
        &mut |units| budget.child_charge(function, units, "selected collection key/probe work"),
    );
    if answer.is_err() {
        // A refused prepayment is sticky; a one-short receiver cannot retry
        // the census with the unspent remainder of its exact allowance.
        exhaust_inner(budget);
    }
    answer
}
pub(in crate::cleanup_plan::replay) fn prepay(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    query_bytes: usize,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    // Explicit legacy/non-owner entry; this result is never certified prepaid.
    if !budget.selected() || budget.physical.is_none() {
        return super::super::collections::prepay(program, function, query_bytes, budget)
            .map_err(Error::Owned);
    }

    let permit = Permit::profile::<()>(function, budget)?;
    let outcome = prepay_inner(program, function, query_bytes, budget);
    permit.resolve(outcome)
}
fn exhaust_inner(budget: &mut ReplayBudget) {
    if budget.lent_skeleton {
        budget.skeleton_remaining = 0;
    } else {
        budget.remaining = 0;
    }
}
fn element_bytes_inner(element: &ResolvedType) -> Result<usize, Deferred> {
    match element {
        ResolvedType::Nominal {
            declaration,
            arguments,
        } if arguments.is_empty() => Ok(declaration.as_str().len()),
        ResolvedType::Nominal { .. }
        | ResolvedType::Function { .. }
        | ResolvedType::TypeParameter { .. } => Err(Deferred::Finite(Refusal::Profile(
            ProfileRefusal::SelectedCollectionElementIsNotABoundedLeaf,
        ))),
        _ => Ok(1),
    }
}
pub(super) fn type_key_bytes_inner(ty: &ResolvedType) -> Result<usize, Deferred> {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(1);
    };
    let nested = match arguments.as_slice() {
        [] => 0,
        [element] => element_bytes_inner(element)?,
        _ => {
            return Err(Deferred::Finite(Refusal::Profile(
                ProfileRefusal::SelectedTypeHasUnsupportedArgumentCount,
            )));
        }
    };
    declaration
        .as_str()
        .len()
        .checked_add(nested)
        .ok_or_else(|| {
            Deferred::Finite(Refusal::Profile(
                ProfileRefusal::SelectedTypeKeyLengthOverflow,
            ))
        })
}
pub(in crate::cleanup_plan::replay) fn type_key_bytes(
    ty: &ResolvedType,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<usize, Error> {
    // Explicit legacy/non-owner entry; this result is never certified prepaid.
    if !budget.selected() || budget.physical.is_none() {
        return super::super::collections::type_key_bytes(ty).map_err(Error::Owned);
    }

    let permit = Permit::profile::<usize>(function, budget)?;
    let outcome = type_key_bytes_inner(ty);
    permit.resolve(outcome)
}

/// Explicit legacy compatibility for a broken/no-owner entry. Valid affine
/// paid Sessions cannot lose their ledger; this is never labelled prepaid.
fn append_unfunded_lost_owner(
    function: &ResolvedFunction,
    binding: &crate::hir::ResolvedBinding,
    value: &ResolvedExpr,
    budget: &mut ReplayBudget,
) -> Result<bool, Error> {
    super::super::collections::append(function, binding, value, budget).map_err(Error::Owned)
}
