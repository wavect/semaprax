//! Exact selected structural twin; complete original remains in the parent owner.
use super::*;
pub(super) fn validate_entry_before_dynamic(
    function: &ResolvedFunction,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    // Actual formal-argument/local slots; existing prepaid backing/cursor
    // headers remain independently charged at their original formation sites.
    type Receiving<'a> = (
        &'a ResolvedFunction,
        &'a BTreeSet<StorageId>,
        &'a Leaves,
        &'a mut ReplayBudget,
        BTreeSet<LivenessFlagId>,
        &'a CleanupPlace,
        LivenessFlagId,
        std::vec::IntoIter<LivenessFlagId>,
        std::slice::Iter<'a, CleanupPlace>,
    );
    budget.reserve_array::<Receiving<'_>>(
        function,
        1,
        "selected validate_entry local receiving",
    )?;
    budget.charge(function, 9, "selected validate_entry local receiving")?;

    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected control permit call receiving",
        Permit::control_receiver::<()>(function, budget)
    )?;
    if function
        .cleanup_plan
        .blocks
        .get(function.cleanup_plan.entry.0 as usize)
        .is_none()
    {
        return Err(permit.control(Reason::Finite(Kind::CleanupEntryBlockIsUnknown)));
    }
    let mut flags = BTreeSet::new();
    for place in &function.cleanup_plan.entry_state.live_owned_parameters {
        for flag in crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
            budget,
            function,
            (
                &ResolvedFunction,
                &CleanupPlace,
                &BTreeSet<StorageId>,
                &Leaves,
                &mut ReplayBudget,
                Result<Vec<LivenessFlagId>, Error>
            ),
            6,
            "selected control validate_place call receiving",
            helpers::validate_place(function, place, storage, leaves, budget)
        )? {
            if !insert_copy(function, budget, &mut flags, flag, "structural entry flags")? {
                return Err(permit.control(Reason::Finite(
                    Kind::CleanupEntryPlacesOverlapOrRepeatLivenessFlags,
                )));
            }
        }
    }
    Ok(())
}
