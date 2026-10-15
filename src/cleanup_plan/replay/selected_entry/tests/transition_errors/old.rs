//! Independent saved transition error oracle; never runtime authority.
use super::*;
use crate::cleanup_plan::replay::transition_storage::transfer;
#[allow(
    clippy::too_many_arguments,
    reason = "Keep the independent receiver and observation controls explicit."
)]
pub(super) fn ordinary(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    transition: &CleanupTransition,
    state: &mut PathState,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    if budget.selected() {
        return selected(
            program, function, transition, state, storage, leaves, helpers, budget,
        );
    }
    if state.published {
        return Err(replay_error(
            function,
            "cleanup transition occurs after result publication",
        ));
    }
    if state.selected_failure.is_some()
        && !matches!(transition, CleanupTransition::SelectFailure { .. })
    {
        return Err(replay_error(
            function,
            "ordinary cleanup transition occurs after failure selection",
        ));
    }
    if state.pending_failure.is_some()
        && !matches!(transition, CleanupTransition::SelectFailure { .. })
    {
        return Err(replay_error(
            function,
            "operation failure is not selected before another transition",
        ));
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
                ));
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
                ));
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
                ));
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
                        return Err(replay_error(
                            function,
                            format!("call `{call}` atomically consumes a variant epoch twice"),
                        ));
                    }
                    let conditional = state.conditional_variants.remove(index);
                    for flag in conditional.cases.into_iter().flat_map(|(_, flags)| flags) {
                        if !consumed.insert(flag) {
                            return Err(replay_error(
                                function,
                                format!(
                                    "call `{call}` atomically consumes argument flag {} twice",
                                    flag.0
                                ),
                            ));
                        }
                    }
                    continue;
                }
                for flag in validate_place(function, &argument.source, storage, leaves)? {
                    if !state.live_order.contains(&flag) {
                        return Err(replay_error(
                            function,
                            format!(
                                "call `{call}` atomically consumes dead argument flag {}",
                                flag.0
                            ),
                        ));
                    }
                    if !consumed.insert(flag) {
                        return Err(replay_error(
                            function,
                            format!(
                                "call `{call}` atomically consumes argument flag {} twice",
                                flag.0
                            ),
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
                ));
            }
            if state.pending_failure.as_ref() != Some(source) {
                return Err(replay_error(
                    function,
                    format!(
                        "selected failure `{}` does not match the pending failing edge",
                        source.expression
                    ),
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
                ));
            }
            if !state.live_order.is_empty() || !state.conditional_variants.is_empty() {
                return Err(replay_error(
                    function,
                    "Copy aggregate result staging carries resource liveness",
                ));
            }
            state.staged_copy_result = Some(source.clone());
        }
    }
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "Keep the independent receiver and observation controls explicit."
)]
pub(super) fn selected(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    transition: &CleanupTransition,
    state: &mut PathState,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    if state.published {
        return Err(replay_error(
            function,
            "cleanup transition occurs after result publication",
        ));
    }
    if state.selected_failure.is_some()
        && !matches!(transition, CleanupTransition::SelectFailure { .. })
    {
        return Err(replay_error(
            function,
            "ordinary cleanup transition occurs after failure selection",
        ));
    }
    if state.pending_failure.is_some()
        && !matches!(transition, CleanupTransition::SelectFailure { .. })
    {
        return Err(replay_error(
            function,
            "operation failure is not selected before another transition",
        ));
    }
    if !state.conditional_variants.is_empty() {
        return Err(replay_error(
            function,
            "selected primitive replay has conditional aggregate state",
        ));
    }
    match transition {
        CleanupTransition::Initialize { destination, .. } => {
            let flags =
                path_join::storage::validate_place(function, destination, storage, leaves, budget)?;
            // append_dead_flags performs the same validity check before growth.
            if flags.is_empty() || flags.iter().any(|flag| state.live_order.contains(flag)) {
                return Err(replay_error(
                    function,
                    "initialize transition initializes an empty or live cleanup place",
                ));
            }
            budget.reserve_vec(
                function,
                &mut state.live_order,
                flags.len(),
                "initialized live history",
            )?;
            append_dead_flags(function, state, flags, "initialize transition")?;
        }
        CleanupTransition::ReserveRenewal { at, binding } => {
            renewal::reserve(
                program, function, at, binding, state, storage, leaves, helpers, budget,
            )?;
        }
        CleanupTransition::Renew {
            at,
            source,
            destination,
        } => {
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
            )?;
        }
        CleanupTransition::Transfer {
            at,
            source,
            destination,
        } => {
            renewal::reject_unmarked_finish(program, function, at, destination, helpers, budget)?;
            let append = strings::reserve_selected_append(
                program,
                function,
                at,
                source,
                destination,
                state,
                budget,
            )?;
            transfer(
                function,
                state,
                source,
                destination,
                storage,
                leaves,
                budget,
            )?;
            strings::publish_selected_append(function, append, state, budget)?;
        }
        CleanupTransition::CallCommit { call, arguments } => {
            let mut consumed = BTreeSet::new();
            for argument in arguments {
                for flag in path_join::storage::validate_place(
                    function,
                    &argument.source,
                    storage,
                    leaves,
                    budget,
                )? {
                    if !state.live_order.contains(&flag) {
                        return Err(replay_error(
                            function,
                            format!(
                                "call `{call}` atomically consumes dead argument flag {}",
                                flag.0
                            ),
                        ));
                    }
                    if consumed.contains(&flag) {
                        return Err(replay_error(
                            function,
                            format!(
                                "call `{call}` atomically consumes argument flag {} twice",
                                flag.0
                            ),
                        ));
                    }
                    budget.reserve_btree_node::<LivenessFlagId, ()>(
                        function,
                        "committed argument flags",
                    )?;
                    consumed.insert(flag);
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
                ));
            }
            if state.pending_failure.as_ref() != Some(source) {
                return Err(replay_error(
                    function,
                    format!(
                        "selected failure `{}` does not match the pending failing edge",
                        source.expression
                    ),
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
                ));
            }
            if !state.live_order.is_empty() {
                return Err(replay_error(
                    function,
                    "Copy aggregate result staging carries resource liveness",
                ));
            }
            budget.reserve_clone(function, source, "staged Copy producer")?;
            state.staged_copy_result = Some(source.clone());
        }
        CleanupTransition::InitializeVariant { .. }
        | CleanupTransition::TransferVariant { .. }
        | CleanupTransition::AuthenticateVariantCase { .. } => {
            return Err(replay_error(
                function,
                "selected primitive replay excludes conditional aggregate transitions",
            ));
        }
    }
    Ok(())
}
