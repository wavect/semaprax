//! Exact selected path twin; complete immediate old owners remain independent.
use super::*;
pub(super) fn validate_control_group_with_budget(
    function: &ResolvedFunction,
    states: &BTreeSet<Box<PathState>>,
    block: BlockId,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    // Actual body/iterator/capture roles; original nested backing and
    // explicit insertion/payload receiving banks remain separately intact.
    budget.reserve_array::<(
        &Box<PathState>,
        std::collections::btree_set::Iter<'_, Box<PathState>>,
        std::collections::btree_set::Range<'_, Box<PathState>>,
    )>(
        function,
        1,
        "selected path validate_control_group_with_budget body receiving",
    )?;
    budget.charge(
        function,
        3,
        "selected path validate_control_group_with_budget body receiving",
    )?;
    budget.reserve_array::<(
        &ResolvedFunction,
        &BTreeSet<Box<PathState>>,
        BlockId,
        &mut ReplayBudget,
        Result<(), Error>,
    )>(
        function,
        1,
        "selected path validate_control_group_with_budget formal receiving",
    )?;
    budget.charge(
        function,
        5,
        "selected path validate_control_group_with_budget formal receiving",
    )?;

    let mut group_state_entries = states.iter();
    while let Some(state) = dynamic_next!(
        budget,
        function,
        group_state_entries,
        receiving::GroupStateEntriesNext<'_>,
        3
    ) {
        path_child!(
            budget,
            function,
            ValidateJoinCompatibilityCall,
            6,
            renewal::validate_join_compatibility(
                function,
                states.range::<Box<PathState>, _>(..state),
                state.as_ref(),
                block,
                budget,
            )
        )?;
    }
    Ok(())
}

pub(super) fn joined_ownership_state(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    states: &BTreeSet<Box<PathState>>,
    budget: &mut ReplayBudget,
) -> Result<(Vec<LivenessFlagId>, Vec<ReplayConditionalVariant>), Error> {
    // Actual body/iterator/capture roles; original nested backing and
    // explicit insertion/payload receiving banks remain separately intact.
    budget.reserve_array::<(
        std::collections::btree_set::Iter<'_, Box<PathState>>,
        &Box<PathState>,
        Vec<LivenessFlagId>,
        Vec<ReplayConditionalVariant>,
        &Box<PathState>,
    )>(
        function,
        1,
        "selected path joined_ownership_state body receiving",
    )?;
    budget.charge(
        function,
        5,
        "selected path joined_ownership_state body receiving",
    )?;
    budget.reserve_array::<(
        &dyn crate::cleanup::ReplayContext,
        &ResolvedFunction,
        &BTreeSet<Box<PathState>>,
        &mut ReplayBudget,
        Result<(Vec<LivenessFlagId>, Vec<ReplayConditionalVariant>), Error>,
    )>(
        function,
        1,
        "selected path joined_ownership_state formal receiving",
    )?;
    budget.charge(
        function,
        5,
        "selected path joined_ownership_state formal receiving",
    )?;
    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected path permit call receiving",
        Permit::control_receiver::<(Vec<LivenessFlagId>, Vec<ReplayConditionalVariant>)>(
            function, budget
        )
    )?;
    let mut states = states.iter();
    let first = dynamic_next!(
        budget,
        function,
        states,
        receiving::GroupStateEntriesNext<'_>,
        3
    )
    .ok_or_else(path_refusal!(
        budget,
        function,
        permit,
        &Box<PathState>,
        Kind::CleanupJoinHasNoIncomingStates
    ))?;
    retained_child!(
        budget,
        function,
        OwnershipClone,
        4,
        storage::reserve_ownership_clone(function, first, budget)
    )?;
    let mut live_order = clone_value!(budget, function, first.live_order, Vec<LivenessFlagId>);
    let mut conditional_variants = clone_value!(
        budget,
        function,
        first.conditional_variants,
        Vec<ReplayConditionalVariant>
    );
    let mut group_state_entries = states;
    while let Some(state) = dynamic_next!(
        budget,
        function,
        group_state_entries,
        receiving::GroupStateEntriesNext<'_>,
        3
    ) {
        live_order = path_child!(
            budget,
            function,
            JoinLiveOrdersCall,
            5,
            join_live_orders(function, &live_order, &state.live_order, budget)
        )?;
        conditional_variants = retained_child!(
            budget,
            function,
            ConditionalJoin,
            6,
            super::super::join_conditional_variants(
                program,
                function,
                &conditional_variants,
                &state.conditional_variants,
                budget,
            )
        )?;
    }
    Ok((live_order, conditional_variants))
}

fn join_live_orders(
    function: &ResolvedFunction,
    left: &[LivenessFlagId],
    right: &[LivenessFlagId],
    budget: &mut ReplayBudget,
) -> Result<Vec<LivenessFlagId>, Error> {
    // Actual body/iterator/capture roles; original nested backing and
    // explicit insertion/payload receiving banks remain separately intact.
    budget.reserve_array::<(
        BTreeSet<LivenessFlagId>,
        LivenessFlagId,
        std::iter::Copied<
            std::iter::Chain<
                std::slice::Iter<'_, LivenessFlagId>,
                std::slice::Iter<'_, LivenessFlagId>,
            >,
        >,
        BTreeMap<LivenessFlagId, BTreeSet<LivenessFlagId>>,
        BTreeMap<LivenessFlagId, u32>,
        &LivenessFlagId,
        &[LivenessFlagId],
        &[LivenessFlagId],
        std::slice::Windows<'_, LivenessFlagId>,
        BTreeSet<LivenessFlagId>,
        &u32,
        Vec<LivenessFlagId>,
        Option<LivenessFlagId>,
        LivenessFlagId,
        LivenessFlagId,
        &mut u32,
    )>(function, 1, "selected path join_live_orders body receiving")?;
    budget.charge(
        function,
        16,
        "selected path join_live_orders body receiving",
    )?;
    budget.reserve_array::<(
        &ResolvedFunction,
        &[LivenessFlagId],
        &[LivenessFlagId],
        &mut ReplayBudget,
        Result<Vec<LivenessFlagId>, Error>,
    )>(
        function,
        1,
        "selected path join_live_orders formal receiving",
    )?;
    budget.charge(
        function,
        5,
        "selected path join_live_orders formal receiving",
    )?;
    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected path permit call receiving",
        Permit::control_receiver::<Vec<LivenessFlagId>>(function, budget)
    )?;
    let mut flags = BTreeSet::new();
    receive!(
        budget,
        function,
        receiving::JoinedFlags<'_>,
        1,
        "selected joined flag iterator receiving",
        ()
    );
    let mut joined_flags = left.iter().chain(right).copied();
    while let Some(flag) = dynamic_next!(
        budget,
        function,
        joined_flags,
        receiving::JoinedFlagsNext<'_>,
        3
    ) {
        retained_insert!(
            budget,
            function,
            FlagSetInsert,
            6,
            storage::insert_set(function, &mut flags, flag, budget, "joined live flags")
        )?;
    }
    let mut successors = BTreeMap::new();
    let mut indegree = BTreeMap::new();
    let mut union_flag_entries = flags.iter();
    while let Some(flag) = dynamic_next!(
        budget,
        function,
        union_flag_entries,
        receiving::UnionFlagEntriesNext<'_>,
        3
    ) {
        retained_insert!(
            budget,
            function,
            SuccessorMapInsert,
            7,
            storage::insert_map(
                function,
                &mut successors,
                *flag,
                BTreeSet::new(),
                budget,
                "join successor lookup",
            )
        )?;
        retained_insert!(
            budget,
            function,
            DegreeMapInsert,
            7,
            storage::insert_map(
                function,
                &mut indegree,
                *flag,
                0u32,
                budget,
                "join indegrees",
            )
        )?;
    }
    let mut join_histories = [left, right].into_iter();
    while let Some(history) = dynamic_next!(
        budget,
        function,
        join_histories,
        receiving::JoinHistoriesNext<'_>,
        3
    ) {
        let mut join_pairs = history.windows(2);
        while let Some(pair) = dynamic_next!(
            budget,
            function,
            join_pairs,
            receiving::JoinPairsNext<'_>,
            3
        ) {
            if retained_insert!(
                budget,
                function,
                FlagSetInsert,
                6,
                storage::insert_set(
                    function,
                    mutable_get!(
                        budget,
                        function,
                        successors,
                        slice_index!(budget, function, pair, 0, LivenessFlagId),
                        LivenessFlagId,
                        BTreeSet<LivenessFlagId>
                    )
                    .expect("joined replay flag is indexed"),
                    pair_at!(budget, function, pair, 1),
                    budget,
                    "join successor edge",
                )
            )? {
                *mutable_get!(
                    budget,
                    function,
                    indegree,
                    slice_index!(budget, function, pair, 1, LivenessFlagId),
                    LivenessFlagId,
                    u32
                )
                .expect("joined replay flag is indexed") += 1;
            }
        }
    }
    let mut ready = BTreeSet::new();
    let mut join_degrees = indegree.iter();
    while let Some((flag, degree)) = dynamic_next!(
        budget,
        function,
        join_degrees,
        receiving::JoinDegreesNext<'_>,
        4
    ) {
        if *degree == 0 {
            retained_insert!(
                budget,
                function,
                FlagSetInsert,
                6,
                storage::insert_set(function, &mut ready, *flag, budget, "join ready flags")
            )?;
        }
    }
    budget.reserve_array::<LivenessFlagId>(function, flags.len(), "joined live history")?;
    let mut joined = Vec::with_capacity(flags.len());
    while let Some(flag) = receive!(
        budget,
        function,
        (
            &mut BTreeSet<LivenessFlagId>,
            Option<LivenessFlagId>,
            LivenessFlagId
        ),
        3,
        "selected path join ready pop receiving",
        ready.pop_first()
    ) {
        joined.push(flag);
        let mut join_successors = map_index!(
            budget,
            function,
            successors,
            &flag,
            LivenessFlagId,
            BTreeSet<LivenessFlagId>
        )
        .iter();
        while let Some(successor) = dynamic_next!(
            budget,
            function,
            join_successors,
            receiving::JoinSuccessorsNext<'_>,
            3
        ) {
            let degree = mutable_get!(budget, function, indegree, successor, LivenessFlagId, u32)
                .expect("joined replay flag is indexed");
            *degree -= 1;
            if *degree == 0 {
                retained_insert!(
                    budget,
                    function,
                    FlagSetInsert,
                    6,
                    storage::insert_set(
                        function,
                        &mut ready,
                        *successor,
                        budget,
                        "join ready flags"
                    )
                )?;
            }
        }
    }
    if joined.len() != flags.len() {
        return Err(path_error!(
            permit,
            Reason::Finite(Kind::CleanupJoinHasConflictingInitializationHistories,)
        ));
    }
    Ok(joined)
}
