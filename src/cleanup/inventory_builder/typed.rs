//! Sealed mandatory selected inventory constructor. The original Diagnostic
//! builder/callback remain independent. No Diagnostic conversion exists here.
use crate::cleanup::{
    collect_shape_flags, CleanupEntryState, CleanupFlag, CleanupInventory, CleanupStorageId,
    CleanupStorageOrigin, CleanupStorageSlot, ConditionalVariantCase, ConditionalVariantEntry,
    FieldLivenessShape, CLEANUP_INVENTORY_SCHEMA_V1, CLEANUP_INVENTORY_SCHEMA_V2,
};
use crate::cleanup_plan::{replay_reserve_vec, ReplayPhysicalReservation};
use crate::hir::{OwnershipMode, ResolvedFunction, ResolvedType, ResolvedTypeDeclarationKind};
use std::cell::RefCell;
mod receiving;
use receiving::receiving;
pub(crate) use receiving::{InitialReceiving, INITIAL_CONTROL_INITIALIZERS};
#[cfg(test)]
pub(crate) mod before_initial;
mod queries;
use queries::{flat_fields, needs_drop_in, primitive_drop, vector_element};
mod failure;
mod selected;
mod traversal;
pub(crate) use failure::InventoryFailure;
use failure::{Kind, Physical, Reason};
struct Ledger<'a> {
    physical: &'a mut ReplayPhysicalReservation,
    remaining: &'a mut usize,
}
struct InventoryBuilder<'a> {
    ledger: RefCell<Ledger<'a>>,
    program: &'a dyn crate::cleanup::ReplayContext,
    slots: Vec<CleanupStorageSlot>,
    flags: Vec<CleanupFlag>,
    live_owned_parameters: Vec<CleanupStorageId>,
    conditional_owned_parameters: Vec<ConditionalVariantEntry>,
}
macro_rules! key_bytes {
    ($ty:expr) => {{
        let ty = $ty;
        'key: {
            let ResolvedType::Nominal {
                declaration,
                arguments,
            } = ty
            else {
                break 'key 1;
            };
            let nested = match arguments.as_slice() {
                [] => 0,
                [ResolvedType::Nominal {
                    declaration,
                    arguments,
                }] if arguments.is_empty() => declaration.as_str().len(),
                [element]
                    if crate::vec_ops::resolved_vec_element_is_admitted(element)
                        || *element == ResolvedType::String =>
                {
                    1
                }
                _ => {
                    return Err(InventoryFailure(Kind::Finite(
                        Reason::SelectedInventoryTypeHasUnsupportedArguments,
                    )));
                }
            };
            break 'key declaration
                .as_str()
                .len()
                .checked_add(nested)
                .ok_or_else(|| {
                    InventoryFailure(Kind::Finite(Reason::SelectedInventoryKeyLengthOverflow))
                })?;
        }
    }};
}
/// Same actual two fields destructured from the invocation ReplayBudget. This
/// API creates no accounting owner or allowance and cannot accept a Diagnostic;
/// Ledger below is only a synchronous pair of borrows into those existing fields.
pub(crate) fn build_after_initial_receiving(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    physical: &mut ReplayPhysicalReservation,
    remaining: &mut usize,
) -> Result<CleanupInventory, InventoryFailure> {
    #[cfg(test)]
    note_initial_entry(physical, remaining);
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
// Sole work-failure producer: the original checked subtraction and zeroing,
// invoked on the actual borrowed ReplayBudget remaining field, in the same order.
pub(crate) fn debit(remaining: &mut usize, units: usize) -> Result<(), InventoryFailure> {
    match remaining.checked_sub(units) {
        Some(next) => {
            *remaining = next;
            Ok(())
        }
        None => {
            *remaining = 0;
            Err(InventoryFailure(Kind::Work))
        }
    }
}
impl InventoryBuilder<'_> {
    fn charge(&self, units: usize) -> Result<(), InventoryFailure> {
        receiving!(&self.ledger, ());
        #[cfg(test)]
        TRACE_ENABLED.with(|enabled| {
            if enabled.get() {
                LOGICAL_TRACE.with(|trace| {
                    trace.borrow_mut().as_mut().unwrap().push(units);
                });
            }
        });
        debit(self.ledger.borrow_mut().remaining, units)
    }
    fn prepay_collection(&self, ty: &ResolvedType) -> Result<(), InventoryFailure> {
        receiving!(&self.ledger, (), receiving::REASON, 1);
        if !matches!(ty, ResolvedType::Nominal { .. }) {
            return Ok(());
        }
        let query_bytes = key_bytes!(ty);
        let program = self.program;
        let fail = || {
            InventoryFailure(Kind::Finite(
                Reason::SelectedCollectionProbePrepaymentRefused,
            ))
        };
        if program.type_count() > 32 {
            return Err(fail());
        }
        let mut bytes = query_bytes.checked_add(1).ok_or_else(fail)?;
        for ordinal in 0..program.type_count() {
            self.charge(1)?;
            let item = program.type_at(ordinal).ok_or_else(fail)?;
            bytes = bytes
                .checked_add(item.id.as_str().len())
                .and_then(|n| n.checked_add(item.name.len() + 1))
                .ok_or_else(fail)?;
            for parameter in &item.type_parameters {
                self.charge(1)?;
                bytes = bytes
                    .checked_add(parameter.name.len() + 1)
                    .ok_or_else(fail)?;
            }
            if let ResolvedTypeDeclarationKind::Record { fields } = &item.kind {
                for field in fields {
                    self.charge(1)?;
                    bytes = bytes
                        .checked_add(field.id.as_str().len())
                        .and_then(|n| n.checked_add(field.name.len() + 1))
                        .ok_or_else(fail)?;
                }
            }
        }
        // Four root/argument/result Vec classifiers plus operation and ownership
        // classification use fewer than 64 indexed probes per key. The separate
        // 128 factor covers current-directory scans and bounded pairwise fields.
        let probes = program
            .declarations()
            .declarations()
            .len()
            .checked_add(1)
            .and_then(|n| n.checked_mul(64))
            .and_then(|n| n.checked_add(128))
            .ok_or_else(fail)?;
        self.charge(bytes.checked_mul(probes).ok_or_else(fail)?)
    }
    fn needs_drop(&self, ty: &ResolvedType) -> Result<bool, InventoryFailure> {
        receiving!(&self.ledger, bool, receiving::REASON_OR_CLASSIFIER, 1);
        if crate::hir::borrowed_scalar::option_u8::is_type_metered(ty, &mut |units| {
            self.charge(units)
        })? {
            return if crate::hir::borrowed_scalar::option_u8::authenticated_metered(
                self.program.declarations(),
                ty,
                &mut |units| self.charge(units),
            )? {
                Ok(false)
            } else {
                Err(InventoryFailure(Kind::Finite(
                    Reason::SelectedOptionMetadataIsUnauthenticated,
                )))
            };
        }
        self.prepay_collection(ty)?;
        Ok(needs_drop_in!(self.program, ty))
    }
    fn shape_for_type(
        &mut self,
        ty: &ResolvedType,
        storage: CleanupStorageId,
    ) -> Result<FieldLivenessShape, InventoryFailure> {
        receiving!(&self.ledger, FieldLivenessShape);
        self.selected_shape(ty, storage)
    }
    fn add_slot(
        &mut self,
        origin: CleanupStorageOrigin,
        ty: ResolvedType,
    ) -> Result<CleanupStorageId, InventoryFailure> {
        receiving!(&self.ledger, CleanupStorageId, receiving::REASON, 1);
        let index = u32::try_from(self.slots.len())
            .map_err(|_| InventoryFailure(Kind::Finite(Reason::TooManyCleanupStorageCandidates)))?;
        let storage = CleanupStorageId(index);
        let shape = self.shape_for_type(&ty, storage)?;
        reserve(&self.ledger, &mut self.slots, 1)?;
        self.slots.push(CleanupStorageSlot {
            id: storage,
            discovery_index: index,
            origin,
            ty,
            shape,
        });
        Ok(storage)
    }

    fn conditional_entry(
        &self,
        storage: CleanupStorageId,
    ) -> Result<Option<ConditionalVariantEntry>, InventoryFailure> {
        receiving!(
            &self.ledger,
            Option<ConditionalVariantEntry>,
            receiving::REASON,
            1
        );
        let Some(slot) = self.slots.iter().find(|slot| slot.id == storage) else {
            return Err(InventoryFailure(Kind::Finite(
                Reason::ConditionalEntryReferencesUnknownStorage,
            )));
        };
        let FieldLivenessShape::Variant { declaration, cases } = &slot.shape else {
            return Ok(None);
        };
        let mut conditional_cases = Vec::with_capacity(cases.len());
        for case in cases {
            let mut live_flags = Vec::new();
            for field in &case.fields {
                collect_shape_flags(&field.shape, &mut live_flags);
            }
            conditional_cases.push(ConditionalVariantCase {
                case: case.case.clone(),
                live_flags,
            });
        }
        Ok(Some(ConditionalVariantEntry {
            storage,
            variant: declaration.clone(),
            cases: conditional_cases,
        }))
    }
}

fn reserve<T>(
    ledger: &RefCell<Ledger<'_>>,
    values: &mut Vec<T>,
    additional: usize,
) -> Result<(), InventoryFailure> {
    receiving!(ledger, (), receiving::PHYSICAL, 1);
    replay_reserve_vec(ledger.borrow_mut().physical, values, additional)
        .map_err(|_| InventoryFailure(Kind::Physical(Physical::Vector)))
}
fn reserve_legacy<T>(
    ledger: &RefCell<Ledger<'_>>,
    values: &mut Vec<T>,
    additional: usize,
    _message: &'static str,
) -> Result<(), InventoryFailure> {
    receiving!(ledger, ());
    reserve(ledger, values, additional)
}

#[cfg(test)]
thread_local! {
    static METADATA_WORK: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
#[cfg(test)]
fn note_metadata(units: usize) {
    METADATA_WORK.with(|value| value.set(value.get().checked_add(units).unwrap()));
}
#[cfg(test)]
pub(crate) fn reset_metadata_work() {
    METADATA_WORK.with(|value| value.set(0));
}
#[cfg(test)]
pub(crate) fn metadata_work() -> usize {
    METADATA_WORK.with(std::cell::Cell::get)
}

#[cfg(test)]
thread_local! {
    static TRACE_ENABLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static LOGICAL_TRACE: RefCell<Option<Vec<usize>>> = const { RefCell::new(None) };
}
// Enabled only for the explicit non-measured differential pass. Every physical
// observer leaves this None, so trace storage never enters allocation windows.
#[cfg(test)]
pub(crate) fn begin_logical_trace() {
    LOGICAL_TRACE.with(|trace| {
        assert!(trace.replace(Some(Vec::new())).is_none());
    });
    TRACE_ENABLED.with(|enabled| assert!(!enabled.replace(true)));
}
#[cfg(test)]
pub(crate) fn take_logical_trace() -> Vec<usize> {
    TRACE_ENABLED.with(|enabled| assert!(enabled.replace(false)));
    LOGICAL_TRACE.with(|trace| trace.take().unwrap())
}

#[cfg(test)]
thread_local! {
    static INITIAL_ENTRIES: std::cell::Cell<(usize, usize, usize, usize)> = const { std::cell::Cell::new((0, 0, 0, 0)) };
}
#[cfg(test)]
fn note_initial_entry(physical: &mut ReplayPhysicalReservation, remaining: &mut usize) {
    INITIAL_ENTRIES.with(|entry| {
        let calls = entry.get().0;
        entry.set((
            calls + 1,
            std::ptr::from_ref(physical) as usize,
            std::ptr::from_ref(remaining) as usize,
            physical.charged(),
        ));
    });
}
#[cfg(test)]
pub(crate) fn reset_initial_entries() {
    INITIAL_ENTRIES.with(|entry| entry.set((0, 0, 0, 0)));
}
#[cfg(test)]
pub(crate) fn initial_entries() -> (usize, usize, usize, usize) {
    INITIAL_ENTRIES.with(std::cell::Cell::get)
}
