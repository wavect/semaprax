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
