//! Transition, exit and edge replay over one path's ownership state.
//!
//! Relocated verbatim from `replay.rs`, which has a recorded size ceiling; the
//! lexical exit continuations added by cleanup plan v18 live here with the rest.
use super::*;

#[allow(
    clippy::too_many_arguments,
    reason = "keep independent held authority and canonical replay inputs explicit"
)]
pub(super) fn execute_replay_transition<E: semantic_refusal::Error>(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    transition: &CleanupTransition,
    state: &mut PathState,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
    budget: &mut ReplayBudget,
) -> Result<(), E> {
    if budget.selected() {
        return transition_storage::execute::<E>(
            program, function, transition, state, storage, leaves, helpers, budget,
        );
    }
    if state.published {
        return Err(replay_error(
            function,
            "cleanup transition occurs after result publication",
        )
        .into());
    }
    if state.selected_failure.is_some()
        && !matches!(transition, CleanupTransition::SelectFailure { .. })
    {
        return Err(replay_error(
            function,
            "ordinary cleanup transition occurs after failure selection",
        )
        .into());
    }
    if state.pending_failure.is_some()
        && !matches!(transition, CleanupTransition::SelectFailure { .. })
    {
        return Err(replay_error(
            function,
            "operation failure is not selected before another transition",
        )
        .into());
    }

    match transition {
        CleanupTransition::Initialize { destination, .. } => {
            let flags = validate_place(function, destination, storage, leaves)?;
            append_dead_flags(function, state, flags, "initialize transition")?;
        }
        CleanupTransition::ReserveRenewal { at, binding } => renewal::reserve(
            program, function, at, binding, state, storage, leaves, helpers, budget,
        )?,
        CleanupTransition::Renew {
            at,
            source,
            destination,
        } => renewal::renew(
            program,
            function,
            at,
            (source, destination),
            state,
            storage,
            leaves,
            helpers,
            budget,
        )?,
        CleanupTransition::Transfer {
            at,
            source,
            destination,
        } => {
            renewal::reject_unmarked_finish(program, function, at, destination, helpers, budget)?;
            let append = strings::reserve_append(function, at, source, destination, state)?;
            replay_transfer(
                function,
                state,
                source,
                destination,
                storage,
                leaves,
                budget,
            )?;
            seal_try_residual(program, function, at, destination, state, storage, leaves)?;
            strings::publish_append(function, append, state)?;
        }
        CleanupTransition::TransferVariant {
            source,
            destination,
            variant,
            ..
        } => {
            if !state
                .conditional_variants
                .iter()
                .any(|conditional| conditional.root == *source)
            {
                materialize_constructed_variant(
                    program, function, state, source, variant, storage, leaves,
                )?;
            }
            replay_transfer(
                function,
                state,
                source,
                destination,
                storage,
                leaves,
                budget,
            )?;
        }
        CleanupTransition::InitializeVariant {
            destination,
            variant,
            ..
        } => {
            let flags = validate_place(function, destination, storage, leaves)?;
            if flags.iter().any(|flag| {
                state.live_order.contains(flag)
                    || state.conditional_variants.iter().any(|conditional| {
                        conditional
                            .cases
                            .iter()
                            .any(|(_, case_flags)| case_flags.contains(flag))
                    })
            }) {
                return Err(replay_error(
                    function,
                    "variant initialization targets a live cleanup place",
                )
                .into());
            }
            let slot = function
                .cleanup_plan
                .slots
                .iter()
                .find(|slot| slot.storage == destination.storage)
                .ok_or_else(|| replay_error(function, "variant initialization has no slot"))?;
            let FieldLivenessShape::Variant { cases, .. } = &slot.field_liveness_shape else {
                return Err(replay_error(
                    function,
                    "variant initialization destination is not conditional storage",
                )
                .into());
            };
            let conditional_cases = cases
                .iter()
                .map(|case| {
                    let prefix = destination
                        .projections
                        .iter()
                        .chain(std::iter::once(&case.case))
                        .cloned()
                        .collect::<Vec<_>>();
                    let case_flags = flags
                        .iter()
                        .filter(|flag| leaves[flag].place.projections.starts_with(&prefix))
                        .copied()
                        .collect::<Vec<_>>();
                    (case.case.clone(), case_flags)
                })
                .collect();
            state.conditional_variants.push(ReplayConditionalVariant {
                root: destination.clone(),
                variant: variant.clone(),
                cases: conditional_cases,
            });
        }
        CleanupTransition::AuthenticateVariantCase {
            source,
            variant,
            case,
            ..
        } => {
            if !state
                .conditional_variants
                .iter()
                .any(|entry| entry.root == *source)
            {
                materialize_constructed_variant(
                    program, function, state, source, variant, storage, leaves,
                )?;
            }
            if let Some(index) = state
                .conditional_variants
                .iter()
                .position(|candidate| candidate.root == *source && candidate.variant == *variant)
            {
                let conditional = state.conditional_variants.remove(index);
                let flags = conditional
                    .cases
                    .into_iter()
                    .find_map(|(candidate, flags)| (candidate == *case).then_some(flags))
                    .ok_or_else(|| {
                        replay_error(function, "conditional state omits authenticated case")
                    })?;
                let flags = if flags.is_empty() {
                    let prefix = source.projected(case.clone()).projections;
                    validate_place(function, source, storage, leaves)?
                        .into_iter()
                        .filter(|flag| leaves[flag].place.projections.starts_with(&prefix))
                        .collect()
                } else {
                    flags
                };
                // A valid selected case may carry only Copy fields. Its authenticated case state is consumed even though there are no cleanup flags to
                // materialize.
                if !flags.is_empty() {
                    append_dead_flags(function, state, flags, "variant authentication")?;
                }
            }
            let prefix = source
                .projections
                .iter()
                .chain(std::iter::once(case))
                .cloned()
                .collect::<Vec<_>>();
            if validate_place(function, source, storage, leaves)?
                .iter()
                .any(|flag| {
                    state.live_order.contains(flag)
                        && !leaves[flag].place.projections.starts_with(&prefix)
                })
            {
                return Err(replay_error(
                    function,
                    "variant authentication retains a live inactive-case payload",
                )
                .into());
            }
        }
        CleanupTransition::CallCommit { call, arguments } => {
            let mut consumed = BTreeSet::new();
            let mut consumed_conditional_roots = BTreeSet::new();
            for argument in arguments {
                if let Some(index) = state
                    .conditional_variants
                    .iter()
                    .position(|variant| variant.root == argument.source)
                {
                    if !consumed_conditional_roots.insert(argument.source.clone()) {
                        return Err(E::structural(
                            function,
                            semantic_refusal::Atom::RepeatedVariantEpoch(call.clone()),
                        ));
                    }
                    let conditional = state.conditional_variants.remove(index);
                    for flag in conditional.cases.into_iter().flat_map(|(_, flags)| flags) {
                        if !consumed.insert(flag) {
                            return Err(E::structural(
                                function,
                                semantic_refusal::Atom::RepeatedCallFlag(call.clone(), flag),
                            ));
                        }
                    }
                    continue;
                }
                for flag in validate_place(function, &argument.source, storage, leaves)? {
                    if !state.live_order.contains(&flag) {
                        return Err(E::structural(
                            function,
                            semantic_refusal::Atom::DeadCallFlag(call.clone(), flag),
                        ));
                    }
                    if !consumed.insert(flag) {
                        return Err(E::structural(
                            function,
                            semantic_refusal::Atom::RepeatedCallFlag(call.clone(), flag),
                        ));
                    }
                }
            }
            for history in state.renewals.values_mut() {
                history.commit(&consumed);
            }
            state.live_order.retain(|flag| !consumed.contains(flag));
        }
        CleanupTransition::SelectFailure { source } => {
            state.renewals.clear();
            if state.selected_failure.is_some() {
                return Err(replay_error(
                    function,
                    "failure selection is not write-once during path replay",
                )
                .into());
            }
            if state.pending_failure.as_ref() != Some(source) {
                return Err(E::structural(
                    function,
                    semantic_refusal::Atom::MismatchedFailure(source.expression.clone()),
                ));
            }
            state.pending_failure = None;
            state.selected_failure = Some(source.clone());
        }
        CleanupTransition::StageCopyResult { source } => {
            if state.staged_copy_result.is_some() {
                return Err(replay_error(
                    function,
                    "Copy aggregate result is staged more than once on one path",
                )
                .into());
            }
            if !state.live_order.is_empty() || !state.conditional_variants.is_empty() {
                return Err(replay_error(
                    function,
                    "Copy aggregate result staging carries resource liveness",
                )
                .into());
            }
            state.staged_copy_result = Some(source.clone());
        }
    }
    Ok(())
}

pub(super) fn replay_transfer(
    function: &ResolvedFunction,
    state: &mut PathState,
    source: &CleanupPlace,
    destination: &CleanupPlace,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    if budget.selected() {
        return transition_storage::transfer(
            function,
            state,
            source,
            destination,
            storage,
            leaves,
            budget,
        );
    }
    let source_flags = validate_place(function, source, storage, leaves)?;
    let destination_flags = validate_place(function, destination, storage, leaves)?;
    let source_set = source_flags.iter().copied().collect::<BTreeSet<_>>();
    let destination_set = destination_flags.iter().copied().collect::<BTreeSet<_>>();
    if source_set.len() != destination_set.len() || !source_set.is_disjoint(&destination_set) {
        return Err(replay_error(
            function,
            "cleanup transfer does not have disjoint equal-size ownership epochs",
        ));
    }
    if let Some(index) = state
        .conditional_variants
        .iter()
        .position(|variant| variant.root == *source)
    {
        let mapping = transfer_mapping(function, source, destination, leaves)?;
        let variant = state.conditional_variants.remove(index);
        let cases = variant
            .cases
            .into_iter()
            .map(|(case, flags)| {
                flags
                    .into_iter()
                    .map(|flag| {
                        mapping.get(&flag).copied().ok_or_else(|| {
                            replay_error(function, "conditional cleanup transfer omits a case leaf")
                        })
                    })
                    .collect::<Result<Vec<_>, Diagnostic>>()
                    .map(|flags| (case, flags))
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        state.conditional_variants.push(ReplayConditionalVariant {
            root: destination.clone(),
            variant: variant.variant,
            cases,
        });
        return Ok(());
    }
    if source_set
        .iter()
        .any(|flag| !state.live_order.contains(flag))
    {
        return Err(replay_error(
            function,
            "cleanup transfer reads a dead source leaf",
        ));
    }
    if destination_set
        .iter()
        .any(|flag| state.live_order.contains(flag))
    {
        return Err(replay_error(
            function,
            "cleanup transfer initializes a live destination leaf",
        ));
    }

    let mapping = transfer_mapping(function, source, destination, leaves)?;
    // A whole-source transfer is a completed aggregate boundary, authenticated by the typed control skeleton. Filling every owned destination field is not:
    // later Copy initializers can still fail, so preserve their preceding field-initialization history until the constructor actually completes.
    let source_history = if source.projections.is_empty() {
        source_flags
    } else {
        state
            .live_order
            .iter()
            .filter(|flag| source_set.contains(flag))
            .copied()
            .collect::<Vec<_>>()
    };
    for history in state.renewals.values_mut() {
        history.stage(|flag| mapping.get(&flag).copied());
    }
    state.live_order.retain(|flag| !source_set.contains(flag));
    if destination.projections.is_empty() {
        state.live_order.extend(destination_flags);
    } else {
        for source_flag in source_history {
            state.live_order.push(mapping[&source_flag]);
        }
    }
    Ok(())
}

pub(super) fn find_resolved_expression<'a>(
    function: &'a ResolvedFunction,
    identity: &ExpressionId,
) -> Option<&'a ResolvedExpr> {
    let mut stack = function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
        .collect::<Vec<_>>();
    while let Some(expression) = stack.pop() {
        if expression.id == *identity {
            return Some(expression);
        }
        let mut index = 0;
        while let Some(child) = replay_expression_child(expression, index) {
            stack.push(child);
            index += 1;
        }
    }
    None
}

pub(super) fn transfer_mapping(
    function: &ResolvedFunction,
    source: &CleanupPlace,
    destination: &CleanupPlace,
    leaves: &Leaves,
) -> Result<BTreeMap<LivenessFlagId, LivenessFlagId>, Diagnostic> {
    let mut mapping = BTreeMap::new();
    let mut destinations = BTreeSet::new();
    for (source_flag, source_leaf) in leaves.iter().filter(|(_, leaf)| {
        leaf.place.storage == source.storage
            && leaf.place.projections.starts_with(&source.projections)
    }) {
        let relative = source_leaf
            .place
            .projections
            .strip_prefix(source.projections.as_slice())
            .ok_or_else(|| replay_error(function, "transfer source is not a projection prefix"))?;
        let projections = destination
            .projections
            .iter()
            .chain(relative)
            .cloned()
            .collect::<Vec<_>>();
        let Some((destination_flag, destination_leaf)) = leaves.iter().find(|(_, leaf)| {
            leaf.place.storage == destination.storage && leaf.place.projections == projections
        }) else {
            return Err(replay_error(
                function,
                "cleanup transfer leaf shapes do not correspond",
            ));
        };
        if source_leaf.lifecycle != destination_leaf.lifecycle
            || !destinations.insert(*destination_flag)
        {
            return Err(replay_error(
                function,
                "cleanup transfer changes a leaf lifecycle or aliases its destination",
            ));
        }
        mapping.insert(*source_flag, *destination_flag);
    }
    if mapping.is_empty() {
        return Err(replay_error(
            function,
            "cleanup transfer has no mapped ownership leaves",
        ));
    }
    Ok(mapping)
}

pub(super) fn append_dead_flags(
    function: &ResolvedFunction,
    state: &mut PathState,
    flags: Vec<LivenessFlagId>,
    operation: &str,
) -> Result<(), Diagnostic> {
    if flags.is_empty()
        || flags.iter().any(|flag| {
            state.live_order.contains(flag)
                || state.conditional_variants.iter().any(|variant| {
                    variant
                        .cases
                        .iter()
                        .any(|(_, case_flags)| case_flags.contains(flag))
                })
        })
    {
        return Err(replay_error(
            function,
            format_args!("{operation} initializes an empty or live cleanup place"),
        ));
    }
    state.live_order.extend(flags);
    Ok(())
}

pub(super) fn state_for_edge(
    function: &ResolvedFunction,
    mut state: PathState,
    condition: &EdgeCondition,
    contract_sources: &BTreeMap<ExpressionId, StatusSourceId>,
) -> Result<PathState, Diagnostic> {
    if state.pending_failure.is_some() || state.selected_failure.is_some() || state.published {
        return Err(replay_error(
            function,
            "conditional edge follows pending, selected, or published state",
        ));
    }
    match condition {
        EdgeCondition::Always => {}
        EdgeCondition::BooleanResult(expression, false) => {
            if let Some(source) = contract_sources.get(expression) {
                state.pending_failure = Some(source.clone());
            }
        }
        EdgeCondition::BooleanResult(_, true)
        | EdgeCondition::VariantCase { .. }
        | EdgeCondition::ArmSelected { .. }
        | EdgeCondition::StatusZero(_) => {}
        EdgeCondition::StatusNonzero(source) => {
            state.pending_failure = Some(source.clone());
        }
    }
    Ok(state)
}

#[allow(
    clippy::too_many_arguments,
    reason = "keep independent held authority and canonical replay inputs explicit"
)]
pub(super) fn replay_exit(
    function: &ResolvedFunction,
    exit: &ExitTarget,
    mut state: PathState,
    storage_regions: &BTreeMap<StorageId, CleanupRegionId>,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    extra_disposed: &BTreeSet<StorageId>,
    budget: &mut ReplayBudget,
) -> Result<ExitFlow, Diagnostic> {
    if budget.selected() {
        return transition_storage::exit(
            function,
            exit,
            state,
            storage_regions,
            storage,
            leaves,
            extra_disposed,
            budget,
        );
    }
    if state.published {
        return Err(replay_error(function, "cleanup exit follows publication"));
    }
    let leaving = exit.leaves_regions.iter().copied().collect::<BTreeSet<_>>();
    let protected_result = match &exit.continuation {
        ExitContinuation::CommitResult {
            source: CleanupResultSource::Owned { storage: result },
        } => validate_place(function, result, storage, leaves)?
            .into_iter()
            .collect::<BTreeSet<_>>(),
        _ => BTreeSet::new(),
    };
    let expected = state
        .live_order
        .iter()
        .rev()
        .filter_map(|flag| {
            let leaf = &leaves[flag];
            let region = storage_regions[&leaf.place.storage];
            ((leaving.contains(&region) || extra_disposed.contains(&leaf.place.storage))
                && !protected_result.contains(flag))
            .then_some(*flag)
        })
        .collect::<Vec<_>>();
    let mut expected_conditional = Vec::new();
    for variant in state.conditional_variants.iter().rev() {
        let region = storage_regions[&variant.root.storage];
        let protected = matches!(
            &exit.continuation,
            ExitContinuation::CommitResult {
                source: CleanupResultSource::Owned { storage },
            } if storage == &variant.root
        );
        if !(leaving.contains(&region) || extra_disposed.contains(&variant.root.storage))
            || protected
        {
            continue;
        }
        for (case, flags) in variant.cases.iter().rev() {
            for flag in flags.iter().rev() {
                expected_conditional.push((
                    *flag,
                    crate::cleanup_plan::VariantCaseGuard {
                        storage: variant.root.storage.clone(),
                        variant: variant.variant.clone(),
                        case: case.clone(),
                    },
                ));
            }
        }
    }
    let actual = exit
        .finalize_in_order
        .iter()
        .filter_map(|action| {
            state
                .live_order
                .contains(&action.guard_flag)
                .then_some(action.guard_flag)
        })
        .collect::<Vec<_>>();
    if actual != expected {
        return Err(replay_error(
            function,
            format_args!(
                "exit {} does not finalize its live region leaves in reverse initialization order",
                exit.id.0
            ),
        ));
    }
    let actual_conditional = exit
        .finalize_in_order
        .iter()
        .filter_map(|action| {
            action
                .active_case
                .as_ref()
                .map(|condition| (action.guard_flag, condition.clone()))
        })
        .collect::<Vec<_>>();
    if actual_conditional != expected_conditional {
        return Err(replay_error(
            function,
            "exit does not preserve exact conditional variant finalizers",
        ));
    }
    let finalized = expected.into_iter().collect::<BTreeSet<_>>();
    state.live_order.retain(|flag| !finalized.contains(flag));
    state.conditional_variants.retain(|variant| {
        !(leaving.contains(&storage_regions[&variant.root.storage])
            || extra_disposed.contains(&variant.root.storage))
            || matches!(
                &exit.continuation,
                ExitContinuation::CommitResult {
                    source: CleanupResultSource::Owned { storage },
                } if storage == &variant.root
            )
    });
    if state.live_order.iter().any(|flag| {
        let leaf = &leaves[flag];
        leaving.contains(&storage_regions[&leaf.place.storage]) && !protected_result.contains(flag)
    }) {
        return Err(replay_error(
            function,
            "cleanup exit retains a live non-result leaf in a region it leaves",
        ));
    }

    finish_exit(function, exit, state, storage, leaves, budget)
}

pub(super) fn finish_exit(
    function: &ResolvedFunction,
    exit: &ExitTarget,
    mut state: PathState,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    budget: &mut ReplayBudget,
) -> Result<ExitFlow, Diagnostic> {
    match &exit.continuation {
        ExitContinuation::Continue(edge) | ExitContinuation::LoopBreak { edge, .. } => {
            if state.pending_failure.is_some() || state.selected_failure.is_some() {
                return Err(replay_error(
                    function,
                    "normal cleanup continuation carries a failure state",
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
                return Err(replay_error(
                    function,
                    "loop latch carries a failure, staged-result or return state",
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
                return Err(replay_error(
                    function,
                    "function return carries a failure, staged-result or earlier return state",
                ));
            }
            state.returned = Some(match source {
                CleanupResultSource::Scalar { expression } => {
                    ReturnedSource::Scalar(expression.clone())
                }
                CleanupResultSource::Owned { storage: result } => {
                    let held = if state
                        .conditional_variants
                        .iter()
                        .any(|variant| variant.root == *result)
                    {
                        true
                    } else {
                        let flags = path_join::storage::validate_place(
                            function, result, storage, leaves, budget,
                        )?;
                        flags.iter().all(|flag| state.live_order.contains(flag))
                    };
                    if !held {
                        return Err(replay_error(
                            function,
                            "owned function return has an incomplete provisional result",
                        ));
                    }
                    budget.reserve_clone(function, result, "returned provisional place")?;
                    ReturnedSource::Owned(result.clone())
                }
            });
            Ok(ExitFlow::Edge(*edge, state))
        }
        ExitContinuation::ReturnFailure { source } => {
            if state.pending_failure.is_some()
                || state.selected_failure.as_ref() != Some(source)
                || !state.live_order.is_empty()
                || !state.conditional_variants.is_empty()
            {
                return Err(replay_error(
                    function,
                    "failure return changes status or retains live ownership",
                ));
            }
            Ok(ExitFlow::Terminal)
        }
        ExitContinuation::CommitResult { source } => {
            if state.pending_failure.is_some() || state.selected_failure.is_some() {
                return Err(replay_error(
                    function,
                    "result commit follows a pending or selected failure",
                ));
            }
            match (&state.returned, source) {
                (None, CleanupResultSource::Scalar { expression })
                    if expression != &function.body.id =>
                {
                    return Err(replay_error(
                        function,
                        "ordinary scalar result commit does not publish the function body",
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
                    return Err(replay_error(
                        function,
                        "return domain commits a result other than its handed-off source",
                    ));
                }
            }
            match source {
                CleanupResultSource::Scalar { .. } => {
                    if !state.live_order.is_empty() || !state.conditional_variants.is_empty() {
                        return Err(replay_error(
                            function,
                            "scalar result commits before non-result cleanup completes",
                        ));
                    }
                    if state.returned.is_none()
                        && !budget.selected()
                        && expression_has_try(&function.body)
                    {
                        let staged = state.staged_copy_result.take().ok_or_else(|| {
                            replay_error(
                                function,
                                "Copy Result commit has no authenticated staged producer",
                            )
                        })?;
                        validate_staged_target(function, &staged)?;
                    } else if state.staged_copy_result.is_some() {
                        return Err(replay_error(
                            function,
                            "non-try scalar result carries a staged Copy aggregate",
                        ));
                    }
                }
                CleanupResultSource::Owned { storage: result } => {
                    if let Some(index) = state
                        .conditional_variants
                        .iter()
                        .position(|variant| variant.root == *result)
                    {
                        if !state.live_order.is_empty() || state.conditional_variants.len() != 1 {
                            return Err(replay_error(
                                function,
                                "conditional owned result retains non-result ownership",
                            ));
                        }
                        state.conditional_variants.remove(index);
                    } else {
                        let result_flags = path_join::storage::validate_place(
                            function, result, storage, leaves, budget,
                        )?;
                        if result_flags
                            .iter()
                            .any(|flag| !state.live_order.contains(flag))
                            || state.live_order.len() != result_flags.len()
                        {
                            return Err(replay_error(
                            function,
                            "owned result commit has incomplete result or remaining non-result ownership",
                        ));
                        }
                        if budget.selected() {
                            state.live_order.clear();
                        } else {
                            let result_flags = result_flags.into_iter().collect::<BTreeSet<_>>();
                            state.live_order.retain(|flag| !result_flags.contains(flag));
                        }
                    }
                }
            }
            state.published = true;
            if !state.live_order.is_empty() || !state.conditional_variants.is_empty() {
                return Err(replay_error(
                    function,
                    "result publication leaves cleanup ownership live",
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
                return Err(replay_error(
                    function,
                    "unit return follows failure or incomplete cleanup",
                ));
            }
            state.published = true;
            Ok(ExitFlow::Terminal)
        }
    }
}

/// How one replayed exit continues the proof flow of its domain.
pub(super) enum ExitFlow {
    /// A published result or returned failure ends the path.
    Terminal,
    /// A CFG edge carries the settled state to its target block.
    Edge(EdgeId, PathState),
    /// A finite iteration reaches its latch with the settled state.
    Latch(PathState),
}

pub(super) fn require_normal_flow_state(
    function: &ResolvedFunction,
    state: &PathState,
    block: BlockId,
) -> Result<(), Diagnostic> {
    if state.pending_failure.is_none() && state.selected_failure.is_none() && !state.published {
        Ok(())
    } else {
        Err(replay_error(
            function,
            format_args!(
                "block {} continues ordinary control flow with failure or publication state",
                block.0
            ),
        ))
    }
}

pub(super) fn validate_staged_target(
    function: &ResolvedFunction,
    source: &StagedCopyResultSource,
) -> Result<(), Diagnostic> {
    let target = match source {
        StagedCopyResultSource::Body {
            expression,
            instance,
        } => {
            if expression != &function.body.id {
                return Err(replay_error(
                    function,
                    "body result staging references another expression",
                ));
            }
            instance
        }
        StagedCopyResultSource::TryResidual {
            target_instance, ..
        }
        | StagedCopyResultSource::TryOptionNone {
            target_instance, ..
        } => target_instance,
    };
    if target != &function.return_type {
        return Err(replay_error(
            function,
            "staged Copy result targets another concrete function result",
        ));
    }
    Ok(())
}
