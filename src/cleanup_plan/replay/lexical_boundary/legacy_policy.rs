//! Complete original boundary-check algorithm for the owning Diagnostic policy.
use super::{
    replay_error, BTreeMap, BTreeSet, BlockId, CleanupRegionId, CleanupResultSource, Diagnostic,
    ExitContinuation, Leaves, LexicalAuth, PathState, Policy, ReplayBudget, ResolvedFunction,
    ReturnedSource, StorageId,
};
impl Policy for Diagnostic {
    #[allow(clippy::too_many_arguments)]
    fn check(
        function: &ResolvedFunction,
        auth: &LexicalAuth,
        block: BlockId,
        state: &PathState,
        storage: &BTreeSet<StorageId>,
        storage_regions: &BTreeMap<StorageId, CleanupRegionId>,
        leaves: &Leaves,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        let Some(boundary) = auth.boundaries.get(&block) else {
            return Ok(());
        };
        let item = function
            .cleanup_plan
            .lexical_control
            .as_deref()
            .and_then(|control| control.execution_boundaries.get(boundary.index))
            .ok_or_else(|| replay_error(function, "execution boundary has no certificate"))?;
        if state.pending_failure.is_some() || state.selected_failure.is_some() || state.published {
            return Err(replay_error(
                function,
                "execution boundary is reached with failure or publication state",
            ));
        }
        let return_matches = match (&state.returned, boundary.returned) {
            (None, None) => true,
            (Some(returned), Some(id)) => match (
                returned,
                &function.cleanup_plan.exits[id.0 as usize].continuation,
            ) {
                (
                    ReturnedSource::Scalar(actual),
                    ExitContinuation::FunctionReturn {
                        source: CleanupResultSource::Scalar { expression },
                        ..
                    },
                ) => actual == expression,
                (
                    ReturnedSource::Owned(actual),
                    ExitContinuation::FunctionReturn {
                        source: CleanupResultSource::Owned { storage },
                        ..
                    },
                ) => actual == storage,
                _ => false,
            },
            _ => false,
        };
        if !return_matches {
            return Err(replay_error(
                function,
                "execution boundary carries another phase's returned result",
            ));
        }
        crate::cleanup_plan::replay::lexical_cert::storage::finalizer_actions(
            function,
            &item.finalize_in_order,
            storage,
            leaves,
            budget,
        )?;
        if budget.selected() {
            if !state.conditional_variants.is_empty()
                || item
                    .finalize_in_order
                    .iter()
                    .any(|action| action.active_case.is_some())
            {
                return Err(replay_error(
                function,
                "selected lexical boundary conditional ownership is outside the primitive profile",
            ));
            }
            let actual = item.finalize_in_order.iter().filter_map(|action| {
                state
                    .live_order
                    .contains(&action.guard_flag)
                    .then_some(action.guard_flag)
            });
            if actual.ne(state.live_order.iter().rev().copied())
                || item.finalize_in_order.len() != state.live_order.len()
            {
                return Err(replay_error(
                    function,
                    "execution boundary finalizers differ from the replayed live ownership contour",
                ));
            }
            return Ok(());
        }
        // Authenticate all region references before comparing either contour,
        // preserving the ordinary route's first-error order. Compare borrowed
        // case facts instead of allocating four vectors and cloning each guard.
        for variant in state.conditional_variants.iter().rev() {
            let _ = storage_regions.get(&variant.root.storage).ok_or_else(|| {
                replay_error(function, "execution boundary variant has no region")
            })?;
        }
        let expected = state.live_order.iter().rev().copied();
        let expected_conditional = state.conditional_variants.iter().rev().flat_map(|variant| {
            variant.cases.iter().rev().flat_map(move |(case, flags)| {
                flags
                    .iter()
                    .rev()
                    .map(move |flag| (*flag, &variant.root.storage, &variant.variant, case))
            })
        });
        let actual = item.finalize_in_order.iter().filter_map(|action| {
            state
                .live_order
                .contains(&action.guard_flag)
                .then_some(action.guard_flag)
        });
        let actual_conditional = item.finalize_in_order.iter().filter_map(|action| {
            action.active_case.as_ref().map(|condition| {
                (
                    action.guard_flag,
                    &condition.storage,
                    &condition.variant,
                    &condition.case,
                )
            })
        });
        // Every action must finalize a live leaf of this exact state: a stale
        // finalizer for a dead flag would double-release at a guard failure.
        let actions = item
            .finalize_in_order
            .iter()
            .filter(|action| action.active_case.is_none())
            .count();
        if actual.ne(expected)
            || actual_conditional.ne(expected_conditional)
            || actions != state.live_order.len()
        {
            return Err(replay_error(
                function,
                "execution boundary finalizers differ from the replayed live ownership contour",
            ));
        }
        Ok(())
    }
}
