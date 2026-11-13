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

pub(in crate::cleanup_plan::replay) fn collect_slot_leaves<const BYTE_ITERATORS: bool>(
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
            ([], *flag, lifecycle),
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
                        ([&field.field], *flag, lifecycle),
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
        FieldLivenessShape::Variant { declaration, cases }
            if BYTE_ITERATORS
                && crate::iterator_ops::is_step(&slot.ty)
                && crate::iterator_ops::element(&slot.ty)
                    == Some(&crate::hir::ResolvedType::Bytes) =>
        {
            use crate::cleanup::{FieldLiveness, VariantCaseLiveness};
            use crate::hir::CompilerDeclaration as C;
            use crate::iterator_ops as iter;
            type Receiving<'a> = (
                &'a DeclarationId,
                &'a [VariantCaseLiveness],
                &'a VariantCaseLiveness,
                &'a VariantCaseLiveness,
                &'a FieldLiveness,
                &'a FieldLiveness,
                [(&'a FieldLiveness, C, C); 2],
                (&'a FieldLiveness, C, C),
                [&'a DeclarationId; 2],
                (LivenessFlagId, &'a DeclarationId),
                Result<(), Error>,
            );
            budget.reserve_array::<Receiving<'_>>(
                function,
                2,
                "selected iterator leaf receiving",
            )?;
            budget.charge(function, 11, "selected iterator leaf receiving")?;
            let invalid =
                || permit.refuse(Refusal::Structural(StructuralRefusal::ConditionalOwnership));
            let [done, yielded] = cases.as_slice() else {
                return Err(invalid());
            };
            let [item, rest] = yielded.fields.as_slice() else {
                return Err(invalid());
            };
            if declaration.as_str() != iter::STEP_ID
                || done.case.as_str() != iter::DONE_ID
                || done.case_index != 0
                || !done.fields.is_empty()
                || yielded.case.as_str() != iter::YIELD_ID
                || yielded.case_index != 1
                || item.field_index != 0
                || rest.field_index != 1
            {
                return Err(invalid());
            }
            for (field, expected_field, expected_lifecycle) in [
                (item, C::IterItem, C::BytesDrop),
                (rest, C::IterRest, C::IterDrop),
            ] {
                let FieldLivenessShape::Leaf { flag, lifecycle } = &field.shape else {
                    return Err(invalid());
                };
                if field.field.as_str() != expected_field.text()
                    || lifecycle.as_str() != expected_lifecycle.text()
                {
                    return Err(invalid());
                }
                insert_leaf(
                    function,
                    &slot.storage,
                    ([&yielded.case, &field.field], *flag, lifecycle),
                    leaves,
                    places,
                    budget,
                )?;
            }
            Ok(())
        }
        FieldLivenessShape::Variant { .. } => {
            Err(permit.refuse(Refusal::Structural(StructuralRefusal::ConditionalOwnership)))
        }
    }
}

fn insert_leaf<const N: usize>(
    function: &ResolvedFunction,
    storage: &StorageId,
    metadata: ([&DeclarationId; N], LivenessFlagId, &DeclarationId),
    leaves: &mut BTreeMap<LivenessFlagId, Leaf>,
    places: &mut BTreeSet<CleanupPlace>,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    let permit = Permit::profile::<()>(function, budget)?;
    let (path, flag, lifecycle) = metadata;
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
    if N != 0 {
        receive!(
            function,
            budget,
            permit,
            VecReceiving<'_, DeclarationId>,
            10,
            budget.child_reserve_vec(function, &mut projections, N, "selected leaf projection")
        );
        for field in path {
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
