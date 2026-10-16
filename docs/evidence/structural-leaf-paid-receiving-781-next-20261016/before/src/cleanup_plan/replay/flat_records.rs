//! Direct current-declaration record operations for the selected replay only.
//! Every route uses the same borrowed classifier as independent inventory.
use super::*;
use crate::cleanup::{FieldLiveness, FieldLivenessShape};

pub(super) fn admitted(program: &dyn ReplayContext, ty: &ResolvedType) -> Result<bool, Diagnostic> {
    if crate::hir::borrowed_scalar::option_u8::is_type(ty) {
        return Ok(
            crate::hir::borrowed_scalar::option_u8::selected_profile_ready()
                && crate::hir::borrowed_scalar::option_u8::authenticated(
                    program.declarations(),
                    ty,
                ),
        );
    }
    if crate::cleanup::selected_vector_element(program, ty)?.is_some() {
        return Ok(true);
    }
    if matches!(ty, ResolvedType::Nominal { .. }) {
        return Ok(crate::cleanup::selected_flat_record_fields(program, ty)?.is_some());
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
pub(super) fn module(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    for ordinal in 0..program.type_count() {
        let record = program
            .type_at(ordinal)
            .ok_or_else(|| replay_error(function, "selected type inventory is incomplete"))?;
        super::selected_profile::collections::prepay(
            program,
            function,
            record.id.as_str().len(),
            budget,
        )?;
        if record.id.as_str() == crate::prelude::VEC_ID {
            budget.charge(function, 9, "selected Vec declaration header")?;
            crate::cleanup::authenticate_selected_vector(program, &record.id)?;
            continue;
        }
        budget.charge(
            function,
            program
                .type_count()
                .checked_add(9)
                .ok_or_else(|| replay_error(function, "record classification work overflowed"))?,
            "selected direct record declaration",
        )?;
        crate::cleanup::selected_flat_record_declaration(program, &record.id)?;
    }
    Ok(())
}
pub(super) fn expression(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
) -> Result<(), Diagnostic> {
    match &expression.kind {
        ResolvedExprKind::ConstructRecord { record, fields } => {
            let Some((owner, declared)) =
                crate::cleanup::selected_flat_record_fields(program, &expression.ty)?
            else {
                return Err(replay_error(
                    function,
                    "selected construction has no exact direct record",
                ));
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
                return Err(replay_error(
                    function,
                    "selected construction field identity, order, type or ownership disagrees",
                ));
            }
        }
        ResolvedExprKind::Project { base, field } => {
            let Some((_, fields)) = crate::cleanup::selected_flat_record_fields(program, &base.ty)?
            else {
                return Err(replay_error(
                    function,
                    "selected projection has no exact direct record",
                ));
            };
            let declared = fields
                .iter()
                .find(|item| item.id == *field)
                .ok_or_else(|| replay_error(function, "selected projection field is foreign"))?;
            if declared.ty != expression.ty {
                return Err(replay_error(
                    function,
                    "selected projection type disagrees with current field",
                ));
            }
        }
        _ => {}
    }
    Ok(())
}
fn leaf(
    function: &ResolvedFunction,
    ty: &ResolvedType,
    next: &mut u32,
    budget: &mut ReplayBudget,
) -> Result<FieldLivenessShape, Diagnostic> {
    if !crate::cleanup::selected_inventory_needs_drop(ty)? {
        return Ok(FieldLivenessShape::NoDrop);
    }
    let lifecycle = crate::cleanup::primitive_leaf_lifecycle_identity(ty)
        .ok_or_else(|| replay_error(function, "selected field has no primitive lifecycle"))?;
    lifecycle_leaf(function, lifecycle, next, budget)
}
/// Lifecycles are fixed compiler identities and allocate no backing. The
/// original text term is still reserved through the replay budget's own
/// route, unchanged and whatever that budget's payer is, so caps and the
/// first-refusal order match the owned-text replay. It is conservative and
/// pays for no identity allocation.
fn lifecycle_leaf(
    function: &ResolvedFunction,
    lifecycle: crate::hir::CompilerDeclaration,
    next: &mut u32,
    budget: &mut ReplayBudget,
) -> Result<FieldLivenessShape, Diagnostic> {
    if *next as usize >= crate::cleanup::MAX_CLEANUP_OWNED_LEAVES {
        return Err(replay_error(
            function,
            "selected field flags exceed the owned-leaf limit",
        ));
    }
    let flag = LivenessFlagId(*next);
    *next = next
        .checked_add(1)
        .ok_or_else(|| replay_error(function, "selected field flags overflowed"))?;
    budget.reserve_bytes(function, lifecycle.text().len(), "selected field lifecycle")?;
    Ok(FieldLivenessShape::Leaf {
        flag,
        lifecycle: DeclarationId::compiler(lifecycle),
    })
}
pub(super) fn shape(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    ty: &ResolvedType,
    next: &mut u32,
    budget: &mut ReplayBudget,
) -> Result<FieldLivenessShape, Diagnostic> {
    if matches!(ty, ResolvedType::Nominal { .. }) {
        super::selected_profile::collections::prepay(
            program,
            function,
            super::selected_profile::collections::type_key_bytes(ty)?,
            budget,
        )?;
    }
    if crate::cleanup::selected_vector_element(program, ty)?.is_some() {
        return lifecycle_leaf(
            function,
            crate::hir::CompilerDeclaration::VecDrop,
            next,
            budget,
        );
    }
    if let Some((owner, fields)) = crate::cleanup::selected_flat_record_fields(program, ty)? {
        if !fields
            .iter()
            .any(|field| matches!(field.ty, ResolvedType::String | ResolvedType::Bytes))
        {
            return Ok(FieldLivenessShape::NoDrop);
        }
        let mut shapes = Vec::new();
        budget.reserve_vec(
            function,
            &mut shapes,
            fields.len(),
            "selected whole record fields",
        )?;
        for field in fields {
            budget.reserve_clone(function, &field.id, "selected record field identity")?;
            shapes.push(FieldLiveness {
                field: field.id.clone(),
                field_index: field.index,
                shape: leaf(function, &field.ty, next, budget)?,
            });
        }
        budget.reserve_clone(function, owner, "selected record declaration identity")?;
        return Ok(FieldLivenessShape::Record {
            declaration: owner.clone(),
            fields: shapes,
        });
    }
    leaf(function, ty, next, budget)
}

pub(super) fn collect_slot_leaves(
    function: &ResolvedFunction,
    slot: &super::super::CleanupSlot,
    leaves: &mut BTreeMap<LivenessFlagId, Leaf>,
    places: &mut BTreeSet<CleanupPlace>,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    match &slot.field_liveness_shape {
        FieldLivenessShape::NoDrop => Ok(()),
        FieldLivenessShape::Leaf { flag, lifecycle } => insert_leaf(
            function,
            &slot.storage,
            (None, *flag, lifecycle),
            leaves,
            places,
            budget,
        ),
        FieldLivenessShape::Record { fields, .. } => {
            if !(1..=8).contains(&fields.len()) {
                return Err(replay_error(
                    function,
                    "selected record leaf count is outside direct shape",
                ));
            }
            for (index, field) in fields.iter().enumerate() {
                if field.field_index as usize != index {
                    return Err(replay_error(
                        function,
                        "selected record field index is noncanonical",
                    ));
                }
                match &field.shape {
                    FieldLivenessShape::NoDrop => {}
                    FieldLivenessShape::Leaf { flag, lifecycle } => insert_leaf(
                        function,
                        &slot.storage,
                        (Some(&field.field), *flag, lifecycle),
                        leaves,
                        places,
                        budget,
                    )?,
                    _ => {
                        return Err(replay_error(
                            function,
                            "selected record has a nested or conditional leaf",
                        ));
                    }
                }
            }
            Ok(())
        }
        FieldLivenessShape::Variant { .. } => Err(replay_error(
            function,
            "selected record has conditional ownership",
        )),
    }
}
fn insert_leaf(
    function: &ResolvedFunction,
    storage: &StorageId,
    metadata: (Option<&DeclarationId>, LivenessFlagId, &DeclarationId),
    leaves: &mut BTreeMap<LivenessFlagId, Leaf>,
    places: &mut BTreeSet<CleanupPlace>,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let (field, flag, lifecycle) = metadata;
    if leaves.contains_key(&flag) {
        return Err(replay_error(function, "selected liveness flag is repeated"));
    }
    budget.reserve_btree_node::<LivenessFlagId, Leaf>(function, "selected leaf index")?;
    budget.reserve_clone(function, storage, "selected leaf storage")?;
    budget.reserve_clone(function, lifecycle, "selected leaf lifecycle")?;
    let mut projections = Vec::new();
    if let Some(field) = field {
        budget.reserve_vec(function, &mut projections, 1, "selected leaf projection")?;
        budget.reserve_clone(function, field, "selected leaf projection identity")?;
        projections.push(field.clone());
    }
    let place = CleanupPlace {
        storage: storage.clone(),
        projections,
    };
    if places.contains(&place) {
        return Err(replay_error(
            function,
            "selected cleanup leaf place is repeated",
        ));
    }
    budget.reserve_clone(function, &place, "selected leaf place index copy")?;
    budget.reserve_btree_node::<CleanupPlace, ()>(function, "selected leaf place index")?;
    let copy = place.clone();
    leaves.insert(
        flag,
        Leaf {
            place,
            lifecycle: lifecycle.clone(),
        },
    );
    places.insert(copy);
    Ok(())
}

#[cfg(test)]
mod tests;
