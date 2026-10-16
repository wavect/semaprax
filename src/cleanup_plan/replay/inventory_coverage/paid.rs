//! Sealed selected raw constructor boundary. Outer inventory comparison and
//! supplemental checker are still the exact original owning Diagnostic route;
//! they are explicit #781 residuals, not relabelled budget provenance.
use super::{
    collect_supplemental, inventory_storage_id, replay_error, ReplayBudget, ReplayContext,
    ResolvedFunction,
};
use crate::cleanup_plan::replay::expression_facts::errors::EngineError;
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
    let permit = crate::cleanup_plan::replay::selected_context::child::Permit::acquire::<()>(
        function, budget,
    )?;
    admit_initial!(budget, function, permit);
    let inventory = {
        let ReplayBudget {
            remaining,
            physical,
            ..
        } = budget;
        let physical = physical.as_deref_mut().ok_or_else(|| {
            replay_error(
                function,
                "selected inventory coverage requires a physical ledger",
            )
        })?;
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
        return Err(replay_error(
            function,
            "cleanup inventory disagrees with independently rebuilt typed HIR",
        )
        .into());
    }
    let plan = &function.cleanup_plan;
    let base_len = inventory.slots.len();
    if plan.slots.len() < base_len {
        return Err(replay_error(
            function,
            "cleanup plan omits storage required by the cleanup inventory",
        )
        .into());
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
            return Err(replay_error(
                function,
                format_args!(
                    "cleanup plan base slot {index} disagrees with the independent inventory"
                ),
            )
            .into());
        }
    }
    if !inventory
        .entry_state
        .conditional_owned_parameters
        .is_empty()
        || !plan.entry_state.conditional_owned_parameters.is_empty()
    {
        return Err(replay_error(
            function,
            "selected inventory coverage does not support conditional entry owners",
        )
        .into());
    }
    if inventory.entry_state.live_owned_parameters.len()
        != plan.entry_state.live_owned_parameters.len()
    {
        return Err(replay_error(
            function,
            "cleanup plan entry ownership disagrees with the independent inventory",
        )
        .into());
    }
    for (id, place) in inventory
        .entry_state
        .live_owned_parameters
        .iter()
        .zip(&plan.entry_state.live_owned_parameters)
    {
        let slot = inventory
            .slots
            .iter()
            .find(|slot| &slot.id == id)
            .ok_or_else(|| {
                replay_error(
                    function,
                    "cleanup inventory entry references unknown storage",
                )
            })?;
        if !place.projections.is_empty() || place.storage != inventory_storage_id(&slot.origin) {
            return Err(replay_error(
                function,
                "cleanup plan entry ownership disagrees with the independent inventory",
            )
            .into());
        }
    }
    let mut next_flag = u32::try_from(inventory.flags.len())
        .map_err(|_| replay_error(function, "too many inventory liveness flags"))?;
    let mut supplemental = Vec::new();
    for expression in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        collect_supplemental(
            program,
            function,
            expression,
            &mut next_flag,
            &mut supplemental,
            budget,
        )?;
    }
    if plan.slots.len() - base_len != supplemental.len() {
        return Err(replay_error(
            function,
            "cleanup plan has missing or extra call-argument storage",
        )
        .into());
    }
    for (offset, (actual, expected)) in plan.slots[base_len..].iter().zip(&supplemental).enumerate()
    {
        let index = base_len
            .checked_add(offset)
            .and_then(|index| u32::try_from(index).ok())
            .ok_or_else(|| replay_error(function, "too many cleanup slots"))?;
        if actual.id.0 != index
            || actual.storage_index != index
            || actual.storage != expected.storage
            || actual.ty != expected.ty
            || actual.field_liveness_shape != expected.shape
        {
            return Err(replay_error(
                function,
                format_args!("supplemental call-argument slot {offset} disagrees with typed HIR"),
            )
            .into());
        }
    }
    Ok(())
}
