//! Exact selected path twin; complete immediate old owners remain independent.
use super::*;
pub(super) fn append_flags_before_formals(
    function: &ResolvedFunction,
    state: &mut PathState,
    flags: Vec<LivenessFlagId>,
    operation: Kind,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    // Actual persistent body roles; original nested backing and
    // explicit insertion/payload receiving banks remain separately intact.
    budget.reserve_array::<(
        &LivenessFlagId,
        &ReplayConditionalVariant,
        &Vec<LivenessFlagId>,
        std::slice::Iter<'_, LivenessFlagId>,
        std::slice::Iter<'_, ReplayConditionalVariant>,
    )>(function, 1, "selected path append_flags body receiving")?;
    budget.charge(function, 5, "selected path append_flags body receiving")?;
    budget.reserve_array::<(
        &ResolvedFunction,
        &mut PathState,
        Vec<LivenessFlagId>,
        Kind,
        &mut ReplayBudget,
        Result<(), Error>,
    )>(function, 1, "selected path append_flags formal receiving")?;
    budget.charge(function, 6, "selected path append_flags formal receiving")?;
    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected path permit call receiving",
        Permit::control_receiver_before_formals::<()>(function, budget)
    )?;
    if flags.is_empty()
        || flag_any!(
            budget,
            function,
            flags,
            flag,
            state.live_order.contains(flag)
                || variant_any!(
                    budget,
                    function,
                    state.conditional_variants,
                    variant,
                    scan!(
                        budget,
                        function,
                        variant.cases.iter(),
                        std::slice::Iter<'_, (DeclarationId, Vec<LivenessFlagId>)>,
                        &(DeclarationId, Vec<LivenessFlagId>),
                        (_, case_flags),
                        (&Vec<LivenessFlagId>),
                        3,
                        case_flags.contains(flag),
                        false,
                        true
                    )
                )
        )
    {
        return Err(path_error!(permit, Reason::Finite(operation)));
    }
    receive!(
        budget,
        function,
        std::vec::IntoIter<LivenessFlagId>,
        1,
        "selected appended flag publication iterator receiving",
        ()
    );
    let mut published = flags.into_iter();
    while let Some(flag) = dynamic_next!(
        budget,
        function,
        published,
        std::vec::IntoIter<LivenessFlagId>,
        LivenessFlagId,
        (LivenessFlagId),
        3
    ) {
        state.live_order.push(flag);
    }
    Ok(())
}
