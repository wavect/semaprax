//! Complete selected inventory comparison and supplemental finite-error receiver.
use super::*;
use crate::cleanup_plan::replay::expression_facts::errors::EngineError;
use crate::cleanup_plan::replay::selected_context::{child::Permit, family::Reason};
pub(in crate::cleanup_plan::replay) mod queries;
mod shape;
mod supplemental;
use supplemental::collect_supplemental;
// Lexical admission before new child arguments/results exist. No forwarding
// helper, closure capture, erased argument vector or authority is introduced.
macro_rules! receive {
    ($budget:ident, $function:ident, $roles:ty, $fields:expr, $phase:literal, $call:expr) => {{
        $budget.reserve_array::<$roles>($function, 1, $phase)?;
        $budget.charge($function, $fields, $phase)?;
        $call
    }};
}
pub(in crate::cleanup_plan::replay) use receive;
// Unit-valued test loops observe admission refusal in this existing call's
// Result instead of escaping through `?`. No extra closure, function, argument
// bank or Result ABI is formed. The same reserve and work debit precede every
// original call argument, including short/zero rows. Production keeps `receive`.
#[cfg(test)]
macro_rules! receive_observed {
    ($budget:ident, $function:ident, $roles:ty, $fields:expr, $phase:literal, $call:expr) => {{
        match $budget.reserve_array::<$roles>($function, 1, $phase) {
            Err(error) => Err(error.into()),
            Ok(()) => match $budget.charge($function, $fields, $phase) {
                Err(error) => Err(error.into()),
                Ok(()) => $call,
            },
        }
    }};
}
#[cfg(test)]
pub(in crate::cleanup_plan::replay) use receive_observed;

// Actual wrapper callee formals are owned by this lexical CALLER frontier.
// Retain the original actual-argument/result packet independently below it.
macro_rules! receive_wrapper {
    ($budget:ident, $function:ident, $roles:ty, $fields:expr, $phase:literal, $call:expr) => {{
        $budget.reserve_array::<crate::cleanup_plan::replay::selected_context::child::WrapperFormalReceiving<'_>>(
            $function, 1, "selected wrapper formal caller frontier",
        )?;
        $budget.charge($function, 3, "selected wrapper formal caller frontier")?;
        crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
            $budget, $function, $roles, $fields, $phase, $call
        )
    }};
}
pub(in crate::cleanup_plan::replay) use receive_wrapper;

// This lexical frontier introduces no preflight helper or forwarding Result.
// The authenticated physical primitive and original checked/zeroing field debit
// both finish before any raw argument.
macro_rules! admit_initial {
    ($budget:ident, $function:ident, $permit:ident) => {{
        match $budget.reserve_bytes(
            $function,
            std::mem::size_of::<crate::cleanup::TypedInventoryInitialReceiving>(),
            "selected raw inventory initial receiving",
        ) {
            Ok(()) => {}
            Err(error) => {
                return Err(
                    crate::cleanup_plan::replay::expression_facts::errors::EngineError::Owned(
                        error,
                    ),
                )
            }
        }
        match crate::cleanup::debit_typed_inventory_initial(
            &mut $budget.remaining,
            crate::cleanup::TYPED_INVENTORY_INITIAL_FIELDS,
        ) {
            Ok(()) => {}
            Err(failure) => {
                return Err($permit.refuse(
                    crate::cleanup_plan::renewal_profile::child_refusal::Refusal::Inventory(
                        failure,
                    ),
                ))
            }
        }
    }};
}
#[cfg(test)]
pub(super) use admit_initial;
pub(in crate::cleanup_plan::replay) fn validate_inventory_coverage(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), EngineError> {
    #[cfg(test)]
    if let Some(lease) = program.borrowed_helper_lease(function) {
        return lease
            .inventory(program, function, budget)
            .map_err(EngineError::Owned);
    }
    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected family call receiving",
        Permit::family_receiver::<()>(function, budget)
    )?;
    type Receiving<'a> = (
        &'a dyn ReplayContext,
        &'a ResolvedFunction,
        &'a mut ReplayBudget,
        crate::cleanup::CleanupInventory,
        usize,
        u32,
        Vec<ExpectedSupplementalSlot>,
        Result<(), EngineError>,
        &'a crate::cleanup_plan::CleanupPlan,
    );
    budget.reserve_array::<Receiving<'_>>(
        function,
        1,
        "selected inventory comparison receiving",
    )?;
    budget.charge(function, 9, "selected inventory comparison receiving")?;
    admit_initial!(budget, function, permit);
    let inventory = {
        let ReplayBudget {
            remaining,
            physical,
            ..
        } = budget;
        let physical = match physical.as_deref_mut() {
            Some(physical) => physical,
            None => {
                return Err(permit.family(Reason::SelectedInventoryCoverageRequiresAPhysicalLedger))
            }
        };
        match crate::cleanup::build_prepaid_typed_flat_record_inventory(
            program, function, physical, remaining,
        ) {
            Ok(inventory) => inventory,
            Err(failure) => {
                return Err(permit.refuse(
                    crate::cleanup_plan::renewal_profile::child_refusal::Refusal::Inventory(
                        failure,
                    ),
                ))
            }
        }
    };
    if inventory != function.cleanup {
        return Err(
            permit.family(Reason::CleanupInventoryDisagreesWithIndependentlyRebuiltTypedHir)
        );
    }
    let plan = &function.cleanup_plan;
    let base_len = inventory.slots.len();
    if plan.slots.len() < base_len {
        return Err(permit.family(Reason::CleanupPlanOmitsStorageRequiredByTheCleanupInventory));
    }
    for (index, slot) in inventory.slots.iter().enumerate() {
        let actual = &plan.slots[index];
        let storage = inventory_storage_id(&slot.origin);
        if actual.id.0 != slot.id.0
            || actual.storage_index != slot.discovery_index
            || actual.storage != storage
            || actual.ty != slot.ty
            || actual.field_liveness_shape != slot.shape
        {
            return Err(permit.family(Reason::BaseSlot(index)));
        }
    }
    if !inventory
        .entry_state
        .conditional_owned_parameters
        .is_empty()
        || !plan.entry_state.conditional_owned_parameters.is_empty()
    {
        return Err(
            permit.family(Reason::SelectedInventoryCoverageDoesNotSupportConditionalEntryOwners)
        );
    }
    if inventory.entry_state.live_owned_parameters.len()
        != plan.entry_state.live_owned_parameters.len()
    {
        return Err(
            permit.family(Reason::CleanupPlanEntryOwnershipDisagreesWithTheIndependentInventory)
        );
    }
    for (id, place) in inventory
        .entry_state
        .live_owned_parameters
        .iter()
        .zip(&plan.entry_state.live_owned_parameters)
    {
        let slot = match inventory.slots.iter().find(|slot| &slot.id == id) {
            Some(slot) => slot,
            None => {
                return Err(permit.family(Reason::CleanupInventoryEntryReferencesUnknownStorage))
            }
        };
        if !place.projections.is_empty() || place.storage != inventory_storage_id(&slot.origin) {
            return Err(permit
                .family(Reason::CleanupPlanEntryOwnershipDisagreesWithTheIndependentInventory));
        }
    }
    let mut next_flag = match u32::try_from(inventory.flags.len()) {
        Ok(next_flag) => next_flag,
        Err(_) => return Err(permit.family(Reason::TooManyInventoryLivenessFlags)),
    };
    let mut supplemental = Vec::new();
    for expression in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
            budget,
            function,
            (
                &dyn crate::cleanup::ReplayContext,
                &crate::hir::ResolvedFunction,
                &crate::hir::ResolvedExpr,
                &mut u32,
                &mut Vec<ExpectedSupplementalSlot>,
                &mut crate::cleanup_plan::replay::ReplayBudget,
                Result<(), crate::cleanup_plan::replay::expression_facts::errors::EngineError>
            ),
            7,
            "selected supplemental call receiving",
            collect_supplemental(
                program,
                function,
                expression,
                &mut next_flag,
                &mut supplemental,
                budget,
            )
        )?;
    }
    if plan.slots.len() - base_len != supplemental.len() {
        return Err(permit.family(Reason::CleanupPlanHasMissingOrExtraCallArgumentStorage));
    }
    for (offset, (actual, expected)) in plan.slots[base_len..].iter().zip(&supplemental).enumerate()
    {
        let index = match base_len
            .checked_add(offset)
            .and_then(|index| u32::try_from(index).ok())
        {
            Some(index) => index,
            None => return Err(permit.family(Reason::TooManyCleanupSlots)),
        };
        if actual.id.0 != index
            || actual.storage_index != index
            || actual.storage != expected.storage
            || actual.ty != expected.ty
            || actual.field_liveness_shape != expected.shape
        {
            return Err(permit.family(Reason::SupplementalSlot(offset)));
        }
    }
    Ok(())
}
