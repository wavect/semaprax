//! Complete selected structural family, same-ledger receiving at every call.
use super::semantic_refusal::Error as _;
use super::{
    flat_records, semantic_refusal, BTreeMap, BTreeSet, Leaves, ReplayBudget, ResolvedFunction,
    StorageId, UNRESOLVED_NORMAL_END,
};
use crate::cleanup_plan::renewal_profile::{
    child_refusal::Refusal, structural_refusal::StructuralRefusal,
};
use crate::cleanup_plan::replay::selected_context::child::receiving::receive;
use crate::cleanup_plan::replay::selected_context::child::{CloneReceiving, NodeReceiving, Permit};
type Error = crate::cleanup_plan::replay::expression_facts::errors::EngineError;

// Lexical replacement of the original u32_index call; no forwarding Result.
macro_rules! index {
    ($function:expr, $permit:ident, $index:expr, $reason:ident) => {
        match u32::try_from($index) {
            Ok(value) => value,
            Err(_) => return Err($permit.refuse(crate::cleanup_plan::renewal_profile::child_refusal::Refusal::Structural(crate::cleanup_plan::renewal_profile::structural_refusal::StructuralRefusal::$reason))),
        }
    };
}
pub(in crate::cleanup_plan::replay) use index;

pub(in crate::cleanup_plan::replay) fn slots(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<BTreeSet<StorageId>, Error> {
    let permit = Permit::profile::<BTreeSet<StorageId>>(function, budget)?;
    // The dynamic Atom initializer and argument receiver are additional to the
    // existing Receipt aggregate. Its shared identity handles allocate no text.
    budget.reserve_array::<(semantic_refusal::Atom, semantic_refusal::Atom)>(
        function,
        1,
        "selected structural semantic atom receiving",
    )?;
    budget.charge(function, 2, "selected structural semantic atom receiving")?;
    let mut storage = BTreeSet::new();
    for (index, slot) in function.cleanup_plan.slots.iter().enumerate() {
        let expected = index!(function, permit, index, TooManySlots);
        if slot.id.0 != expected || slot.storage_index != expected {
            return Err(Error::structural(
                function,
                semantic_refusal::Atom::NoncontiguousSlot(index),
            ));
        }
        if !storage.contains(&slot.storage) {
            receive!(
                function,
                budget,
                permit,
                NodeReceiving<'_>,
                8,
                budget.child_reserve_btree_node::<StorageId, ()>(function, "slot storage index")
            );
            receive!(
                function,
                budget,
                permit,
                CloneReceiving<'_, StorageId>,
                9,
                budget.child_reserve_clone(function, &slot.storage, "slot storage identity")
            );
        }
        if !storage.insert(slot.storage.clone()) {
            return Err(Error::structural(
                function,
                semantic_refusal::Atom::RepeatedStorage(slot.storage.clone()),
            ));
        }
    }
    Ok(storage)
}

pub(in crate::cleanup_plan::replay) fn regions(
    function: &ResolvedFunction,
    storage: &BTreeSet<StorageId>,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    let permit = Permit::profile::<()>(function, budget)?;
    let plan = &function.cleanup_plan;
    if plan.regions.is_empty() {
        return Err(permit.refuse(Refusal::Structural(StructuralRefusal::NoRootRegion)));
    }
    let mut assigned = BTreeSet::new();
    for (index, region) in plan.regions.iter().enumerate() {
        let expected = index!(function, permit, index, TooManyRegions);
        if region.id.0 != expected {
            return Err(permit.refuse(Refusal::Structural(
                StructuralRefusal::RegionIdsNoncontiguous,
            )));
        }
        match (index, region.parent) {
            (0, None) => {}
            (0, Some(_)) | (_, None) => {
                return Err(
                    permit.refuse(Refusal::Structural(StructuralRefusal::InvalidRegionRoot))
                );
            }
            (_, Some(parent)) if usize::try_from(parent.0).ok().is_some_and(|p| p < index) => {}
            _ => {
                return Err(permit.refuse(Refusal::Structural(StructuralRefusal::ParentAfterChild)));
            }
        }
        for item in &region.slots {
            if !storage.contains(item) {
                return Err(
                    permit.refuse(Refusal::Structural(StructuralRefusal::UnknownRegionStorage))
                );
            }
            if !assigned.contains(item) {
                receive!(
                    function,
                    budget,
                    permit,
                    NodeReceiving<'_>,
                    8,
                    budget.child_reserve_btree_node::<StorageId, ()>(
                        function,
                        "region storage index"
                    )
                );
                receive!(
                    function,
                    budget,
                    permit,
                    CloneReceiving<'_, StorageId>,
                    9,
                    budget.child_reserve_clone(function, item, "region storage identity")
                );
            }
            if !assigned.insert(item.clone()) {
                return Err(permit.refuse(Refusal::Structural(
                    StructuralRefusal::RepeatedRegionStorage,
                )));
            }
        }
        let unresolved =
            plan.lexical_control.is_some() && region.normal_scope_end == UNRESOLVED_NORMAL_END;
        if !unresolved
            && usize::try_from(region.normal_scope_end.0)
                .ok()
                .is_none_or(|exit| exit >= plan.exits.len())
        {
            return Err(permit.refuse(Refusal::Structural(StructuralRefusal::UnknownNormalExit)));
        }
    }
    if &assigned != storage {
        return Err(permit.refuse(Refusal::Structural(StructuralRefusal::UnassignedStorage)));
    }
    Ok(())
}

pub(in crate::cleanup_plan::replay) fn leaves(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Leaves, Error> {
    let permit = Permit::profile::<Leaves>(function, budget)?;
    let mut leaves = BTreeMap::new();
    let mut places = BTreeSet::new();
    for slot in &function.cleanup_plan.slots {
        flat_records::paid_leaves::collect_slot_leaves(
            function,
            slot,
            &mut leaves,
            &mut places,
            budget,
        )?;
    }
    for (expected, flag) in leaves.keys().enumerate() {
        let expected = index!(function, permit, expected, TooManyFlags);
        if flag.0 != expected {
            return Err(permit.refuse(Refusal::Structural(StructuralRefusal::NoncontiguousFlags)));
        }
    }
    Leaves::new_with_receiving(function, leaves, budget)
}
