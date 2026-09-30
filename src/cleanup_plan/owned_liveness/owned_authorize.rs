//! Compiler-owned disposal of one staged flat owned Decision.
use crate::cleanup::FieldLivenessShape;
use crate::cleanup_plan::{CleanupPlace, FinalizeAction, StorageId};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    DeclarationId, DeclarationIndex, ResolvedExpr, ResolvedExprKind, ResolvedFunction, ResolvedType,
};

pub(crate) fn owned_authorize_result_disposal(
    declarations: &DeclarationIndex,
    function: &ResolvedFunction,
) -> Result<Vec<FinalizeAction>, Diagnostic> {
    variant_disposal(declarations, function, StorageId::ProvisionalResult, None)
}
pub(crate) fn owned_authorize_partial_disposal(
    declarations: &DeclarationIndex,
    function: &ResolvedFunction,
    constructor: &ResolvedExpr,
) -> Result<Vec<FinalizeAction>, Diagnostic> {
    let ResolvedExprKind::ConstructVariant { case, fields, .. } = &constructor.kind else {
        return Err(Diagnostic::io(
            "SPX-T303",
            "owned authorize missing constructor",
        ));
    };
    let Some(first) = fields.first().filter(|f| f.value.ty == ResolvedType::Bytes) else {
        return Err(Diagnostic::io(
            "SPX-T303",
            "owned authorize missing first Bytes field",
        ));
    };
    variant_disposal(
        declarations,
        function,
        StorageId::Temporary(constructor.id.clone()),
        Some((case, &first.field)),
    )
}
fn variant_disposal(
    declarations: &DeclarationIndex,
    function: &ResolvedFunction,
    storage: StorageId,
    partial: Option<(&DeclarationId, &DeclarationId)>,
) -> Result<Vec<FinalizeAction>, Diagnostic> {
    let refuse = || Diagnostic::io("SPX-T303", "owned authorize provisional inventory differs");
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = &function.return_type
    else {
        return Err(refuse());
    };
    if !arguments.is_empty() {
        return Err(refuse());
    }
    let slot = function
        .cleanup_plan
        .slots
        .iter()
        .find(|s| s.storage == storage)
        .ok_or_else(refuse)?;
    let inventory = function
        .cleanup
        .slots
        .get(slot.storage_index as usize)
        .ok_or_else(refuse)?;
    if slot.ty != function.return_type
        || !match (&storage, &inventory.origin) {
            (
                StorageId::ProvisionalResult,
                crate::cleanup::CleanupStorageOrigin::ProvisionalResult { value },
            ) => value == &function.result_id,
            (
                StorageId::Temporary(id),
                crate::cleanup::CleanupStorageOrigin::Temporary { expression },
            ) => id == expression,
            _ => false,
        }
        || !crate::cleanup::field_liveness_shapes_equal(
            &inventory.shape,
            &slot.field_liveness_shape,
        )?
    {
        return Err(refuse());
    }
    let FieldLivenessShape::Variant {
        declaration: actual,
        cases,
    } = &slot.field_liveness_shape
    else {
        return Err(refuse());
    };
    let declared = declarations.variant_cases(declaration).ok_or_else(refuse)?;
    if actual != declaration || cases.len() != declared.len() {
        return Err(refuse());
    }
    let mut ordered = Vec::new();
    let mut seen = Vec::new();
    for (index, (case, expected)) in cases.iter().zip(declared).enumerate() {
        let fields = declarations.case_fields(&expected.id).ok_or_else(refuse)?;
        if case.case != expected.id
            || case.case_index != index as u32
            || case.fields.len() != fields.len()
        {
            return Err(refuse());
        }
        let mut flags = Vec::new();
        for (index, (field, expected)) in case.fields.iter().zip(fields).enumerate() {
            if field.field != expected.id || field.field_index != index as u32 {
                return Err(refuse());
            }
            match (&field.shape, &expected.ty) {
                (FieldLivenessShape::NoDrop, ty) if crate::hir::is_scalar_resolved_type(ty) => {}
                (FieldLivenessShape::Leaf { flag, lifecycle }, ResolvedType::Bytes)
                    if lifecycle.as_str() == crate::cleanup::BYTES_DROP_LIFECYCLE_ID =>
                {
                    let metadata = function
                        .cleanup
                        .flags
                        .iter()
                        .find(|f| f.id == *flag)
                        .ok_or_else(refuse)?;
                    if seen.contains(flag)
                        || metadata.place.storage != inventory.id
                        || metadata.place.projections != [case.case.clone(), field.field.clone()]
                        || metadata.lifecycle != *lifecycle
                    {
                        return Err(refuse());
                    }
                    seen.push(*flag);
                    flags.push(*flag);
                }
                _ => return Err(refuse()),
            }
        }
        ordered.push((case.case.clone(), flags));
    }
    if function
        .cleanup
        .flags
        .iter()
        .filter(|f| f.place.storage == inventory.id)
        .count()
        != seen.len()
    {
        return Err(refuse());
    }
    let metadata = |flag| {
        let metadata = function
            .cleanup
            .flags
            .iter()
            .find(|f| f.id == flag)
            .expect("validated compiler flag");
        (
            CleanupPlace {
                storage: storage.clone(),
                projections: metadata.place.projections.clone(),
            },
            metadata.lifecycle.clone(),
        )
    };
    if let Some((case, field)) = partial {
        let flags: Vec<_> = function
            .cleanup
            .flags
            .iter()
            .filter(|f| {
                f.place.storage == inventory.id
                    && f.place.projections == [case.clone(), field.clone()]
            })
            .map(|f| f.id)
            .collect();
        if flags.len() != 1 {
            return Err(refuse());
        }
        // After the first field transfer, the constructor's tag has not been
        // sealed. This live leaf is unconditional at its actual temp storage.
        return Ok(crate::cleanup_plan::build::canonical_finalizers_for(
            &flags,
            metadata,
            |_| true,
        ));
    }
    Ok(
        crate::cleanup_plan::build::canonical_conditional_finalizers_for(
            &CleanupPlace {
                storage: storage.clone(),
                projections: Vec::new(),
            },
            declaration,
            &ordered,
            metadata,
            |_| true,
        ),
    )
}
