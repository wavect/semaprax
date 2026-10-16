//! Complete selected attached-leaf receiver; legacy producer remains separate.
use super::{
    BTreeMap, BTreeSet, CleanupPlace, DeclarationId, FieldLivenessShape, Leaf, LivenessFlagId,
    ReplayBudget, ResolvedFunction, StorageId,
};
use crate::cleanup_plan::renewal_profile::{
    child_refusal::Refusal, structural_refusal::StructuralRefusal,
};
use crate::cleanup_plan::replay::selected_context::child::receiving::receive;
use crate::cleanup_plan::replay::selected_context::child::{
    CloneReceiving, NodeReceiving, Permit, VecReceiving,
};
type Error = crate::cleanup_plan::replay::expression_facts::errors::EngineError;

pub(in crate::cleanup_plan::replay) fn collect_slot_leaves(
    function: &ResolvedFunction,
    slot: &crate::cleanup_plan::CleanupSlot,
    leaves: &mut BTreeMap<LivenessFlagId, Leaf>,
    places: &mut BTreeSet<CleanupPlace>,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    let permit = Permit::profile::<()>(function, budget)?;
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
                return Err(permit.refuse(Refusal::Structural(StructuralRefusal::LeafCount)));
            }
            for (index, field) in fields.iter().enumerate() {
                if field.field_index as usize != index {
                    return Err(permit.refuse(Refusal::Structural(StructuralRefusal::FieldIndex)));
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
                        return Err(
                            permit.refuse(Refusal::Structural(StructuralRefusal::NestedLeaf))
                        );
                    }
                }
            }
            Ok(())
        }
        FieldLivenessShape::Variant { .. } => {
            Err(permit.refuse(Refusal::Structural(StructuralRefusal::ConditionalOwnership)))
        }
    }
}

fn insert_leaf(
    function: &ResolvedFunction,
    storage: &StorageId,
    metadata: (Option<&DeclarationId>, LivenessFlagId, &DeclarationId),
    leaves: &mut BTreeMap<LivenessFlagId, Leaf>,
    places: &mut BTreeSet<CleanupPlace>,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    let permit = Permit::profile::<()>(function, budget)?;
    let (field, flag, lifecycle) = metadata;
    if leaves.contains_key(&flag) {
        return Err(permit.refuse(Refusal::Structural(StructuralRefusal::RepeatedFlag)));
    }
    receive!(
        function,
        budget,
        permit,
        NodeReceiving<'_>,
        8,
        budget.child_reserve_btree_node::<LivenessFlagId, Leaf>(function, "selected leaf index")
    );
    receive!(
        function,
        budget,
        permit,
        CloneReceiving<'_, StorageId>,
        9,
        budget.child_reserve_clone(function, storage, "selected leaf storage")
    );
    receive!(
        function,
        budget,
        permit,
        CloneReceiving<'_, DeclarationId>,
        9,
        budget.child_reserve_clone(function, lifecycle, "selected leaf lifecycle")
    );
    let mut projections = Vec::new();
    if let Some(field) = field {
        receive!(
            function,
            budget,
            permit,
            VecReceiving<'_, DeclarationId>,
            10,
            budget.child_reserve_vec(function, &mut projections, 1, "selected leaf projection")
        );
        receive!(
            function,
            budget,
            permit,
            CloneReceiving<'_, DeclarationId>,
            9,
            budget.child_reserve_clone(function, field, "selected leaf projection identity")
        );
        projections.push(field.clone());
    }
    let place = CleanupPlace {
        storage: storage.clone(),
        projections,
    };
    if places.contains(&place) {
        return Err(permit.refuse(Refusal::Structural(StructuralRefusal::RepeatedPlace)));
    }
    receive!(
        function,
        budget,
        permit,
        CloneReceiving<'_, CleanupPlace>,
        9,
        budget.child_reserve_clone(function, &place, "selected leaf place index copy")
    );
    receive!(
        function,
        budget,
        permit,
        NodeReceiving<'_>,
        8,
        budget.child_reserve_btree_node::<CleanupPlace, ()>(function, "selected leaf place index")
    );
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
