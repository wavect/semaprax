//! Exact selected path twin; complete immediate old owners remain independent.
use super::*;
use crate::cleanup_plan::replay::semantic_refusal::Error as _;
#[allow(clippy::too_many_arguments)]
pub(super) fn execute(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    transition: &CleanupTransition,
    state: &mut PathState,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    // Actual persistent body roles; original nested backing and
    // explicit insertion/payload receiving banks remain separately intact.
    budget.reserve_array::<(
        &CleanupPlace,
        Vec<LivenessFlagId>,
        &ExpressionId,
        &CleanupPlace,
        &CleanupPlace,
        &CleanupPlace,
        &ExpressionId,
        &CleanupPlace,
        &CleanupPlace,
        Option<Vec<LivenessFlagId>>,
        &ExpressionId,
        &Vec<crate::cleanup_plan::CallArgumentTransfer>,
        BTreeSet<LivenessFlagId>,
        &crate::cleanup_plan::CallArgumentTransfer,
        std::slice::Iter<'_, crate::cleanup_plan::CallArgumentTransfer>,
        LivenessFlagId,
        std::vec::IntoIter<LivenessFlagId>,
        &mut crate::cleanup_plan::renewal_history::RenewalHistory,
        std::collections::btree_map::ValuesMut<
            '_,
            ExpressionId,
            crate::cleanup_plan::renewal_history::RenewalHistory,
        >,
        &BTreeSet<LivenessFlagId>,
        &StatusSourceId,
        &StagedCopyResultSource,
    )>(function, 1, "selected path execute body receiving")?;
    budget.charge(function, 22, "selected path execute body receiving")?;
    budget.reserve_array::<(
        &dyn crate::cleanup::ReplayContext,
        &ResolvedFunction,
        &CleanupTransition,
        &mut PathState,
        &BTreeSet<StorageId>,
        &Leaves,
        Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
        &mut ReplayBudget,
        Result<(), Error>,
    )>(function, 1, "selected path execute formal receiving")?;
    budget.charge(function, 9, "selected path execute formal receiving")?;
    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected path permit call receiving",
        Permit::control_receiver::<()>(function, budget)
    )?;
    if state.published {
        return Err(path_error!(
            permit,
            Reason::Finite(Kind::CleanupTransitionOccursAfterResultPublication,)
        ));
    }
    if state.selected_failure.is_some()
        && !matches!(transition, CleanupTransition::SelectFailure { .. })
    {
        return Err(path_error!(
            permit,
            Reason::Finite(Kind::OrdinaryCleanupTransitionOccursAfterFailureSelection,)
        ));
    }
    if state.pending_failure.is_some()
        && !matches!(transition, CleanupTransition::SelectFailure { .. })
    {
        return Err(path_error!(
            permit,
            Reason::Finite(Kind::OperationFailureIsNotSelectedBeforeAnotherTransition,)
        ));
    }
    if !state.conditional_variants.is_empty() {
        return Err(path_error!(
            permit,
            Reason::Finite(Kind::SelectedPrimitiveReplayHasConditionalAggregateState,)
        ));
    }
    match transition {
        CleanupTransition::Initialize { destination, .. } => {
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
            // append_dead_flags performs the same validity check before growth.
            if flags.is_empty()
                || flag_any!(
                    budget,
                    function,
                    flags,
                    flag,
                    state.live_order.contains(flag)
                )
            {
                return Err(path_error!(
                    permit,
                    Reason::Finite(Kind::InitializeTransitionInitializesAnEmptyOrLiveCleanupPlace,)
                ));
            }
            budget.reserve_vec(
                function,
                &mut state.live_order,
                flags.len(),
                "initialized live history",
            )?;
            path_child!(
                budget,
                function,
                AppendFlagsCall,
                6,
                finish::append_flags(
                    function,
                    state,
                    flags,
                    Kind::InitializeTransitionInitializesAnEmptyOrLiveCleanupPlace,
                    budget,
                )
            )?;
        }
        CleanupTransition::ReserveRenewal { at, binding } => {
            path_child!(
                budget,
                function,
                ReserveCall,
                10,
                renewal::reserve(
                    program, function, at, binding, state, storage, leaves, helpers, budget,
                )
            )?;
        }
        CleanupTransition::Renew {
            at,
            source,
            destination,
        } => {
            path_child!(
                budget,
                function,
                RenewCall,
                10,
                renewal::renew(
                    program,
                    function,
                    at,
                    (source, destination),
                    state,
                    storage,
                    leaves,
                    helpers,
                    budget,
                )
            )?;
        }
        CleanupTransition::Transfer {
            at,
            source,
            destination,
        } => {
            path_child!(
                budget,
                function,
                RejectUnmarkedFinishCall,
                7,
                renewal::reject_unmarked_finish(
                    program,
                    function,
                    at,
                    destination,
                    helpers,
                    budget
                )
            )?;
            let append = path_child!(
                budget,
                function,
                ReserveAppendCall,
                8,
                strings::reserve_append(program, function, at, source, destination, state, budget)
            )?;
            path_child!(
                budget,
                function,
                TransferCall,
                8,
                transfer(
                    function,
                    state,
                    source,
                    destination,
                    storage,
                    leaves,
                    budget,
                )
            )?;
            path_child!(
                budget,
                function,
                PublishAppendCall,
                5,
                strings::publish_append(function, append, state, budget)
            )?;
        }
        CleanupTransition::CallCommit { call, arguments } => {
            let mut consumed = BTreeSet::new();
            let mut call_arguments = arguments.iter();
            while let Some(argument) = dynamic_next!(
                budget,
                function,
                call_arguments,
                receiving::CallArgumentsNext<'_>,
                3
            ) {
                let mut argument_flags = path_child!(
                    budget,
                    function,
                    PlaceCall,
                    6,
                    structural_control::selected::helpers::validate_place(
                        function,
                        &argument.source,
                        storage,
                        leaves,
                        budget,
                    )
                )?
                .into_iter();
                while let Some(flag) = dynamic_next!(
                    budget,
                    function,
                    argument_flags,
                    receiving::ArgumentFlagsNext<'_>,
                    3
                ) {
                    if !state.live_order.contains(&flag) {
                        return Err(existing_structural_error!(
                            budget,
                            function,
                            call,
                            DeadCallFlag,
                            flag
                        ));
                    }
                    if consumed.contains(&flag) {
                        return Err(existing_structural_error!(
                            budget,
                            function,
                            call,
                            RepeatedCallFlag,
                            flag
                        ));
                    }
                    budget.reserve_btree_node::<LivenessFlagId, ()>(
                        function,
                        "committed argument flags",
                    )?;
                    receive!(
                        budget,
                        function,
                        (&mut BTreeSet<LivenessFlagId>, LivenessFlagId, bool),
                        3,
                        "selected committed flag insertion receiving",
                        consumed.insert(flag)
                    );
                }
            }
            let mut renewal_entries = state.renewals.values_mut();
            while let Some(history) = dynamic_next!(
                budget,
                function,
                renewal_entries,
                receiving::RenewalEntriesNext<'_>,
                3
            ) {
                receive!(
                    budget,
                    function,
                    (
                        &ResolvedFunction,
                        &mut crate::cleanup_plan::renewal_history::RenewalHistory,
                        &BTreeSet<LivenessFlagId>,
                        &mut ReplayBudget,
                        Result<(), Error>
                    ),
                    5,
                    "selected renewal commit caller receiving",
                    history::commit(function, history, &consumed, budget)
                )?;
            }
            retain_flags!(
                budget,
                function,
                state.live_order,
                flag,
                !consumed.contains(flag)
            );
        }
        CleanupTransition::SelectFailure { source } => {
            state.renewals.clear();
            if state.selected_failure.is_some() {
                return Err(path_error!(
                    permit,
                    Reason::Finite(Kind::FailureSelectionIsNotWriteOnceDuringPathReplay,)
                ));
            }
            if receive!(
                budget,
                function,
                (&StatusSourceId, bool),
                2,
                "selected path failure comparison receiving",
                match &state.pending_failure {
                    Some(held) => held != source,
                    None => true,
                }
            ) {
                return Err(existing_structural_error!(
                    budget,
                    function,
                    source.expression,
                    MismatchedFailure
                ));
            }
            state.pending_failure = None;
            state.selected_failure = Some(clone_value!(budget, function, source, StatusSourceId));
        }
        CleanupTransition::StageCopyResult { source } => {
            if state.staged_copy_result.is_some() {
                return Err(path_error!(
                    permit,
                    Reason::Finite(Kind::CopyAggregateResultIsStagedMoreThanOnceOnOnePath,)
                ));
            }
            if !state.live_order.is_empty() {
                return Err(path_error!(
                    permit,
                    Reason::Finite(Kind::CopyAggregateResultStagingCarriesResourceLiveness,)
                ));
            }
            budget.reserve_clone(function, source, "staged Copy producer")?;
            state.staged_copy_result = Some(clone_value!(
                budget,
                function,
                source,
                StagedCopyResultSource
            ));
        }
        CleanupTransition::InitializeVariant { .. }
        | CleanupTransition::TransferVariant { .. }
        | CleanupTransition::AuthenticateVariantCase { .. } => {
            return Err(path_error!(
                permit,
                Reason::Finite(
                    Kind::SelectedPrimitiveReplayExcludesConditionalAggregateTransitions,
                )
            ));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn transfer(
    function: &ResolvedFunction,
    state: &mut PathState,
    source: &CleanupPlace,
    destination: &CleanupPlace,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    // Actual persistent body roles; original nested backing and
    // explicit insertion/payload receiving banks remain separately intact.
    budget.reserve_array::<(
        Vec<LivenessFlagId>,
        Vec<LivenessFlagId>,
        Option<usize>,
        Vec<(LivenessFlagId, LivenessFlagId)>,
        &LivenessFlagId,
        &Leaf,
        &[DeclarationId],
        usize,
        Option<(LivenessFlagId, &Leaf)>,
        &LivenessFlagId,
        &Leaf,
        LivenessFlagId,
        &Leaf,
        usize,
        &mut ReplayConditionalVariant,
        &mut Vec<LivenessFlagId>,
        &mut LivenessFlagId,
        usize,
        Vec<LivenessFlagId>,
        Vec<LivenessFlagId>,
        usize,
        usize,
        usize,
        usize,
        &mut crate::cleanup_plan::renewal_history::RenewalHistory,
        LivenessFlagId,
        &LivenessFlagId,
        std::slice::Iter<'_, LivenessFlagId>,
        std::collections::btree_map::Iter<'_, LivenessFlagId, Leaf>,
        std::vec::IntoIter<LivenessFlagId>,
    )>(function, 1, "selected path transfer body receiving")?;
    budget.charge(function, 30, "selected path transfer body receiving")?;
    budget.reserve_array::<(
        &ResolvedFunction,
        &mut PathState,
        &CleanupPlace,
        &CleanupPlace,
        &BTreeSet<StorageId>,
        &Leaves,
        &mut ReplayBudget,
        Result<(), Error>,
    )>(function, 1, "selected path transfer formal receiving")?;
    budget.charge(function, 8, "selected path transfer formal receiving")?;
    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected path permit call receiving",
        Permit::control_receiver::<()>(function, budget)
    )?;
    let source_flags = path_child!(
        budget,
        function,
        PlaceCall,
        6,
        structural_control::selected::helpers::validate_place(
            function, source, storage, leaves, budget,
        )
    )?;
    let destination_flags = path_child!(
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
    if source_flags.len() != destination_flags.len()
        || flag_any!(
            budget,
            function,
            source_flags,
            flag,
            destination_flags.contains(flag)
        )
    {
        return Err(path_error!(
            permit,
            Reason::Finite(Kind::CleanupTransferDoesNotHaveDisjointEqualSizeOwnershipEpochs,)
        ));
    }
    let conditional = variant_position!(
        budget,
        function,
        state.conditional_variants,
        variant,
        variant.root == *source
    );
    if conditional.is_none() {
        if flag_any!(
            budget,
            function,
            source_flags,
            flag,
            !state.live_order.contains(flag)
        ) {
            return Err(path_error!(
                permit,
                Reason::Finite(Kind::CleanupTransferReadsADeadSourceLeaf)
            ));
        }
        if flag_any!(
            budget,
            function,
            destination_flags,
            flag,
            state.live_order.contains(flag)
        ) {
            return Err(path_error!(
                permit,
                Reason::Finite(Kind::CleanupTransferInitializesALiveDestinationLeaf,)
            ));
        }
    }
    budget.reserve_array::<(LivenessFlagId, LivenessFlagId)>(
        function,
        source_flags.len(),
        "ownership transfer mapping",
    )?;
    let mut mapping = Vec::with_capacity(source_flags.len());
    let mut source_flag_entries = source_flags.iter();
    while let Some(source_flag) = dynamic_next!(
        budget,
        function,
        source_flag_entries,
        receiving::SourceFlagEntriesNext<'_>,
        3
    ) {
        let source_leaf = leaf_at!(budget, function, leaves, source_flag);
        let relative = receive!(
            budget,
            function,
            (
                &[DeclarationId],
                &[DeclarationId],
                Option<&[DeclarationId]>,
                &[DeclarationId]
            ),
            4,
            "selected transfer projection prefix receiving",
            source_leaf
                .place
                .projections
                .strip_prefix(source.projections.as_slice())
        )
        .ok_or_else(path_refusal!(
            budget,
            function,
            permit,
            &[DeclarationId],
            Kind::TransferSourceIsNotAProjectionPrefix
        ))?;
        let destination_depth = destination
            .projections
            .len()
            .checked_add(relative.len())
            .ok_or_else(path_refusal!(
                budget,
                function,
                permit,
                usize,
                Kind::TransferDestinationProjectionDepthOverflows
            ))?;
        let mut matched = None;
        let mut leaf_entries = leaves.iter();
        while let Some((candidate, leaf)) = dynamic_next!(
            budget,
            function,
            leaf_entries,
            receiving::LeafEntriesNext<'_>,
            4
        ) {
            budget.charge(function, 1, "selected transfer leaf correspondence")?;
            if leaf.place.storage == destination.storage
                && leaf.place.projections.len() == destination_depth
                && leaf.place.projections.starts_with(&destination.projections)
                && *receive!(
                    budget,
                    function,
                    (
                        &Vec<DeclarationId>,
                        std::ops::RangeFrom<usize>,
                        &[DeclarationId]
                    ),
                    3,
                    "selected transfer relative projection receiving",
                    &leaf.place.projections[destination.projections.len()..]
                ) == *relative
            {
                matched = Some((*candidate, leaf));
                break;
            }
        }
        let Some((destination_flag, destination_leaf)) = matched else {
            return Err(path_error!(
                permit,
                Reason::Finite(Kind::CleanupTransferLeafShapesDoNotCorrespond,)
            ));
        };
        if source_leaf.lifecycle != destination_leaf.lifecycle
            || scan!(
                budget,
                function,
                mapping.iter(),
                std::slice::Iter<'_, (LivenessFlagId, LivenessFlagId)>,
                &(LivenessFlagId, LivenessFlagId),
                (_, flag),
                (&LivenessFlagId),
                3,
                *flag == destination_flag,
                false,
                true
            )
        {
            return Err(path_error!(
                permit,
                Reason::Finite(Kind::CleanupTransferChangesALeafLifecycleOrAliasesItsDestination,)
            ));
        }
        mapping.push((*source_flag, destination_flag));
    }
    if mapping.is_empty() {
        return Err(path_error!(
            permit,
            Reason::Finite(Kind::CleanupTransferHasNoMappedOwnershipLeaves,)
        ));
    }
    if let Some(index) = conditional {
        // Validate all flags before mutating this path. No rebuilding or sorting
        // the case vectors is needed to preserve their existing canonical order.
        if scan!(
            budget,
            function,
            index!(
                budget,
                function,
                state.conditional_variants,
                index,
                ReplayConditionalVariant
            )
            .cases
            .iter(),
            std::slice::Iter<'_, (DeclarationId, Vec<LivenessFlagId>)>,
            &(DeclarationId, Vec<LivenessFlagId>),
            (_, flags),
            (&Vec<LivenessFlagId>),
            3,
            flag_any!(
                budget,
                function,
                flags,
                flag,
                !scan!(
                    budget,
                    function,
                    mapping.iter(),
                    std::slice::Iter<'_, (LivenessFlagId, LivenessFlagId)>,
                    &(LivenessFlagId, LivenessFlagId),
                    (source, _),
                    (&LivenessFlagId),
                    3,
                    source == flag,
                    false,
                    true
                )
            ),
            false,
            true
        ) {
            return Err(path_error!(
                permit,
                Reason::Finite(Kind::ConditionalCleanupTransferOmitsACaseLeaf,)
            ));
        }
        budget.reserve_clone(function, destination, "conditional destination place")?;
        let variant = index_mut!(
            budget,
            function,
            state.conditional_variants,
            index,
            ReplayConditionalVariant
        );
        let mut variant_case_entries = variant.cases.iter_mut();
        while let Some((_, flags)) = dynamic_next!(
            budget,
            function,
            variant_case_entries,
            receiving::VariantCaseEntriesNext<'_>,
            3
        ) {
            let mut variant_flag_entries = flags.iter_mut();
            while let Some(flag) = dynamic_next!(
                budget,
                function,
                variant_flag_entries,
                receiving::VariantFlagEntriesNext<'_>,
                3
            ) {
                *flag = receive!(
                    budget,
                    function,
                    LivenessFlagId,
                    1,
                    "selected conditional mapping target receiving",
                    find!(
                        budget,
                        function,
                        mapping.iter(),
                        std::slice::Iter<'_, (LivenessFlagId, LivenessFlagId)>,
                        &(LivenessFlagId, LivenessFlagId),
                        pair,
                        (&(LivenessFlagId, LivenessFlagId)),
                        3,
                        pair.0 == *flag
                    )
                    .expect("validated complete conditional transfer mapping")
                    .1
                );
            }
        }
        variant.root = clone_value!(budget, function, destination, CleanupPlace);
        return Ok(());
    }
    // Preserve partial-field history before removing source flags. Whole-source
    // transfers use structural source order, as in ordinary replay.
    let history_count = if source.projections.is_empty() {
        source_flags.len()
    } else {
        receive!(
            budget,
            function,
            (usize, std::slice::Iter<'_, LivenessFlagId>),
            2,
            "selected partial transfer count receiving",
            ()
        );
        let mut count = 0usize;
        let mut candidates = state.live_order.iter();
        while let Some(flag) = dynamic_next!(
            budget,
            function,
            candidates,
            std::slice::Iter<'_, LivenessFlagId>,
            &LivenessFlagId,
            (&LivenessFlagId),
            3
        ) {
            if receive!(
                budget,
                function,
                bool,
                1,
                "selected partial history count predicate receiving",
                source_flags.contains(flag)
            ) {
                count += 1;
            }
        }
        count
    };
    budget.reserve_array::<LivenessFlagId>(function, history_count, "partial transfer history")?;
    let source_history = if source.projections.is_empty() {
        clone_value!(budget, function, source_flags, Vec<LivenessFlagId>)
    } else {
        let mut history = Vec::with_capacity(history_count);
        receive!(
            budget,
            function,
            std::slice::Iter<'_, LivenessFlagId>,
            1,
            "selected partial transfer collect iterator receiving",
            ()
        );
        let mut candidates = state.live_order.iter();
        while let Some(flag) = dynamic_next!(
            budget,
            function,
            candidates,
            std::slice::Iter<'_, LivenessFlagId>,
            &LivenessFlagId,
            (&LivenessFlagId),
            3
        ) {
            if receive!(
                budget,
                function,
                bool,
                1,
                "selected partial history collect predicate receiving",
                source_flags.contains(flag)
            ) {
                receive!(
                    budget,
                    function,
                    LivenessFlagId,
                    1,
                    "selected partial history copy receiving",
                    ()
                );
                history.push(*flag);
            }
        }
        history
    };
    let after_remove = state
        .live_order
        .len()
        .checked_sub(source_flags.len())
        .ok_or_else(path_refusal!(
            budget,
            function,
            permit,
            usize,
            Kind::CleanupTransferSourceHistoryIsIncomplete
        ))?;
    let add = if destination.projections.is_empty() {
        destination_flags.len()
    } else {
        source_history.len()
    };
    let required = after_remove.checked_add(add).ok_or_else(path_refusal!(
        budget,
        function,
        permit,
        usize,
        Kind::CleanupTransferHistoryLengthOverflowed
    ))?;
    // Reserve while the old live buffer and mapping/history scratch remain held.
    if required > state.live_order.capacity() {
        let extra = required.saturating_sub(state.live_order.len());
        budget.reserve_vec(
            function,
            &mut state.live_order,
            extra,
            "transferred live history",
        )?;
    }
    let mut renewal_entries = state.renewals.values_mut();
    while let Some(history) = dynamic_next!(
        budget,
        function,
        renewal_entries,
        receiving::RenewalEntriesNext<'_>,
        3
    ) {
        receive!(
            budget,
            function,
            (
                &ResolvedFunction,
                &mut crate::cleanup_plan::renewal_history::RenewalHistory,
                &[(LivenessFlagId, LivenessFlagId)],
                &mut ReplayBudget,
                Result<(), Error>
            ),
            5,
            "selected renewal stage caller receiving",
            history::stage(function, history, &mapping, budget)
        )?;
    }
    retain_flags!(
        budget,
        function,
        state.live_order,
        flag,
        !source_flags.contains(flag)
    );
    if destination.projections.is_empty() {
        receive!(
            budget,
            function,
            std::vec::IntoIter<LivenessFlagId>,
            1,
            "selected whole transfer publication iterator receiving",
            ()
        );
        let mut published = destination_flags.into_iter();
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
    } else {
        receive!(
            budget,
            function,
            std::vec::IntoIter<LivenessFlagId>,
            1,
            "selected partial transfer publication iterator receiving",
            ()
        );
        let mut published = source_history.into_iter();
        while let Some(flag) = dynamic_next!(
            budget,
            function,
            published,
            std::vec::IntoIter<LivenessFlagId>,
            LivenessFlagId,
            (LivenessFlagId),
            3
        ) {
            let target = receive!(
                budget,
                function,
                LivenessFlagId,
                1,
                "selected partial transfer target copy receiving",
                find!(
                    budget,
                    function,
                    mapping.iter(),
                    std::slice::Iter<'_, (LivenessFlagId, LivenessFlagId)>,
                    &(LivenessFlagId, LivenessFlagId),
                    pair,
                    (&(LivenessFlagId, LivenessFlagId)),
                    3,
                    pair.0 == flag
                )
                .expect("validated source transfer mapping")
                .1
            );
            state.live_order.push(target);
        }
    }
    Ok(())
}

/// Compare finalizers directly with borrowed live history. No ownership set,
/// conditional guard copies, or replacement cleanup order is materialized.
#[allow(clippy::too_many_arguments)]
pub(super) fn exit(
    function: &ResolvedFunction,
    exit: &ExitTarget,
    mut state: PathState,
    storage_regions: &BTreeMap<StorageId, CleanupRegionId>,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    extra_disposed: &BTreeSet<StorageId>,
    budget: &mut ReplayBudget,
) -> Result<ExitFlow, Error> {
    // Actual persistent body roles; original nested backing and
    // explicit insertion/payload receiving banks remain separately intact.
    budget.reserve_array::<(Option<&CleanupPlace>, &CleanupPlace)>(
        function,
        1,
        "selected path exit body receiving",
    )?;
    budget.charge(function, 2, "selected path exit body receiving")?;
    budget.reserve_array::<(
        &ResolvedFunction,
        &ExitTarget,
        PathState,
        &BTreeMap<StorageId, CleanupRegionId>,
        &BTreeSet<StorageId>,
        &Leaves,
        &BTreeSet<StorageId>,
        &mut ReplayBudget,
        Result<ExitFlow, Error>,
    )>(function, 1, "selected path exit formal receiving")?;
    budget.charge(function, 9, "selected path exit formal receiving")?;
    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected path permit call receiving",
        Permit::control_receiver::<ExitFlow>(function, budget)
    )?;
    if state.published {
        return Err(path_error!(
            permit,
            Reason::Finite(Kind::CleanupExitFollowsPublication)
        ));
    }
    if !state.conditional_variants.is_empty() {
        return Err(path_error!(
            permit,
            Reason::Finite(Kind::SelectedPrimitiveExitHasConditionalOwnership,)
        ));
    }
    let protected = match &exit.continuation {
        ExitContinuation::CommitResult {
            source: CleanupResultSource::Owned { storage },
        } => Some(storage),
        _ => None,
    };
    if let Some(result) = protected {
        // Preserve ordinary unknown/empty-place refusal before finalizer checks.
        path_child!(
            budget,
            function,
            PlaceCall,
            6,
            structural_control::selected::helpers::validate_place(
                function, result, storage, leaves, budget,
            )
        )?;
    }
    // These are lexical query expansions, not closures or forwarding calls.
    macro_rules! protects {
        ($flag:expr) => {{
            receive!(
                budget,
                function,
                (&CleanupPlace, &Leaf, bool),
                3,
                "selected exit protected leaf query receiving",
                ()
            );
            match protected {
                Some(place) => {
                    let leaf = leaf_at!(budget, function, leaves, $flag);
                    leaf.place.storage == place.storage
                        && leaf.place.projections.starts_with(&place.projections)
                }
                None => false,
            }
        }};
    }
    macro_rules! leaves_region {
        ($flag:expr) => {{
            receive!(
                budget,
                function,
                (&Leaf, bool),
                2,
                "selected exit departing leaf query receiving",
                ()
            );
            let leaf = leaf_at!(budget, function, leaves, $flag);
            exit.leaves_regions.contains(map_index!(
                budget,
                function,
                storage_regions,
                &leaf.place.storage,
                StorageId,
                CleanupRegionId
            )) || extra_disposed.contains(&leaf.place.storage)
        }};
    }
    receive!(
        budget,
        function,
        (
            std::iter::Rev<std::slice::Iter<'_, LivenessFlagId>>,
            std::slice::Iter<'_, crate::cleanup_plan::FinalizeAction>
        ),
        2,
        "selected exit comparison iterators receiving",
        ()
    );
    let mut expected = state.live_order.iter().rev();
    let mut actual = exit.finalize_in_order.iter();
    // Iterator::ne evaluates one filtered left result followed by one filtered
    // right result, and stops at the first unequal pair or matched terminal pair.
    loop {
        receive!(
            budget,
            function,
            Option<&LivenessFlagId>,
            1,
            "selected exit expected filtered result receiving",
            ()
        );
        let mut expected_flag = None;
        while let Some(flag) = dynamic_next!(
            budget,
            function,
            expected,
            std::iter::Rev<std::slice::Iter<'_, LivenessFlagId>>,
            &LivenessFlagId,
            (&LivenessFlagId),
            3
        ) {
            if receive!(
                budget,
                function,
                bool,
                1,
                "selected exit expected filter receiving",
                leaves_region!(flag) && !protects!(flag)
            ) {
                expected_flag = Some(flag);
                break;
            }
        }
        receive!(
            budget,
            function,
            Option<&LivenessFlagId>,
            1,
            "selected exit actual filtered result receiving",
            ()
        );
        let mut actual_flag = None;
        while let Some(action) = dynamic_next!(
            budget,
            function,
            actual,
            std::slice::Iter<'_, crate::cleanup_plan::FinalizeAction>,
            &crate::cleanup_plan::FinalizeAction,
            (&crate::cleanup_plan::FinalizeAction),
            3
        ) {
            if receive!(
                budget,
                function,
                bool,
                1,
                "selected exit actual filter receiving",
                state.live_order.contains(&action.guard_flag)
            ) {
                receive!(
                    budget,
                    function,
                    &LivenessFlagId,
                    1,
                    "selected exit action guard borrow receiving",
                    ()
                );
                actual_flag = Some(&action.guard_flag);
                break;
            }
        }
        if receive!(
            budget,
            function,
            bool,
            1,
            "selected exit sequence comparison receiving",
            expected_flag != actual_flag
        ) {
            return Err(path_error!(permit, Reason::ExitOrder(exit.id)));
        }
        if expected_flag.is_none() {
            break;
        }
    }
    if scan!(
        budget,
        function,
        exit.finalize_in_order.iter(),
        std::slice::Iter<'_, crate::cleanup_plan::FinalizeAction>,
        &crate::cleanup_plan::FinalizeAction,
        action,
        (&crate::cleanup_plan::FinalizeAction),
        3,
        action.active_case.is_some(),
        false,
        true
    ) {
        return Err(path_error!(
            permit,
            Reason::Finite(Kind::ExitDoesNotPreserveExactConditionalVariantFinalizers)
        ));
    }
    retain_flags!(
        budget,
        function,
        state.live_order,
        flag,
        !(leaves_region!(flag) && !protects!(flag))
    );
    if flag_any!(
        budget,
        function,
        state.live_order,
        flag,
        exit.leaves_regions.contains(map_index!(
            budget,
            function,
            storage_regions,
            &leaf_at!(budget, function, leaves, flag).place.storage,
            StorageId,
            CleanupRegionId
        )) && !protects!(flag)
    ) {
        return Err(path_error!(
            permit,
            Reason::Finite(Kind::CleanupExitRetainsALiveNonResultLeafInARegionItLeaves)
        ));
    }
    path_child!(
        budget,
        function,
        FinishExitCall,
        7,
        finish::finish_exit(function, exit, state, storage, leaves, budget)
    )
}
