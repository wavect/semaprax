//! Complete60ad initial entry, retained only for historical typed controls.
use super::note_metadata;
use super::receiving::before_initial::{INITIAL_BYTES, INITIAL_CONTROL_INITIALIZERS};
use super::{debit, InventoryBuilder, InventoryFailure, Kind, Reason};
use crate::cleanup::{
    CleanupEntryState, CleanupInventory, CleanupStorageOrigin, FieldLivenessShape,
    CLEANUP_INVENTORY_SCHEMA_V1, CLEANUP_INVENTORY_SCHEMA_V2,
};
use crate::cleanup_plan::ReplayPhysicalReservation;
use crate::hir::{OwnershipMode, ResolvedFunction};
use std::cell::RefCell;

pub(crate) fn build(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    physical: &mut ReplayPhysicalReservation,
    remaining: &mut usize,
) -> Result<CleanupInventory, InventoryFailure> {
    match physical.reserve(INITIAL_BYTES) {
        Ok(()) => {}
        Err(_) => return Err(InventoryFailure(Kind::Receiving)),
    }
    debit(remaining, INITIAL_CONTROL_INITIALIZERS)?;
    #[cfg(test)]
    note_metadata(INITIAL_CONTROL_INITIALIZERS);
    let mut builder = InventoryBuilder {
        ledger: RefCell::new(Ledger {
            physical,
            remaining,
        }),
        program,
        slots: Vec::new(),
        flags: Vec::new(),
        live_owned_parameters: Vec::new(),
        conditional_owned_parameters: Vec::new(),
    };
    for (parameter_index, parameter) in function.params.iter().enumerate() {
        builder.charge(1)?;
        if parameter.ownership == OwnershipMode::Own && builder.needs_drop(&parameter.ty)? {
            let storage = builder.add_slot(
                CleanupStorageOrigin::Parameter {
                    value: builder.clone_owned(&parameter.id)?,
                    parameter_index: u32::try_from(parameter_index).map_err(|_| {
                        InventoryFailure(Kind::Finite(Reason::TooManyFunctionParameters))
                    })?,
                },
                builder.clone_owned(&parameter.ty)?,
            )?;
            if let Some(entry) = builder.conditional_entry(storage)? {
                reserve(
                    &builder.ledger,
                    &mut builder.conditional_owned_parameters,
                    1,
                )?;
                builder.conditional_owned_parameters.push(entry);
            } else {
                reserve(&builder.ledger, &mut builder.live_owned_parameters, 1)?;
                builder.live_owned_parameters.push(storage);
            }
        }
    }

    for expression in &function.requires {
        builder.collect_expression(expression)?;
    }
    builder.collect_expression(&function.body)?;
    if builder.needs_drop(&function.return_type)? {
        builder.add_slot(
            CleanupStorageOrigin::ProvisionalResult {
                value: builder.clone_owned(&function.result_id)?,
            },
            builder.clone_owned(&function.return_type)?,
        )?;
    }
    for expression in &function.ensures {
        builder.collect_expression(expression)?;
    }

    let schema = if builder
        .slots
        .iter()
        .any(|slot| matches!(slot.shape, FieldLivenessShape::Variant { .. }))
    {
        CLEANUP_INVENTORY_SCHEMA_V2
    } else {
        CLEANUP_INVENTORY_SCHEMA_V1
    };
    Ok(CleanupInventory {
        schema,
        entry_state: CleanupEntryState {
            live_owned_parameters: builder.live_owned_parameters,
            conditional_owned_parameters: builder.conditional_owned_parameters,
        },
        slots: builder.slots,
        flags: builder.flags,
    })
}
