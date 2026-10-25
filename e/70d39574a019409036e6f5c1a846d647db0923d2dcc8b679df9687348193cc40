//! Exact selected path twin; complete immediate old owners remain independent.
use super::*;
pub(in crate::cleanup_plan::replay) fn reserve_append(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    at: &ExpressionId,
    source: &CleanupPlace,
    destination: &CleanupPlace,
    state: &mut PathState,
    budget: &mut ReplayBudget,
) -> Result<Option<Vec<LivenessFlagId>>, Error> {
    // Actual persistent body roles; original nested backing and
    // explicit insertion/payload receiving banks remain separately intact.
    budget.reserve_array::<(
        &crate::cleanup_plan::ReplayRenewalBindings<'_>,
        &BTreeMap<ExpressionId, ExpressionId>,
        &CleanupPlace,
        &ExpressionId,
        std::collections::btree_map::Iter<'_, ExpressionId, ExpressionId>,
        crate::cleanup_plan::renewal_history::RenewalHistory,
        Vec<LivenessFlagId>,
        Option<crate::cleanup_plan::renewal_history::RenewalHistory>,
    )>(function, 1, "selected path reserve_append body receiving")?;
    budget.charge(function, 8, "selected path reserve_append body receiving")?;
    budget.reserve_array::<(
        &dyn ReplayContext,
        &ResolvedFunction,
        &ExpressionId,
        &CleanupPlace,
        &CleanupPlace,
        &mut PathState,
        &mut ReplayBudget,
        Result<Option<Vec<LivenessFlagId>>, Error>,
    )>(function, 1, "selected path reserve_append formal receiving")?;
    budget.charge(function, 8, "selected path reserve_append formal receiving")?;
    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected path permit call receiving",
        Permit::control_receiver::<Option<Vec<LivenessFlagId>>>(function, budget)
    )?;
    let sites = selected_renewals!(budget, function, program).ok_or_else(path_refusal!(
        budget,
        function,
        permit,
        &crate::cleanup_plan::ReplayRenewalBindings<'_>,
        Kind::SelectedAppendLacksCurrentFunctionCensus
    ))?;
    let appends = sites.appends();
    receive!(
        budget,
        function,
        (usize, Option<usize>, usize),
        3,
        "selected append arithmetic receiving",
        ()
    );
    let lookup_work = (appends.len() + 1)
        .checked_mul(at.as_str().len() + 1)
        .and_then(|n| n.checked_mul(2))
        .ok_or_else(path_refusal!(
            budget,
            function,
            permit,
            usize,
            Kind::SelectedAppendLookupOverflow
        ))?;
    budget.charge(
        function,
        lookup_work,
        "selected append exact current lookup",
    )?;
    if matches!(source.storage, StorageId::Value(_))
        && source.projections.is_empty()
        && appends.contains_key(at)
    {
        if state.renewals.contains_key(at) {
            return Err(path_error!(
                permit,
                Reason::Finite(Kind::StringAppendReservesItsOwnerTwice)
            ));
        }
        budget.reserve_btree_node::<ExpressionId, crate::cleanup_plan::renewal_history::RenewalHistory>(
            function, "selected append history node")?;
        budget.reserve_clone(function, &state.live_order, "selected append live history")?;
        map_insert!(
            budget,
            function,
            state.renewals,
            clone_value!(budget, function, at, ExpressionId),
            receive!(
                budget,
                function,
                (
                    &ResolvedFunction,
                    Vec<LivenessFlagId>,
                    Option<LivenessFlagId>,
                    &mut ReplayBudget,
                    Result<crate::cleanup_plan::renewal_history::RenewalHistory, Error>
                ),
                5,
                "selected renewal new caller receiving",
                history::new(
                    function,
                    clone_value!(budget, function, state.live_order, Vec<LivenessFlagId>),
                    None,
                    budget
                )
            )?,
            ExpressionId,
            crate::cleanup_plan::renewal_history::RenewalHistory
        );
        return Ok(None);
    }
    if matches!(destination.storage, StorageId::Value(_)) && destination.projections.is_empty() {
        // The old find visits in key order and stops at its first match.
        receive!(
            budget,
            function,
            Option<(&ExpressionId, &ExpressionId)>,
            1,
            "selected append find result receiving",
            ()
        );
        let mut found = None;
        let mut append_entries = appends.iter();
        while let Some((operand, value)) = dynamic_next!(
            budget,
            function,
            append_entries,
            std::collections::btree_map::Iter<'_, ExpressionId, ExpressionId>,
            (&ExpressionId, &ExpressionId),
            (&ExpressionId, &ExpressionId),
            4
        ) {
            if value == at {
                found = Some((operand, value));
                break;
            }
        }
        if let Some((operand, _)) = found {
            return match receive!(
                budget,
                function,
                (
                    &mut BTreeMap<
                        ExpressionId,
                        crate::cleanup_plan::renewal_history::RenewalHistory,
                    >,
                    &ExpressionId,
                    Option<crate::cleanup_plan::renewal_history::RenewalHistory>,
                    crate::cleanup_plan::renewal_history::RenewalHistory
                ),
                4,
                "selected append removal receiving",
                state.renewals.remove(operand)
            ) {
                Some(history) => Ok(Some(history.history)),
                None => Err(path_error!(
                    permit,
                    Reason::Finite(Kind::StringAppendPublishesWithoutAReservedOwner,)
                )),
            };
        }
    }
    Ok(None)
}
pub(in crate::cleanup_plan::replay) fn publish_append(
    function: &ResolvedFunction,
    history: Option<Vec<LivenessFlagId>>,
    state: &mut PathState,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    // Actual persistent body roles; original nested backing and
    // explicit insertion/payload receiving banks remain separately intact.
    budget.reserve_array::<(
        Vec<LivenessFlagId>,
        usize,
        usize,
        Vec<LivenessFlagId>,
        &Vec<LivenessFlagId>,
        std::slice::Iter<'_, LivenessFlagId>,
        std::slice::Iter<'_, LivenessFlagId>,
    )>(function, 1, "selected path publish_append body receiving")?;
    budget.charge(function, 7, "selected path publish_append body receiving")?;
    budget.reserve_array::<(
        &ResolvedFunction,
        Option<Vec<LivenessFlagId>>,
        &mut PathState,
        &mut ReplayBudget,
        Result<(), Error>,
    )>(function, 1, "selected path publish_append formal receiving")?;
    budget.charge(function, 5, "selected path publish_append formal receiving")?;
    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected path permit call receiving",
        Permit::control_receiver::<()>(function, budget)
    )?;
    if let Some(history) = history {
        let capacity = history
            .len()
            .checked_add(state.live_order.len())
            .ok_or_else(path_refusal!(
                budget,
                function,
                permit,
                usize,
                Kind::AppendPublicationLengthOverflow
            ))?;
        let publication_work = capacity
            .checked_mul(capacity + 1)
            .ok_or_else(path_refusal!(
                budget,
                function,
                permit,
                usize,
                Kind::AppendPublicationWorkOverflow
            ))?;
        budget.charge(
            function,
            publication_work,
            "selected append publication exact history",
        )?;
        budget.reserve_array::<LivenessFlagId>(
            function,
            capacity,
            "selected append publication backing",
        )?;
        let mut ordered = Vec::with_capacity(capacity);
        let mut history_entries = history.iter();
        while let Some(flag) = dynamic_next!(
            budget,
            function,
            history_entries,
            std::slice::Iter<'_, LivenessFlagId>,
            &LivenessFlagId,
            (&LivenessFlagId),
            3
        ) {
            if state.live_order.contains(flag) {
                ordered.push(*flag);
            }
        }
        let mut live_entries = state.live_order.iter();
        while let Some(flag) = dynamic_next!(
            budget,
            function,
            live_entries,
            std::slice::Iter<'_, LivenessFlagId>,
            &LivenessFlagId,
            (&LivenessFlagId),
            3
        ) {
            if !history.contains(flag) {
                ordered.push(*flag);
            }
        }
        state.live_order = ordered;
    }
    Ok(())
}
