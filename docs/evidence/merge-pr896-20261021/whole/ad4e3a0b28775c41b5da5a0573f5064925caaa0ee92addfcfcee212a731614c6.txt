//! Exact selected path twin; complete immediate old owners remain independent.
use super::*;
pub(super) fn validate_binding_selected(
    function: &ResolvedFunction,
    at: &ExpressionId,
    place: &CleanupPlace,
    sites: &crate::cleanup_plan::ReplayRenewalBindings<'_>,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    // Actual persistent body roles; original nested backing and
    // explicit insertion/payload receiving banks remain separately intact.
    budget.reserve_array::<(&crate::hir::ResolvedBinding, &crate::hir::ValueId)>(
        function,
        1,
        "selected path validate_binding_selected body receiving",
    )?;
    budget.charge(
        function,
        2,
        "selected path validate_binding_selected body receiving",
    )?;
    budget.reserve_array::<(
        &ResolvedFunction,
        &ExpressionId,
        &CleanupPlace,
        &crate::cleanup_plan::ReplayRenewalBindings<'_>,
        &mut ReplayBudget,
        Result<(), Error>,
    )>(
        function,
        1,
        "selected path validate_binding_selected formal receiving",
    )?;
    budget.charge(
        function,
        6,
        "selected path validate_binding_selected formal receiving",
    )?;
    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected path permit call receiving",
        Permit::control_receiver::<()>(function, budget)
    )?;
    if !matches!(
        function.cleanup_plan.schema,
        CLEANUP_PLAN_SCHEMA_V18 | CLEANUP_PLAN_SCHEMA_V19
    ) || sites.is_helper(at) && function.cleanup_plan.schema != CLEANUP_PLAN_SCHEMA_V19
        || !receive!(
            budget,
            function,
            (
                &crate::cleanup_plan::ReplayRenewalBindings<'_>,
                &ExpressionId,
                Option<&crate::hir::ResolvedBinding>,
                &crate::hir::ResolvedBinding,
                bool
            ),
            5,
            "selected renewal binding query receiving",
            match sites.get(at) {
                Some(binding) =>
                    matches!(&place.storage,StorageId::Value(id) if id == &binding.id)
                        && place.projections.is_empty(),
                None => false,
            }
        )
    {
        return Err(path_error!(
            permit,
            Reason::Finite(Kind::RenewalIsOutsideAnExactAuthenticatedSameOwnerAssignment,)
        ));
    }
    Ok(())
}

pub(super) fn reject_unmarked_finish(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    at: &ExpressionId,
    destination: &CleanupPlace,
    _helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    // Actual persistent body roles; original nested backing and
    // explicit insertion/payload receiving banks remain separately intact.
    budget.reserve_array::<(
        Option<&crate::hir::ResolvedBinding>,
        &crate::hir::ResolvedBinding,
        bool,
    )>(
        function,
        1,
        "selected path reject_unmarked_finish body receiving",
    )?;
    budget.charge(
        function,
        3,
        "selected path reject_unmarked_finish body receiving",
    )?;
    budget.reserve_array::<(
        &dyn crate::cleanup::ReplayContext,
        &ResolvedFunction,
        &ExpressionId,
        &CleanupPlace,
        Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
        &mut ReplayBudget,
        Result<(), Error>,
    )>(
        function,
        1,
        "selected path reject_unmarked_finish formal receiving",
    )?;
    budget.charge(
        function,
        7,
        "selected path reject_unmarked_finish formal receiving",
    )?;
    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected path permit call receiving",
        Permit::control_receiver::<()>(function, budget)
    )?;
    let sites = selected_renewals!(budget, function, program).expect("held selected renewal lease");
    let found = receive!(
        budget,
        function,
        (
            &crate::cleanup_plan::ReplayRenewalBindings<'_>,
            &ExpressionId,
            Option<&crate::hir::ResolvedBinding>,
            &crate::hir::ResolvedBinding
        ),
        4,
        "selected renewal ordinary-transfer query receiving",
        sites.get(at)
    );
    let matches = match found {
        Some(binding) => {
            receive!(
                budget,
                function,
                (&crate::hir::ValueId, usize, Option<usize>, bool),
                4,
                "selected renewal destination comparison receiving",
                ()
            );
            // Complete old whole() work and ordering, before the comparison.
            budget.charge(function, 1, "ordinary renewal destination shape")?;
            match &destination.storage {
                StorageId::Value(value) => {
                    let units = value
                        .as_str()
                        .len()
                        .min(binding.id.as_str().len())
                        .checked_add(2)
                        .ok_or_else(path_refusal!(
                            budget,
                            function,
                            permit,
                            usize,
                            Kind::OrdinaryRenewalDestinationComparisonOverflowed
                        ))?;
                    budget.charge(function, units, "ordinary renewal destination comparison")?;
                    value == &binding.id && destination.projections.is_empty()
                }
                _ => false,
            }
        }
        None => false,
    };
    if matches {
        return Err(path_error!(
            permit,
            Reason::Finite(Kind::AuthenticatedRenewalUsesAnOrdinaryTransfer,)
        ));
    }
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "explicit paid formals preserve the existing receiving ABI without an aggregate owner"
)]
pub(super) fn reserve(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    at: &ExpressionId,
    binding: &CleanupPlace,
    state: &mut PathState,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    _helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    // Actual persistent body roles; original nested backing and
    // explicit insertion/payload receiving banks remain separately intact.
    budget.reserve_array::<(
        &crate::cleanup_plan::ReplayRenewalBindings<'_>,
        Vec<LivenessFlagId>,
        Option<LivenessFlagId>,
        Option<&ResolvedExpr>,
        Vec<LivenessFlagId>,
        crate::cleanup_plan::renewal_history::RenewalHistory,
    )>(function, 1, "selected path reserve body receiving")?;
    budget.charge(function, 6, "selected path reserve body receiving")?;
    budget.reserve_array::<(
        &dyn crate::cleanup::ReplayContext,
        &ResolvedFunction,
        &ExpressionId,
        &CleanupPlace,
        &mut PathState,
        &BTreeSet<StorageId>,
        &Leaves,
        Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
        &mut ReplayBudget,
        Result<(), Error>,
    )>(function, 1, "selected path reserve formal receiving")?;
    budget.charge(function, 10, "selected path reserve formal receiving")?;
    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected path permit call receiving",
        Permit::control_receiver::<()>(function, budget)
    )?;
    path_child!(
        budget,
        function,
        ValidateBindingSelectedCall,
        6,
        validate_binding_selected(
            function,
            at,
            binding,
            selected_renewals!(budget, function, program).expect("held selected renewal lease"),
            budget,
        )
    )?;
    let flags = path_child!(
        budget,
        function,
        PlaceCall,
        6,
        structural_control::selected::helpers::validate_place(
            function, binding, storage, leaves, budget,
        )
    )?;
    if flags.len() != 1
        || !state
            .live_order
            .contains(&flag_at!(budget, function, flags, 0))
        || state.renewals.contains_key(at)
        || (!matches!(
            function.cleanup_plan.schema,
            CLEANUP_PLAN_SCHEMA_V16
                | CLEANUP_PLAN_SCHEMA_V17
                | CLEANUP_PLAN_SCHEMA_V18
                | CLEANUP_PLAN_SCHEMA_V19
        ) && !state.renewals.is_empty())
    {
        return Err(path_error!(
            permit,
            Reason::Finite(Kind::RenewalReservationRequiresOneLiveUnreservedOwnerLeaf,)
        ));
    }
    let protected = receive!(
        budget,
        function,
        (
            &ResolvedFunction,
            &ExpressionId,
            Option<&crate::hir::ResolvedExpr>,
            &crate::hir::ResolvedExpr,
            bool,
            Option<LivenessFlagId>
        ),
        6,
        "selected renewal provenance lookup receiving",
        match receive!(
            budget,
            function,
            (
                &ResolvedFunction,
                &ExpressionId,
                &mut ReplayBudget,
                Result<Option<&crate::hir::ResolvedExpr>, Error>
            ),
            4,
            "selected renewal expression lookup caller receiving",
            lookup::find(function, at, budget)
        )? {
            Some(expression) if owned_provenance!(budget, function, expression) =>
                Some(flag_at!(budget, function, flags, 0)),
            _ => None,
        }
    );
    budget.reserve_array::<LivenessFlagId>(
        function,
        state.live_order.len(),
        "renewal original history",
    )?;
    if protected.is_some() {
        budget.reserve_array::<(LivenessFlagId, LivenessFlagId)>(
            function,
            state.live_order.len(),
            "renewal staged epochs",
        )?;
    }
    budget
        .reserve_btree_node::<ExpressionId, crate::cleanup_plan::renewal_history::RenewalHistory>(
            function,
            "renewal reservation node",
        )?;
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
                protected,
                budget
            )
        )?,
        ExpressionId,
        crate::cleanup_plan::renewal_history::RenewalHistory
    );
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "explicit paid formals preserve the existing receiving ABI without an aggregate owner"
)]
pub(super) fn renew(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    at: &ExpressionId,
    (source, destination): (&CleanupPlace, &CleanupPlace),
    state: &mut PathState,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    _helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    // Actual persistent body roles; original nested backing and
    // explicit insertion/payload receiving banks remain separately intact.
    budget.reserve_array::<(
        &crate::cleanup_plan::ReplayRenewalBindings<'_>,
        crate::cleanup_plan::renewal_history::RenewalHistory,
        Vec<LivenessFlagId>,
        Vec<LivenessFlagId>,
        LivenessFlagId,
        &Vec<LivenessFlagId>,
        &LivenessFlagId,
        std::slice::Iter<'_, LivenessFlagId>,
        std::slice::Iter<'_, LivenessFlagId>,
    )>(function, 1, "selected path renew body receiving")?;
    budget.charge(function, 9, "selected path renew body receiving")?;
    budget.reserve_array::<(
        &dyn crate::cleanup::ReplayContext,
        &ResolvedFunction,
        &ExpressionId,
        (&CleanupPlace, &CleanupPlace),
        &mut PathState,
        &BTreeSet<StorageId>,
        &Leaves,
        Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
        &mut ReplayBudget,
        Result<(), Error>,
    )>(function, 1, "selected path renew formal receiving")?;
    budget.charge(function, 10, "selected path renew formal receiving")?;
    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected path permit call receiving",
        Permit::control_receiver::<()>(function, budget)
    )?;
    path_child!(
        budget,
        function,
        ValidateBindingSelectedCall,
        6,
        validate_binding_selected(
            function,
            at,
            destination,
            selected_renewals!(budget, function, program).expect("held selected renewal lease"),
            budget,
        )
    )?;
    let history = receive!(
        budget,
        function,
        (
            &mut BTreeMap<ExpressionId, crate::cleanup_plan::renewal_history::RenewalHistory>,
            &ExpressionId,
            Option<crate::cleanup_plan::renewal_history::RenewalHistory>,
            crate::cleanup_plan::renewal_history::RenewalHistory
        ),
        4,
        "selected renewal reservation removal receiving",
        state.renewals.remove(at)
    )
    .ok_or_else(path_refusal!(
        budget,
        function,
        permit,
        crate::cleanup_plan::renewal_history::RenewalHistory,
        Kind::RenewalHasNoReservation
    ))?;
    let flags = path_child!(
        budget,
        function,
        PlaceCall,
        6,
        structural_control::selected::helpers::validate_place(
            function,
            destination,
            storage,
            leaves,
            budget,
        )
    )?;
    if flags.len() != 1 {
        return Err(path_error!(
            permit,
            Reason::Finite(Kind::RenewalDestinationIsNotOneOwnerLeaf)
        ));
    }
    if !budget.selected() && crate::string_ops::replacement::binding(function, at).is_some() {
        let source_flags = path_child!(
            budget,
            function,
            PlaceCall,
            6,
            structural_control::selected::helpers::validate_place(
                function, source, storage, leaves, budget,
            )
        )?;
        if source_flags == flags {
            return Err(path_error!(
                permit,
                Reason::Finite(Kind::StringReplacementAliasesItsNewOwner)
            ));
        }
        retain_flags!(
            budget,
            function,
            state.live_order,
            flag,
            !flags.contains(flag)
        );
    }
    path_child!(
        budget,
        function,
        TransferCall,
        8,
        execution::transfer(
            function,
            state,
            source,
            destination,
            storage,
            leaves,
            budget,
        )
    )?;
    let flag = flag_at!(budget, function, flags, 0);
    if !budget.selected() && crate::string_ops::replacement::binding(function, at).is_some() {
        string_replacement::finish(function, flag, &history, state)?;
        return Ok(());
    }
    if !history.contains(&flag)
        || !equal_without_flag!(budget, function, history, state.live_order, flag)
    {
        return Err(path_error!(
            permit,
            Reason::Finite(Kind::RenewalChangedUnrelatedCleanupInitializationHistory,)
        ));
    }
    state.live_order = history.history;
    Ok(())
}

pub(super) fn validate_join_compatibility<'a>(
    function: &ResolvedFunction,
    existing: std::collections::btree_set::Range<'a, Box<PathState>>,
    incoming: &PathState,
    block: BlockId,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    // Actual persistent body roles; original nested backing and
    // explicit insertion/payload receiving banks remain separately intact.
    budget.reserve_array::<(
        std::collections::btree_set::Range<'_, Box<PathState>>,
        std::collections::btree_set::Range<'_, Box<PathState>>,
        &Box<PathState>,
        usize,
        Vec<&Vec<LivenessFlagId>>,
        BTreeSet<LivenessFlagId>,
        LivenessFlagId,
        &Vec<LivenessFlagId>,
        std::slice::Iter<'_, &Vec<LivenessFlagId>>,
        BTreeMap<LivenessFlagId, BTreeSet<LivenessFlagId>>,
        BTreeMap<LivenessFlagId, usize>,
        &LivenessFlagId,
        &[LivenessFlagId],
        std::slice::Windows<'_, LivenessFlagId>,
        BTreeSet<LivenessFlagId>,
        &usize,
        usize,
        Option<LivenessFlagId>,
        LivenessFlagId,
        &LivenessFlagId,
        &mut usize,
    )>(
        function,
        1,
        "selected path validate_join_compatibility body receiving",
    )?;
    budget.charge(
        function,
        21,
        "selected path validate_join_compatibility body receiving",
    )?;
    budget.reserve_array::<(
        &ResolvedFunction,
        std::collections::btree_set::Range<'a, Box<PathState>>,
        &PathState,
        BlockId,
        &mut ReplayBudget,
        Result<(), Error>,
    )>(
        function,
        1,
        "selected path validate_join_compatibility formal receiving",
    )?;
    budget.charge(
        function,
        6,
        "selected path validate_join_compatibility formal receiving",
    )?;
    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected path permit call receiving",
        Permit::control_receiver::<()>(function, budget)
    )?;
    if scan!(
        budget,
        function,
        clone_value!(
            budget,
            function,
            existing,
            std::collections::btree_set::Range<'_, Box<PathState>>
        ),
        std::collections::btree_set::Range<'_, Box<PathState>>,
        &Box<PathState>,
        state,
        (&Box<PathState>),
        3,
        state.renewals != incoming.renewals
            || state.pending_failure != incoming.pending_failure
            || state.selected_failure != incoming.selected_failure
            || state.published != incoming.published
            || state.returned != incoming.returned,
        false,
        true
    ) {
        return Err(path_error!(permit, Reason::BlockControl(block)));
    }

    receive!(
        budget,
        function,
        (
            usize,
            std::collections::btree_set::Range<'_, Box<PathState>>
        ),
        2,
        "selected renewal history count iterator receiving",
        ()
    );
    let mut count = 0usize;
    let mut counted = clone_value!(
        budget,
        function,
        existing,
        std::collections::btree_set::Range<'_, Box<PathState>>
    );
    #[allow(
        clippy::redundant_pattern_matching,
        reason = "bare pattern preserves the paid Option receiving boundary without a method receiver"
    )]
    while let Some(_) = dynamic_next!(
        budget,
        function,
        counted,
        (
            &mut std::collections::btree_set::Range<'_, Box<PathState>>,
            Option<&Box<PathState>>,
        ),
        2
    ) {
        count += 1;
    }
    let count = count.saturating_add(1);
    budget.reserve_array::<&Vec<LivenessFlagId>>(function, count, "join history references")?;
    let mut histories = Vec::with_capacity(count);
    receive!(
        budget,
        function,
        std::collections::btree_set::Range<'_, Box<PathState>>,
        1,
        "selected renewal collected history iterator receiving",
        ()
    );
    let mut collected = existing;
    while let Some(state) = dynamic_next!(
        budget,
        function,
        collected,
        (
            &mut std::collections::btree_set::Range<'_, Box<PathState>>,
            Option<&Box<PathState>>,
            &Box<PathState>,
        ),
        3
    ) {
        receive!(
            budget,
            function,
            &Vec<LivenessFlagId>,
            1,
            "selected renewal collected history reference receiving",
            ()
        );
        histories.push(&state.live_order);
    }
    histories.push(&incoming.live_order);
    let mut flags = BTreeSet::new();
    let mut history_references = histories.iter();
    while let Some(history) = dynamic_next!(
        budget,
        function,
        history_references,
        receiving::HistoryReferencesNext<'_>,
        3
    ) {
        receive!(
            budget,
            function,
            std::slice::Iter<'_, LivenessFlagId>,
            1,
            "selected renewal history iterator receiving",
            ()
        );
        let mut history_flags = history.iter();
        while let Some(&flag) = dynamic_next!(
            budget,
            function,
            history_flags,
            receiving::HistoryFlagsNext<'_>,
            3
        ) {
            retained_insert!(
                budget,
                function,
                FlagSetInsert,
                6,
                path_join::storage::insert_set(
                    function,
                    &mut flags,
                    flag,
                    budget,
                    "join union flags"
                )
            )?;
        }
    }
    let mut successors = BTreeMap::new();
    let mut indegree = BTreeMap::new();
    let mut renewal_union_flags = flags.iter();
    while let Some(flag) = dynamic_next!(
        budget,
        function,
        renewal_union_flags,
        receiving::RenewalUnionFlagsNext<'_>,
        3
    ) {
        retained_insert!(
            budget,
            function,
            SuccessorMapInsert,
            7,
            path_join::storage::insert_map(
                function,
                &mut successors,
                *flag,
                BTreeSet::new(),
                budget,
                "join successor roots",
            )
        )?;
        retained_insert!(
            budget,
            function,
            RenewalDegreeMapInsert,
            7,
            path_join::storage::insert_map(
                function,
                &mut indegree,
                *flag,
                0usize,
                budget,
                "join indegrees",
            )
        )?;
    }
    let mut renewal_histories = histories.into_iter();
    while let Some(history) = dynamic_next!(
        budget,
        function,
        renewal_histories,
        receiving::RenewalHistoriesNext<'_>,
        3
    ) {
        let mut renewal_pairs = history.windows(2);
        while let Some(pair) = dynamic_next!(
            budget,
            function,
            renewal_pairs,
            receiving::RenewalPairsNext<'_>,
            3
        ) {
            if retained_insert!(
                budget,
                function,
                FlagSetInsert,
                6,
                path_join::storage::insert_set(
                    function,
                    mutable_get!(
                        budget,
                        function,
                        successors,
                        slice_index!(budget, function, pair, 0, LivenessFlagId),
                        LivenessFlagId,
                        BTreeSet<LivenessFlagId>
                    )
                    .expect("replayed live flag is indexed"),
                    pair_at!(budget, function, pair, 1),
                    budget,
                    "join history edges",
                )
            )? {
                if let Some(degree) = mutable_get!(
                    budget,
                    function,
                    indegree,
                    slice_index!(budget, function, pair, 1, LivenessFlagId),
                    LivenessFlagId,
                    usize
                ) {
                    *degree += 1;
                }
            }
        }
    }
    let mut ready = BTreeSet::new();
    let mut renewal_degrees = indegree.iter();
    while let Some((flag, degree)) = dynamic_next!(
        budget,
        function,
        renewal_degrees,
        receiving::RenewalDegreesNext<'_>,
        4
    ) {
        if *degree == 0 {
            retained_insert!(
                budget,
                function,
                FlagSetInsert,
                6,
                path_join::storage::insert_set(
                    function,
                    &mut ready,
                    *flag,
                    budget,
                    "join ready flags",
                )
            )?;
        }
    }
    let mut visited = 0_usize;
    while let Some(flag) = receive!(
        budget,
        function,
        (
            &mut BTreeSet<LivenessFlagId>,
            Option<LivenessFlagId>,
            LivenessFlagId
        ),
        3,
        "selected path renewal ready pop receiving",
        ready.pop_first()
    ) {
        visited += 1;
        let mut renewal_successors = map_index!(
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
            renewal_successors,
            receiving::RenewalSuccessorsNext<'_>,
            3
        ) {
            let degree = mutable_get!(budget, function, indegree, successor, LivenessFlagId, usize)
                .expect("replayed successor flag is indexed");
            *degree -= 1;
            if *degree == 0 {
                retained_insert!(
                    budget,
                    function,
                    FlagSetInsert,
                    6,
                    path_join::storage::insert_set(
                        function,
                        &mut ready,
                        *successor,
                        budget,
                        "join ready flags",
                    )
                )?;
            }
        }
    }
    if visited != flags.len() {
        return Err(path_error!(permit, Reason::BlockHistory(block)));
    }
    Ok(())
}

// Complete old materialization work, with paid selected lookup/scans before
// their own actual transient storage. Original general work API stays intact.
pub(super) fn charge_transition(
    function: &ResolvedFunction,
    transition: &CleanupTransition,
    state: &PathState,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    receive!(
        budget,
        function,
        (
            &ResolvedFunction,
            &CleanupTransition,
            &PathState,
            &mut ReplayBudget,
            Result<(), Error>,
            usize,
            Diagnostic,
            Error,
            Diagnostic,
            Error
        ),
        10,
        "selected renewal work formal and units receiving",
        ()
    );
    let units = match transition {
        CleanupTransition::ReserveRenewal { at, .. } => {
            receive!(
                budget,
                function,
                (&ResolvedExpr, bool),
                2,
                "selected renewal work provenance receiving",
                ()
            );
            let owned_rhs = match receive!(
                budget,
                function,
                (
                    &ResolvedFunction,
                    &ExpressionId,
                    &mut ReplayBudget,
                    Result<Option<&ResolvedExpr>, Error>
                ),
                4,
                "selected renewal work lookup caller receiving",
                lookup::find(function, at, budget)
            )? {
                Some(expression) => owned_provenance!(budget, function, expression),
                None => false,
            };
            state
                .live_order
                .len()
                .saturating_mul(if owned_rhs { 3 } else { 1 })
                .saturating_add(usize::from(owned_rhs))
                .saturating_add(at.as_str().len())
                .saturating_add(1)
        }
        CleanupTransition::Renew { .. } => {
            retained_units!(budget, function, state).saturating_add(state.live_order.len())
        }
        CleanupTransition::Transfer { .. } | CleanupTransition::CallCommit { .. } => {
            receive!(
                budget,
                function,
                (
                    usize,
                    std::collections::btree_map::Values<
                        '_,
                        ExpressionId,
                        crate::cleanup_plan::renewal_history::RenewalHistory,
                    >
                ),
                2,
                "selected renewal provenance total receiving",
                ()
            );
            let mut units = 0usize;
            let mut histories = state.renewals.values();
            while let Some(history) = dynamic_next!(
                budget,
                function,
                histories,
                std::collections::btree_map::Values<
                    '_,
                    ExpressionId,
                    crate::cleanup_plan::renewal_history::RenewalHistory,
                >,
                &crate::cleanup_plan::renewal_history::RenewalHistory,
                (&crate::cleanup_plan::renewal_history::RenewalHistory),
                3
            ) {
                units = units.saturating_add(receive!(
                    budget,
                    function,
                    (&crate::cleanup_plan::renewal_history::RenewalHistory, usize),
                    2,
                    "selected renewal provenance scalar query receiving",
                    history.provenance_units()
                ));
            }
            units
        }
        _ => 0,
    };
    budget.charge(function, units, "renewal history materialization")?;
    Ok(())
}
