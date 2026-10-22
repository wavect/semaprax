//! Exact selected structural twin; complete original remains in the parent owner.
use super::*;
pub(in crate::cleanup_plan::replay) fn validate_place_before_formals(
    function: &ResolvedFunction,
    place: &CleanupPlace,
    known: &BTreeSet<StorageId>,
    leaves: &Leaves,
    budget: &mut ReplayBudget,
) -> Result<Vec<LivenessFlagId>, Error> {
    // Actual formal-argument/local slots; existing prepaid backing/cursor
    // headers remain independently charged at their original formation sites.
    type Receiving<'a> = (
        &'a ResolvedFunction,
        &'a CleanupPlace,
        &'a BTreeSet<StorageId>,
        &'a Leaves,
        &'a mut ReplayBudget,
        std::collections::btree_map::Values<'a, LivenessFlagId, super::super::Leaf>,
        Option<&'a super::super::Leaf>,
        &'a super::super::Leaf,
        usize,
        std::collections::btree_map::Iter<'a, LivenessFlagId, super::super::Leaf>,
        Option<(&'a LivenessFlagId, &'a super::super::Leaf)>,
        &'a LivenessFlagId,
        &'a super::super::Leaf,
        Vec<LivenessFlagId>,
        Result<Vec<LivenessFlagId>, Error>,
    );
    budget.reserve_array::<Receiving<'_>>(
        function,
        1,
        "selected validate_place local receiving",
    )?;
    budget.charge(function, 15, "selected validate_place local receiving")?;

    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected control permit call receiving",
        Permit::control_receiver_before_formals::<Vec<LivenessFlagId>>(function, budget)
    )?;
    if !known.contains(&place.storage) {
        return Err(permit.control(Reason::Finite(Kind::CleanupPlaceReferencesUnknownStorage)));
    }
    // Two full flag-order scans preserve the old count-then-publication order.
    // Their concrete iterator, next-result and binding roles are paid above.
    let mut count = 0;
    let mut leaf_values = leaves.values();
    while let Some(leaf) = dynamic_next!(
        budget,
        function,
        leaf_values,
        std::collections::btree_map::Values<'_, LivenessFlagId, super::super::Leaf>,
        &super::super::Leaf,
        (&super::super::Leaf),
        3
    ) {
        if leaf.place.storage == place.storage
            && leaf.place.projections.starts_with(&place.projections)
        {
            count += 1;
        }
    }
    if count == 0 {
        return Err(permit.control(Reason::Finite(Kind::CleanupPlaceHasNoDroppableLeaf)));
    }
    budget.reserve_array::<LivenessFlagId>(function, count, "entry place leaves")?;
    let mut result = Vec::with_capacity(count);
    let mut leaf_entries = leaves.iter();
    while let Some((flag, leaf)) = dynamic_next!(
        budget,
        function,
        leaf_entries,
        std::collections::btree_map::Iter<'_, LivenessFlagId, super::super::Leaf>,
        (&LivenessFlagId, &super::super::Leaf),
        (&LivenessFlagId, &super::super::Leaf),
        4
    ) {
        if leaf.place.storage == place.storage
            && leaf.place.projections.starts_with(&place.projections)
        {
            result.push(*flag);
        }
    }
    Ok(result)
}
