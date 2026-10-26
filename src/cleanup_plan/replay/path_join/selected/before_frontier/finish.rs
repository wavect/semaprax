//! Exact selected path twin; complete immediate old owners remain independent.
use super::*;
pub(super) fn append_flags(
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
    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected path permit call receiving",
        Permit::control_receiver::<()>(function, budget)
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

pub(super) fn state_for_edge(
    function: &ResolvedFunction,
    mut state: PathState,
    condition: &EdgeCondition,
    contract_sources: &BTreeMap<ExpressionId, StatusSourceId>,
    budget: &mut ReplayBudget,
) -> Result<PathState, Error> {
    // Actual persistent body roles; original nested backing and
    // explicit insertion/payload receiving banks remain separately intact.
    budget.reserve_array::<(&ExpressionId, &StatusSourceId, StatusSourceId)>(
        function,
        1,
        "selected path state_for_edge body receiving",
    )?;
    budget.charge(function, 3, "selected path state_for_edge body receiving")?;
    budget.reserve_array::<(
        &ResolvedFunction,
        PathState,
        &EdgeCondition,
        &BTreeMap<ExpressionId, StatusSourceId>,
        &mut ReplayBudget,
        Result<PathState, Error>,
    )>(function, 1, "selected path state_for_edge formal receiving")?;
    budget.charge(function, 6, "selected path state_for_edge formal receiving")?;
    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected path permit call receiving",
        Permit::control_receiver::<PathState>(function, budget)
    )?;
    if state.pending_failure.is_some() || state.selected_failure.is_some() || state.published {
        return Err(path_error!(
            permit,
            Reason::Finite(Kind::ConditionalEdgeFollowsPendingSelectedOrPublishedState,)
        ));
    }
    match condition {
        EdgeCondition::Always => {}
        EdgeCondition::BooleanResult(expression, false) => {
            if let Some(source) = dynamic_get!(budget,function,contract_sources,&BTreeMap<ExpressionId,StatusSourceId>,expression,&ExpressionId,Option<&StatusSourceId>,(&StatusSourceId),4)
            {
                state.pending_failure =
                    Some(clone_value!(budget, function, source, StatusSourceId));
            }
        }
        EdgeCondition::BooleanResult(_, true)
        | EdgeCondition::VariantCase { .. }
        | EdgeCondition::ArmSelected { .. }
        | EdgeCondition::StatusZero(_) => {}
        EdgeCondition::StatusNonzero(source) => {
            state.pending_failure = Some(clone_value!(budget, function, source, StatusSourceId));
        }
    }
    Ok(state)
}

pub(super) fn finish_exit(
    function: &ResolvedFunction,
    exit: &ExitTarget,
    mut state: PathState,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    budget: &mut ReplayBudget,
) -> Result<ExitFlow, Error> {
    // Actual persistent body roles; original nested backing and
    // explicit insertion/payload receiving banks remain separately intact.
    budget.reserve_array::<(
        &EdgeId,
        &CleanupResultSource,
        &ExpressionId,
        &CleanupPlace,
        bool,
        Vec<LivenessFlagId>,
        ReturnedSource,
        &ExpressionId,
        &CleanupPlace,
        StagedCopyResultSource,
        usize,
        Vec<LivenessFlagId>,
        &LivenessFlagId,
        std::slice::Iter<'_, LivenessFlagId>,
    )>(function, 1, "selected path finish_exit body receiving")?;
    budget.charge(function, 14, "selected path finish_exit body receiving")?;
    budget.reserve_array::<(
        &ResolvedFunction,
        &ExitTarget,
        PathState,
        &BTreeSet<StorageId>,
        &Leaves,
        &mut ReplayBudget,
        Result<ExitFlow, Error>,
    )>(function, 1, "selected path finish_exit formal receiving")?;
    budget.charge(function, 7, "selected path finish_exit formal receiving")?;
    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected path permit call receiving",
        Permit::control_receiver::<ExitFlow>(function, budget)
    )?;
    match &exit.continuation {
        ExitContinuation::Continue(edge) | ExitContinuation::LoopBreak { edge, .. } => {
            if state.pending_failure.is_some() || state.selected_failure.is_some() {
                return Err(path_error!(
                    permit,
                    Reason::Finite(Kind::NormalCleanupContinuationCarriesAFailureState,)
                ));
            }
            Ok(ExitFlow::Edge(*edge, state))
        }
        ExitContinuation::LoopLatch { .. } => {
            if state.pending_failure.is_some()
                || state.selected_failure.is_some()
                || state.staged_copy_result.is_some()
                || state.returned.is_some()
            {
                return Err(path_error!(
                    permit,
                    Reason::Finite(Kind::LoopLatchCarriesAFailureStagedResultOrReturnState,)
                ));
            }
            Ok(ExitFlow::Latch(state))
        }
        ExitContinuation::FunctionReturn { edge, source, .. } => {
            if state.pending_failure.is_some()
                || state.selected_failure.is_some()
                || state.staged_copy_result.is_some()
                || state.returned.is_some()
            {
                return Err(path_error!(
                    permit,
                    Reason::Finite(
                        Kind::FunctionReturnCarriesAFailureStagedResultOrEarlierReturnState,
                    )
                ));
            }
            state.returned = Some(match source {
                CleanupResultSource::Scalar { expression } => {
                    ReturnedSource::Scalar(clone_value!(budget, function, expression, ExpressionId))
                }
                CleanupResultSource::Owned { storage: result } => {
                    let held = if variant_any!(
                        budget,
                        function,
                        state.conditional_variants,
                        variant,
                        variant.root == *result
                    ) {
                        true
                    } else {
                        let flags = path_child!(
                            budget,
                            function,
                            PlaceCall,
                            6,
                            structural_control::selected::helpers::validate_place(
                                function, result, storage, leaves, budget,
                            )
                        )?;
                        flag_all!(
                            budget,
                            function,
                            flags,
                            flag,
                            state.live_order.contains(flag)
                        )
                    };
                    if !held {
                        return Err(path_error!(
                            permit,
                            Reason::Finite(
                                Kind::OwnedFunctionReturnHasAnIncompleteProvisionalResult,
                            )
                        ));
                    }
                    budget.reserve_clone(function, result, "returned provisional place")?;
                    ReturnedSource::Owned(clone_value!(budget, function, result, CleanupPlace))
                }
            });
            Ok(ExitFlow::Edge(*edge, state))
        }
        ExitContinuation::ReturnFailure { source } => {
            if state.pending_failure.is_some()
                || receive!(
                    budget,
                    function,
                    (&StatusSourceId, bool),
                    2,
                    "selected path failure comparison receiving",
                    match &state.selected_failure {
                        Some(held) => held != source,
                        None => true,
                    }
                )
                || !state.live_order.is_empty()
                || !state.conditional_variants.is_empty()
            {
                return Err(path_error!(
                    permit,
                    Reason::Finite(Kind::FailureReturnChangesStatusOrRetainsLiveOwnership,)
                ));
            }
            Ok(ExitFlow::Terminal)
        }
        ExitContinuation::CommitResult { source } => {
            if state.pending_failure.is_some() || state.selected_failure.is_some() {
                return Err(path_error!(
                    permit,
                    Reason::Finite(Kind::ResultCommitFollowsAPendingOrSelectedFailure,)
                ));
            }
            match (&state.returned, source) {
                (None, CleanupResultSource::Scalar { expression })
                    if expression != &function.body.id =>
                {
                    return Err(path_error!(
                        permit,
                        Reason::Finite(
                            Kind::OrdinaryScalarResultCommitDoesNotPublishTheFunctionBody,
                        )
                    ));
                }
                (None, _) => {}
                (
                    Some(ReturnedSource::Scalar(expected)),
                    CleanupResultSource::Scalar { expression },
                ) if expected == expression => {}
                (Some(ReturnedSource::Owned(expected)), CleanupResultSource::Owned { storage })
                    if expected == storage => {}
                (Some(_), _) => {
                    return Err(path_error!(
                        permit,
                        Reason::Finite(Kind::ReturnDomainCommitsAResultOtherThanItsHandedOffSource,)
                    ));
                }
            }
            match source {
                CleanupResultSource::Scalar { .. } => {
                    if !state.live_order.is_empty() || !state.conditional_variants.is_empty() {
                        return Err(path_error!(
                            permit,
                            Reason::Finite(
                                Kind::ScalarResultCommitsBeforeNonResultCleanupCompletes,
                            )
                        ));
                    }
                    if state.returned.is_none()
                        && !budget.selected()
                        && expression_has_try(&function.body)
                    {
                        let staged = state.staged_copy_result.take().ok_or_else(path_refusal!(
                            budget,
                            function,
                            permit,
                            StagedCopyResultSource,
                            Kind::CopyResultCommitHasNoAuthenticatedStagedProducer
                        ))?;
                        validate_staged_target(function, &staged)?;
                    } else if state.staged_copy_result.is_some() {
                        return Err(path_error!(
                            permit,
                            Reason::Finite(Kind::NonTryScalarResultCarriesAStagedCopyAggregate,)
                        ));
                    }
                }
                CleanupResultSource::Owned { storage: result } => {
                    if let Some(index) = variant_position!(
                        budget,
                        function,
                        state.conditional_variants,
                        variant,
                        variant.root == *result
                    ) {
                        if !state.live_order.is_empty() || state.conditional_variants.len() != 1 {
                            return Err(path_error!(
                                permit,
                                Reason::Finite(
                                    Kind::ConditionalOwnedResultRetainsNonResultOwnership,
                                )
                            ));
                        }
                        state.conditional_variants.remove(index);
                    } else {
                        let result_flags = path_child!(
                            budget,
                            function,
                            PlaceCall,
                            6,
                            structural_control::selected::helpers::validate_place(
                                function, result, storage, leaves, budget,
                            )
                        )?;
                        if flag_any!(
                            budget,
                            function,
                            result_flags,
                            flag,
                            !state.live_order.contains(flag)
                        ) || state.live_order.len() != result_flags.len()
                        {
                            return Err(path_error!(permit, Reason::Finite(Kind::OwnedResultCommitHasIncompleteResultOrRemainingNonResultOwnership)));
                        }
                        if budget.selected() {
                            state.live_order.clear();
                        } else {
                            let result_flags = result_flags.into_iter().collect::<BTreeSet<_>>();
                            retain_flags!(
                                budget,
                                function,
                                state.live_order,
                                flag,
                                !result_flags.contains(flag)
                            );
                        }
                    }
                }
            }
            state.published = true;
            if !state.live_order.is_empty() || !state.conditional_variants.is_empty() {
                return Err(path_error!(
                    permit,
                    Reason::Finite(Kind::ResultPublicationLeavesCleanupOwnershipLive,)
                ));
            }
            Ok(ExitFlow::Terminal)
        }
        ExitContinuation::ReturnUnit => {
            if state.pending_failure.is_some()
                || state.selected_failure.is_some()
                || !state.live_order.is_empty()
                || !state.conditional_variants.is_empty()
            {
                return Err(path_error!(
                    permit,
                    Reason::Finite(Kind::UnitReturnFollowsFailureOrIncompleteCleanup,)
                ));
            }
            state.published = true;
            Ok(ExitFlow::Terminal)
        }
    }
}

pub(super) fn require_normal_flow_state(
    function: &ResolvedFunction,
    state: &PathState,
    block: BlockId,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    budget.reserve_array::<(
        &ResolvedFunction,
        &PathState,
        BlockId,
        &mut ReplayBudget,
        Result<(), Error>,
    )>(
        function,
        1,
        "selected path require_normal_flow_state formal receiving",
    )?;
    budget.charge(
        function,
        5,
        "selected path require_normal_flow_state formal receiving",
    )?;
    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected path permit call receiving",
        Permit::control_receiver::<()>(function, budget)
    )?;
    if state.pending_failure.is_none() && state.selected_failure.is_none() && !state.published {
        Ok(())
    } else {
        Err(path_error!(permit, Reason::NormalFlow(block)))
    }
}
