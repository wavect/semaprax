//! Exact selected structural twin; complete original remains in the parent owner.
use super::*;
pub(in crate::cleanup_plan::replay) fn validate_entry(
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

    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected control permit call receiving",
        Permit::control_receiver::<()>(function, budget)
    )?;
    if dynamic_get!(
        budget,
        function,
        function.cleanup_plan.blocks,
        &[crate::cleanup_plan::CleanupBlock],
        function.cleanup_plan.entry.0 as usize,
        usize,
        Option<&crate::cleanup_plan::CleanupBlock>,
        (),
        3
    )
    .is_none()
    {
        return Err(permit.control(Reason::Finite(Kind::CleanupEntryBlockIsUnknown)));
    }
    let mut flags = BTreeSet::new();
    let mut place_entries = function
        .cleanup_plan
        .entry_state
        .live_owned_parameters
        .iter();
    while let Some(place) = dynamic_next!(
        budget,
        function,
        place_entries,
        std::slice::Iter<'_, CleanupPlace>,
        &CleanupPlace,
        (&CleanupPlace),
        3
    ) {
        let mut flag_entries = crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
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
        )?
        .into_iter();
        while let Some(flag) = dynamic_next!(
            budget,
            function,
            flag_entries,
            std::vec::IntoIter<LivenessFlagId>,
            LivenessFlagId,
            (LivenessFlagId),
            3
        ) {
            if !insert_copy(function, budget, &mut flags, flag, "structural entry flags")? {
                return Err(permit.control(Reason::Finite(
                    Kind::CleanupEntryPlacesOverlapOrRepeatLivenessFlags,
                )));
            }
        }
    }
    Ok(())
}
