//! Complete selected flat_records finite-error twins; old bodies remain independent.
use super::collections;
use super::{Deferred, Error, Permit};
use crate::cleanup::ReplayContext;
use crate::cleanup_plan::renewal_profile::{
    child_refusal::Refusal, profile_refusal::ProfileRefusal,
};

use crate::cleanup_plan::replay::ReplayBudget;
use crate::hir::{OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedFunction, ResolvedType};
pub(super) fn admitted_inner(
    program: &dyn ReplayContext,
    ty: &ResolvedType,
) -> Result<bool, Deferred> {
    if crate::hir::borrowed_scalar::option_u8::is_type(ty) {
        return Ok(
            crate::hir::borrowed_scalar::option_u8::selected_profile_ready()
                && crate::hir::borrowed_scalar::option_u8::authenticated(
                    program.declarations(),
                    ty,
                ),
        );
    }
    if crate::cleanup::selected_inline::selected_vector_element_inline(program, ty)?.is_some() {
        return Ok(true);
    }
    if matches!(ty, ResolvedType::Nominal { .. }) {
        return Ok(
            crate::cleanup::selected_inline::selected_flat_record_fields_inline(program, ty)?
                .is_some(),
        );
    }
    Ok(crate::hir::is_scalar_resolved_type(ty)
        || matches!(
            ty,
            ResolvedType::Bytes
                | ResolvedType::String
                | ResolvedType::Str
                | ResolvedType::SliceU8
                | ResolvedType::ArrayU8(_)
        ))
}
pub(in crate::cleanup_plan::replay) fn admitted(
    program: &dyn ReplayContext,
    ty: &ResolvedType,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<bool, Error> {
    // Explicit legacy/non-owner entry; this result is never certified prepaid.
    if !budget.selected() || budget.physical.is_none() {
        return super::super::super::flat_records::admitted(program, ty).map_err(Error::Owned);
    }

    let permit = Permit::profile::<bool>(function, budget)?;
    let outcome = admitted_inner(program, ty);
    permit.resolve(outcome)
}
pub(super) fn module_inner(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), Deferred> {
    for ordinal in 0..program.type_count() {
        let record = program.type_at(ordinal).ok_or_else(|| {
            Deferred::Finite(Refusal::Profile(
                ProfileRefusal::SelectedTypeInventoryIsIncomplete,
            ))
        })?;
        collections::prepay_inner(program, function, record.id.as_str().len(), budget)?;
        if record.id.as_str() == crate::prelude::VEC_ID {
            budget.child_charge(function, 9, "selected Vec declaration header")?;
            crate::cleanup::selected_inline::authenticate_selected_vector_inline(
                program, &record.id,
            )?;
            continue;
        }
        budget.child_charge(
            function,
            program.type_count().checked_add(9).ok_or_else(|| {
                Deferred::Finite(Refusal::Profile(
                    ProfileRefusal::RecordClassificationWorkOverflowed,
                ))
            })?,
            "selected direct record declaration",
        )?;
        crate::cleanup::selected_inline::selected_flat_record_declaration_inline(
            program, &record.id,
        )?;
    }
    Ok(())
}
pub(in crate::cleanup_plan::replay) fn module(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    // Explicit legacy/non-owner entry; this result is never certified prepaid.
    if !budget.selected() || budget.physical.is_none() {
        return super::super::super::flat_records::module(program, function, budget)
            .map_err(Error::Owned);
    }

    let permit = Permit::profile::<()>(function, budget)?;
    let outcome = module_inner(program, function, budget);
    permit.resolve(outcome)
}
pub(super) fn expression_inner(
    program: &dyn ReplayContext,
    _function: &ResolvedFunction,
    expression: &ResolvedExpr,
) -> Result<(), Deferred> {
    match &expression.kind {
        ResolvedExprKind::ConstructRecord { record, fields } => {
            let Some((owner, declared)) =
                crate::cleanup::selected_inline::selected_flat_record_fields_inline(
                    program,
                    &expression.ty,
                )?
            else {
                return Err(Deferred::Finite(Refusal::Profile(
                    ProfileRefusal::SelectedConstructionHasNoExactDirectRecord,
                )));
            };
            if record != owner
                || fields.len() != declared.len()
                || fields.iter().zip(declared).any(|(value, field)| {
                    value.field != field.id
                        || value.value.ty != field.ty
                        || if matches!(field.ty, ResolvedType::String | ResolvedType::Bytes) {
                            value.value.ownership != OwnershipMode::Own
                        } else {
                            value.value.ownership != OwnershipMode::Value
                        }
                })
            {
                return Err(Deferred::Finite(Refusal::Profile(
                    ProfileRefusal::SelectedConstructionFieldIdentityOrderTypeOrOwnershipDisagrees,
                )));
            }
        }
        ResolvedExprKind::Project { base, field } => {
            let Some((_, fields)) =
                crate::cleanup::selected_inline::selected_flat_record_fields_inline(
                    program, &base.ty,
                )?
            else {
                return Err(Deferred::Finite(Refusal::Profile(
                    ProfileRefusal::SelectedProjectionHasNoExactDirectRecord,
                )));
            };
            let declared = fields
                .iter()
                .find(|item| item.id == *field)
                .ok_or_else(|| {
                    Deferred::Finite(Refusal::Profile(
                        ProfileRefusal::SelectedProjectionFieldIsForeign,
                    ))
                })?;
            if declared.ty != expression.ty {
                return Err(Deferred::Finite(Refusal::Profile(
                    ProfileRefusal::SelectedProjectionTypeDisagreesWithCurrentField,
                )));
            }
        }
        _ => {}
    }
    Ok(())
}
pub(in crate::cleanup_plan::replay) fn expression(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    // Explicit legacy/non-owner entry; this result is never certified prepaid.
    if !budget.selected() || budget.physical.is_none() {
        return super::super::super::flat_records::expression(program, function, expression)
            .map_err(Error::Owned);
    }

    let permit = Permit::profile::<()>(function, budget)?;
    let outcome = expression_inner(program, function, expression);
    permit.resolve(outcome)
}
