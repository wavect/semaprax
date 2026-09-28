//! Actual compiler storage/flags and canonical cleanup at a consuming record
//! constructor's field-transfer boundaries. This is not runtime authority.
use crate::cleanup::{FieldLivenessShape, LivenessFlagId};
use crate::cleanup_plan::{
    CleanupPlace, CleanupTransition, ExitContinuation, FinalizeAction, StorageId,
};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    DeclarationId, DeclarationIndex, ExpressionId, ResolvedExpr, ResolvedExprKind,
    ResolvedFunction, ResolvedType,
};

#[derive(Clone)]
pub(crate) struct OwnedRecordFieldTransfer {
    pub field_index: usize,
    pub at: ExpressionId,
    pub source: CleanupPlace,
    pub destination: CleanupPlace,
}
#[derive(Clone)]
pub(crate) struct OwnedRecordTransferPlan {
    pub fields: Vec<OwnedRecordFieldTransfer>,
    /// Index is the number of committed in-process field transfers. Copy
    /// evaluation never changes this basis; a failed charge does not transfer.
    pub failure_by_prefix: Vec<Vec<FinalizeAction>>,
    pub result_disposal: Vec<FinalizeAction>,
    pub completion_cleanup: Vec<FinalizeAction>,
}
fn refused() -> Diagnostic {
    Diagnostic::io("SPX-T303", "owned stage constructor transfer proof differs")
}

/// The caller admits only one direct record constructor, consuming every Bytes
/// leaf of one flat parameter exactly once. The ordinary HIR validator has
/// already reconstructed all slots/transitions; this query independently binds
/// the runtime's finite phase basis to their actual flags and storage.
pub(crate) fn owned_record_transfer_plan(
    declarations: &DeclarationIndex,
    function: &ResolvedFunction,
    constructor: &ResolvedExpr,
) -> Result<OwnedRecordTransferPlan, Diagnostic> {
    let ResolvedExprKind::ConstructRecord { fields, .. } = &constructor.kind else {
        return Err(refused());
    };
    if function.params.len() != 1 {
        return Err(refused());
    }
    let input_storage = StorageId::Value(function.params[0].id.clone());
    let temporary = StorageId::Temporary(constructor.id.clone());
    if function.cleanup_plan.entry_state.live_owned_parameters
        != [CleanupPlace {
            storage: input_storage.clone(),
            projections: vec![],
        }]
        || !function
            .cleanup_plan
            .entry_state
            .conditional_owned_parameters
            .is_empty()
    {
        return Err(refused());
    }
    let input = flat_slot(
        declarations,
        function,
        &input_storage,
        &function.params[0].ty,
    )?;
    let temp = flat_slot(declarations, function, &temporary, &constructor.ty)?;
    let result = flat_slot(
        declarations,
        function,
        &StorageId::ProvisionalResult,
        &function.return_type,
    )?;
    if input.len() != temp.len() || temp.len() != result.len() || input.is_empty() {
        return Err(refused());
    }
    let mut metadata = input
        .iter()
        .chain(&temp)
        .chain(&result)
        .cloned()
        .collect::<Vec<_>>();
    let mut live = input.iter().map(|(flag, _, _)| *flag).collect::<Vec<_>>();
    let vector = |live: &[LivenessFlagId],
                  metadata: &[(LivenessFlagId, CleanupPlace, DeclarationId)]| {
        crate::cleanup_plan::build::canonical_finalizers_for(
            live,
            |flag| {
                let (_, place, lifecycle) = metadata
                    .iter()
                    .find(|(id, _, _)| *id == flag)
                    .expect("checked compiler flag");
                (place.clone(), lifecycle.clone())
            },
            |_| true,
        )
    };
    let mut failure_by_prefix = vec![vector(&live, &metadata)];
    let mut transfers = Vec::new();
    for (field_index, field) in fields.iter().enumerate() {
        if field.value.ty != ResolvedType::Bytes {
            continue;
        }
        let ResolvedExprKind::Place(place) = &field.value.kind else {
            return Err(refused());
        };
        let [crate::hir::PlaceProjection::Field(source_field)] = place.projections.as_slice()
        else {
            return Err(refused());
        };
        if place.root != function.params[0].id {
            return Err(refused());
        }
        let source = CleanupPlace {
            storage: input_storage.clone(),
            projections: vec![source_field.clone()],
        };
        let destination = CleanupPlace {
            storage: temporary.clone(),
            projections: vec![field.field.clone()],
        };
        let mut exact = function.cleanup_plan.blocks.iter().flat_map(|b| &b.transitions).filter(|t| matches!(t,
            CleanupTransition::Transfer { at, source: s, destination: d } if *at == field.value.id && *s == source && *d == destination));
        if exact.next().is_none() || exact.next().is_some() {
            return Err(refused());
        }
        let source_flag = input
            .iter()
            .find(|(_, p, _)| *p == source)
            .map(|(id, _, _)| *id)
            .ok_or_else(refused)?;
        let destination_flag = temp
            .iter()
            .find(|(_, p, _)| *p == destination)
            .map(|(id, _, _)| *id)
            .ok_or_else(refused)?;
        if !live.contains(&source_flag) || live.contains(&destination_flag) {
            return Err(refused());
        }
        live.retain(|id| *id != source_flag);
        live.push(destination_flag);
        transfers.push(OwnedRecordFieldTransfer {
            field_index,
            at: field.value.id.clone(),
            source,
            destination,
        });
        failure_by_prefix.push(vector(&live, &metadata));
    }
    if transfers.len() != input.len()
        || live
            .iter()
            .any(|flag| input.iter().any(|(id, _, _)| id == flag))
    {
        return Err(refused());
    }
    // The ordinary compiler normalizes a completed record into its structural
    // flag order before moving it onward. Follow the actual whole-root transfer
    // chain through enclosing expression slots to ProvisionalResult; do not
    // assume the constructor directly transfers to the result slot.
    let mut storage = temporary;
    let mut visited = Vec::new();
    while storage != StorageId::ProvisionalResult {
        if visited.contains(&storage) || visited.len() > function.cleanup_plan.slots.len() {
            return Err(refused());
        }
        visited.push(storage.clone());
        let mut next = function
            .cleanup_plan
            .blocks
            .iter()
            .flat_map(|b| &b.transitions)
            .filter_map(|t| match t {
                CleanupTransition::Transfer {
                    source,
                    destination,
                    ..
                } if source.storage == storage
                    && source.projections.is_empty()
                    && destination.projections.is_empty() =>
                {
                    Some(destination.storage.clone())
                }
                _ => None,
            });
        let destination = next.next().ok_or_else(refused)?;
        if next.next().is_some() {
            return Err(refused());
        }
        let flags = flat_slot(declarations, function, &destination, &constructor.ty)?;
        if flags.len() != temp.len() {
            return Err(refused());
        }
        for entry in &flags {
            if !metadata.iter().any(|(id, _, _)| *id == entry.0) {
                metadata.push(entry.clone());
            }
        }
        live = flags.iter().map(|(id, _, _)| *id).collect();
        storage = destination;
    }
    let result_disposal = vector(&live, &metadata);
    let mut commits = function
        .cleanup_plan
        .exits
        .iter()
        .filter(|e| matches!(e.continuation, ExitContinuation::CommitResult { .. }));
    let commit = commits.next().ok_or_else(refused)?;
    if commits.next().is_some() || !commit.finalize_in_order.is_empty() {
        return Err(refused());
    }
    // Every ordinary failure path within this exact source profile must agree
    // with one of the compiler phase vectors (including the provisional tail).
    for exit in &function.cleanup_plan.exits {
        if matches!(exit.continuation, ExitContinuation::ReturnFailure { .. })
            && !failure_by_prefix.contains(&exit.finalize_in_order)
            && exit.finalize_in_order != result_disposal
        {
            return Err(refused());
        }
    }
    Ok(OwnedRecordTransferPlan {
        fields: transfers,
        failure_by_prefix,
        result_disposal,
        completion_cleanup: commit.finalize_in_order.clone(),
    })
}
fn flat_slot(
    declarations: &DeclarationIndex,
    function: &ResolvedFunction,
    storage: &StorageId,
    ty: &ResolvedType,
) -> Result<Vec<(LivenessFlagId, CleanupPlace, DeclarationId)>, Diagnostic> {
    let slot = function
        .cleanup_plan
        .slots
        .iter()
        .find(|s| s.storage == *storage)
        .ok_or_else(refused)?;
    let inventory = function
        .cleanup
        .slots
        .get(slot.storage_index as usize)
        .ok_or_else(refused)?;
    let origin_matches = match (storage, &inventory.origin) {
        (StorageId::Value(id), crate::cleanup::CleanupStorageOrigin::Parameter { value, .. }) => {
            id == value
        }
        (
            StorageId::Temporary(id),
            crate::cleanup::CleanupStorageOrigin::Temporary { expression },
        ) => id == expression,
        (
            StorageId::ProvisionalResult,
            crate::cleanup::CleanupStorageOrigin::ProvisionalResult { value },
        ) => value == &function.result_id,
        _ => false,
    };
    if !origin_matches
        || slot.ty != *ty
        || !crate::cleanup::field_liveness_shapes_equal(
            &inventory.shape,
            &slot.field_liveness_shape,
        )?
    {
        return Err(refused());
    }
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Err(refused());
    };
    let FieldLivenessShape::Record {
        declaration: actual,
        fields,
    } = &slot.field_liveness_shape
    else {
        return Err(refused());
    };
    let expected = declarations
        .record_fields(declaration)
        .ok_or_else(refused)?;
    if !arguments.is_empty() || actual != declaration || fields.len() != expected.len() {
        return Err(refused());
    }
    let mut output = Vec::new();
    for (index, (field, expected)) in fields.iter().zip(expected).enumerate() {
        if field.field != expected.id || field.field_index != index as u32 {
            return Err(refused());
        }
        match (&field.shape, &expected.ty) {
            (FieldLivenessShape::NoDrop, ty) if crate::hir::is_scalar_resolved_type(ty) => {}
            (FieldLivenessShape::Leaf { flag, lifecycle }, ResolvedType::Bytes)
                if lifecycle.as_str() == crate::cleanup::BYTES_DROP_LIFECYCLE_ID =>
            {
                let facts = function
                    .cleanup
                    .flags
                    .iter()
                    .filter(|f| f.id == *flag)
                    .collect::<Vec<_>>();
                if facts.len() != 1
                    || facts[0].place.storage != inventory.id
                    || facts[0].place.projections != [field.field.clone()]
                    || facts[0].lifecycle != *lifecycle
                    || output.iter().any(|(id, _, _)| id == flag)
                {
                    return Err(refused());
                }
                output.push((
                    *flag,
                    CleanupPlace {
                        storage: storage.clone(),
                        projections: vec![field.field.clone()],
                    },
                    lifecycle.clone(),
                ));
            }
            _ => return Err(refused()),
        }
    }
    if function
        .cleanup
        .flags
        .iter()
        .filter(|f| f.place.storage == inventory.id)
        .count()
        != output.len()
    {
        return Err(refused());
    }
    Ok(output)
}

#[derive(Clone)]
pub(crate) struct OwnedStepCaseTransferPlan {
    pub constructor: ExpressionId,
    pub case: DeclarationId,
    pub fields: Vec<OwnedRecordFieldTransfer>,
    pub failure_by_prefix: Vec<Vec<FinalizeAction>>,
    /// These are the live guards after this branch. The completion vector is
    /// the original compiler vector, including its false guarded operations.
    pub completion_live_flags: Vec<LivenessFlagId>,
}

#[derive(Clone)]
pub(crate) struct OwnedStepTransferPlan {
    pub initial_disposal: Vec<FinalizeAction>,
    pub cases: Vec<OwnedStepCaseTransferPlan>,
    pub completion_cleanup: Vec<FinalizeAction>,
    pub provisional_failure: Vec<FinalizeAction>,
    pub result_disposal: Vec<FinalizeAction>,
}

type LeafFacts = (LivenessFlagId, CleanupPlace, DeclarationId);

/// The closed reducer profile passes its actual constructors, not synthetic
/// HIR. Every ownership edge and guard below comes from ordinary cleanup.
pub(crate) fn owned_step_transfer_plan(
    declarations: &DeclarationIndex,
    function: &ResolvedFunction,
    constructors: &[&ResolvedExpr],
) -> Result<OwnedStepTransferPlan, Diagnostic> {
    let params = function
        .params
        .iter()
        .filter(|p| p.ownership == crate::hir::OwnershipMode::Own)
        .collect::<Vec<_>>();
    if params.len() != 2 || constructors.is_empty() || constructors.len() > 4 {
        return Err(refused());
    }
    let expected_entry = params
        .iter()
        .map(|p| CleanupPlace {
            storage: StorageId::Value(p.id.clone()),
            projections: vec![],
        })
        .collect::<Vec<_>>();
    if function.cleanup_plan.entry_state.live_owned_parameters != expected_entry
        || !function
            .cleanup_plan
            .entry_state
            .conditional_owned_parameters
            .is_empty()
    {
        return Err(refused());
    }
    let mut inputs = Vec::new();
    for p in &params {
        let leaves = flat_slot(
            declarations,
            function,
            &StorageId::Value(p.id.clone()),
            &p.ty,
        )?;
        if leaves.is_empty() {
            return Err(refused());
        }
        inputs.extend(leaves);
    }
    let initial = inputs.iter().map(|(id, _, _)| *id).collect::<Vec<_>>();
    let initial_disposal = finalize_leaves(&initial, &inputs);
    let result = variant_slot(declarations, function, &StorageId::ProvisionalResult)?;
    let ResolvedType::Nominal {
        declaration: variant,
        arguments,
    } = &function.return_type
    else {
        return Err(refused());
    };
    if !arguments.is_empty() {
        return Err(refused());
    }
    let mut cases = Vec::new();
    for constructor in constructors {
        let ResolvedExprKind::ConstructVariant { case, fields, .. } = &constructor.kind else {
            return Err(refused());
        };
        if cases
            .iter()
            .any(|p: &OwnedStepCaseTransferPlan| p.case == *case)
        {
            return Err(refused());
        }
        let temporary = StorageId::Temporary(constructor.id.clone());
        let temp = variant_slot(declarations, function, &temporary)?;
        let selected = temp.iter().find(|(id, _)| id == case).ok_or_else(refused)?;
        let mut facts = inputs.clone();
        facts.extend(selected.1.iter().cloned());
        let mut live = initial.clone();
        let mut failure_by_prefix = vec![initial_disposal.clone()];
        let mut transfers = Vec::new();
        for (field_index, field) in fields.iter().enumerate() {
            if field.value.ty != ResolvedType::Bytes {
                continue;
            }
            let ResolvedExprKind::Place(place) = &field.value.kind else {
                return Err(refused());
            };
            let [crate::hir::PlaceProjection::Field(id)] = place.projections.as_slice() else {
                return Err(refused());
            };
            if !params.iter().any(|p| p.id == place.root) {
                return Err(refused());
            }
            let source = CleanupPlace {
                storage: StorageId::Value(place.root.clone()),
                projections: vec![id.clone()],
            };
            let destination = CleanupPlace {
                storage: temporary.clone(),
                projections: vec![case.clone(), field.field.clone()],
            };
            let mut edges = function.cleanup_plan.blocks.iter().flat_map(|b| &b.transitions).filter(|t| matches!(t, CleanupTransition::Transfer { at, source: s, destination: d } if *at == field.value.id && *s == source && *d == destination));
            if edges.next().is_none() || edges.next().is_some() {
                return Err(refused());
            }
            let source_flag = inputs
                .iter()
                .find(|(_, p, _)| *p == source)
                .map(|(id, _, _)| *id)
                .ok_or_else(refused)?;
            let destination_flag = selected
                .1
                .iter()
                .find(|(_, p, _)| *p == destination)
                .map(|(id, _, _)| *id)
                .ok_or_else(refused)?;
            if !live.contains(&source_flag) || live.contains(&destination_flag) {
                return Err(refused());
            }
            live.retain(|id| *id != source_flag);
            live.push(destination_flag);
            transfers.push(OwnedRecordFieldTransfer {
                field_index,
                at: field.value.id.clone(),
                source,
                destination,
            });
            failure_by_prefix.push(finalize_leaves(&live, &facts));
        }
        if transfers.len() != selected.1.len() {
            return Err(refused());
        }
        let remaining = initial
            .iter()
            .filter(|id| live.contains(id))
            .copied()
            .collect::<Vec<_>>();
        // Whole variant transfers preserve the selected case and actual flag
        // namespace at every enclosing expression; no record-style shortcut.
        let mut storage = temporary;
        let mut visited = Vec::new();
        while storage != StorageId::ProvisionalResult {
            if visited.contains(&storage) || visited.len() > function.cleanup_plan.slots.len() {
                return Err(refused());
            }
            visited.push(storage.clone());
            let mut edges = function
                .cleanup_plan
                .blocks
                .iter()
                .flat_map(|b| &b.transitions)
                .filter_map(|t| match t {
                    CleanupTransition::TransferVariant {
                        source,
                        destination,
                        variant: actual,
                        ..
                    } if source.storage == storage
                        && source.projections.is_empty()
                        && destination.projections.is_empty()
                        && actual == variant =>
                    {
                        Some(destination.storage.clone())
                    }
                    _ => None,
                });
            let destination = edges.next().ok_or_else(refused)?;
            if edges.next().is_some() {
                return Err(refused());
            }
            let slots = variant_slot(declarations, function, &destination)?;
            if slots
                .iter()
                .find(|(id, _)| id == case)
                .ok_or_else(refused)?
                .1
                .len()
                != selected.1.len()
            {
                return Err(refused());
            }
            storage = destination;
        }
        cases.push(OwnedStepCaseTransferPlan {
            constructor: constructor.id.clone(),
            case: case.clone(),
            fields: transfers,
            failure_by_prefix,
            completion_live_flags: remaining,
        });
    }
    let mut commits = function
        .cleanup_plan
        .exits
        .iter()
        .filter(|e| matches!(e.continuation, ExitContinuation::CommitResult { .. }));
    let commit = commits.next().ok_or_else(refused)?;
    if commits.next().is_some() {
        return Err(refused());
    }
    let union = initial
        .iter()
        .filter(|id| cases.iter().any(|p| p.completion_live_flags.contains(id)))
        .copied()
        .collect::<Vec<_>>();
    let completion_cleanup = finalize_leaves(&union, &inputs);
    if completion_cleanup != commit.finalize_in_order {
        return Err(refused());
    }
    let ordered = result
        .iter()
        .map(|(case, fields)| {
            (
                case.clone(),
                fields.iter().map(|(id, _, _)| *id).collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();
    let metadata = result
        .iter()
        .flat_map(|(_, fields)| fields)
        .collect::<Vec<_>>();
    let conditional_finalizers = |ordered: &[(DeclarationId, Vec<LivenessFlagId>)]| {
        crate::cleanup_plan::build::canonical_conditional_finalizers_for(
            &CleanupPlace {
                storage: StorageId::ProvisionalResult,
                projections: vec![],
            },
            variant,
            ordered,
            |flag| {
                let (_, place, lifecycle) = metadata
                    .iter()
                    .find(|(id, _, _)| *id == flag)
                    .expect("checked result flag");
                (place.clone(), lifecycle.clone())
            },
            |_| true,
        )
    };
    let result_disposal = conditional_finalizers(&ordered);
    // The ordinary return failure carries only cases reachable through the
    // proven source constructors/whole-variant transfer chains. Keep their
    // declaration inventory order; full result disposal remains separate.
    let returned_cases = ordered
        .iter()
        .filter(|(case, _)| cases.iter().any(|constructor| &constructor.case == case))
        .cloned()
        .collect::<Vec<_>>();
    let mut provisional_failure = completion_cleanup.clone();
    provisional_failure.extend(conditional_finalizers(&returned_cases));
    for exit in &function.cleanup_plan.exits {
        if matches!(exit.continuation, ExitContinuation::ReturnFailure { .. })
            && exit.finalize_in_order != initial_disposal
            && exit.finalize_in_order != provisional_failure
            && !cases
                .iter()
                .any(|p| p.failure_by_prefix.contains(&exit.finalize_in_order))
        {
            return Err(refused());
        }
    }
    Ok(OwnedStepTransferPlan {
        initial_disposal,
        cases,
        completion_cleanup,
        provisional_failure,
        result_disposal,
    })
}

fn finalize_leaves(live: &[LivenessFlagId], facts: &[LeafFacts]) -> Vec<FinalizeAction> {
    crate::cleanup_plan::build::canonical_finalizers_for(
        live,
        |flag| {
            let (_, place, lifecycle) = facts
                .iter()
                .find(|(id, _, _)| *id == flag)
                .expect("checked compiler leaf");
            (place.clone(), lifecycle.clone())
        },
        |_| true,
    )
}

fn variant_slot(
    declarations: &DeclarationIndex,
    function: &ResolvedFunction,
    storage: &StorageId,
) -> Result<Vec<(DeclarationId, Vec<LeafFacts>)>, Diagnostic> {
    let slot = function
        .cleanup_plan
        .slots
        .iter()
        .find(|s| s.storage == *storage)
        .ok_or_else(refused)?;
    let inventory = function
        .cleanup
        .slots
        .get(slot.storage_index as usize)
        .ok_or_else(refused)?;
    let origin = match (storage, &inventory.origin) {
        (
            StorageId::Temporary(id),
            crate::cleanup::CleanupStorageOrigin::Temporary { expression },
        ) => id == expression,
        (
            StorageId::ProvisionalResult,
            crate::cleanup::CleanupStorageOrigin::ProvisionalResult { value },
        ) => value == &function.result_id,
        _ => false,
    };
    if !origin
        || slot.ty != function.return_type
        || !crate::cleanup::field_liveness_shapes_equal(
            &inventory.shape,
            &slot.field_liveness_shape,
        )?
    {
        return Err(refused());
    }
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = &slot.ty
    else {
        return Err(refused());
    };
    let FieldLivenessShape::Variant {
        declaration: actual,
        cases,
    } = &slot.field_liveness_shape
    else {
        return Err(refused());
    };
    let declared = declarations
        .variant_cases(declaration)
        .ok_or_else(refused)?;
    if !arguments.is_empty() || actual != declaration || cases.len() != declared.len() {
        return Err(refused());
    }
    let mut output = Vec::new();
    let mut seen = Vec::new();
    for (index, (case, expected)) in cases.iter().zip(declared).enumerate() {
        let fields = declarations.case_fields(&expected.id).ok_or_else(refused)?;
        if case.case != expected.id
            || case.case_index != index as u32
            || case.fields.len() != fields.len()
        {
            return Err(refused());
        }
        let mut leaves = Vec::new();
        for (index, (field, expected)) in case.fields.iter().zip(fields).enumerate() {
            if field.field != expected.id || field.field_index != index as u32 {
                return Err(refused());
            }
            match (&field.shape, &expected.ty) {
                (FieldLivenessShape::NoDrop, ty) if crate::hir::is_scalar_resolved_type(ty) => {}
                (FieldLivenessShape::Leaf { flag, lifecycle }, ResolvedType::Bytes)
                    if lifecycle.as_str() == crate::cleanup::BYTES_DROP_LIFECYCLE_ID =>
                {
                    let facts = function
                        .cleanup
                        .flags
                        .iter()
                        .filter(|f| f.id == *flag)
                        .collect::<Vec<_>>();
                    if facts.len() != 1
                        || seen.contains(flag)
                        || facts[0].place.storage != inventory.id
                        || facts[0].place.projections != [case.case.clone(), field.field.clone()]
                        || facts[0].lifecycle != *lifecycle
                    {
                        return Err(refused());
                    }
                    seen.push(*flag);
                    leaves.push((
                        *flag,
                        CleanupPlace {
                            storage: storage.clone(),
                            projections: facts[0].place.projections.clone(),
                        },
                        lifecycle.clone(),
                    ));
                }
                _ => return Err(refused()),
            }
        }
        output.push((case.case.clone(), leaves));
    }
    if function
        .cleanup
        .flags
        .iter()
        .filter(|f| f.place.storage == inventory.id)
        .count()
        != seen.len()
    {
        return Err(refused());
    }
    Ok(output)
}
