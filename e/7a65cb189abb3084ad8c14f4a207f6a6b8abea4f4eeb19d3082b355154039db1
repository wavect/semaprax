//! Paid ordinary frontier controls and transport of the existing allocation.
use super::*;
use std::mem::size_of;

// These banks cover the new directory, selector, iterator, partial-Drop and
// call receiving roles before they are formed. The complete original backing
// and all original work remain separately charged by their existing owners.
pub(super) fn reserve_directory(
    function: &ResolvedFunction,
    count: usize,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    budget.reserve_array::<(
        &ResolvedFunction,
        usize,
        &mut ReplayBudget,
        Result<(), Diagnostic>,
        Vec<frontier::Incoming<PathState>>,
        &mut Vec<frontier::Incoming<PathState>>,
        fn() -> frontier::Incoming<PathState>,
    )>(function, 1, "ordinary frontier directory receiving")?;
    budget.reserve_array::<(
        frontier::Incoming<PathState>,
        frontier::Incoming<PathState>,
        frontier::Incoming<PathState>,
        &mut fn() -> frontier::Incoming<PathState>,
    )>(
        function,
        count,
        "ordinary frontier repeated default receiving",
    )
}

pub(super) fn reserve_block(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    // Charge every dynamic control group as well as the outer block. The
    // single incoming branch uses its block bank; a Many group pays again.
    budget.reserve_bytes(
        function,
        4 * size_of::<frontier::Incoming<PathState>>()
            + 3 * size_of::<frontier::Iter<'_, PathState>>()
            // Only Empty/One uses the frontier iterator in the join fold.
            // Pay both next calls, including exhaustion, and each forwarded
            // Option/receiver before forming them. Many uses the original
            // direct tree iterator and introduces no per-visit forwarding.
            + 4 * size_of::<Option<&Box<PathState>>>()
            + 4 * size_of::<&mut frontier::Iter<'_, PathState>>()
            + 2 * size_of::<bool>()
            + 2 * size_of::<&frontier::Incoming<PathState>>()
            + 2 * size_of::<&BTreeSet<Box<PathState>>>()
            + 3 * size_of::<std::option::IntoIter<Box<PathState>>>()
            + 8 * size_of::<Box<PathState>>()
            + 6 * size_of::<&Box<PathState>>()
            + 4 * size_of::<&PathState>()
            + 2 * size_of::<Result<(), Diagnostic>>()
            + 2 * size_of::<&ResolvedFunction>()
            + 2 * size_of::<&mut ReplayBudget>()
            + 4 * size_of::<usize>()
            + 4 * size_of::<bool>()
            + size_of::<std::iter::Empty<&PathState>>()
            + size_of::<BlockId>()
            + size_of::<(
                &ResolvedFunction,
                Box<PathState>,
                &EdgeCondition,
                &BTreeMap<ExpressionId, StatusSourceId>,
                &mut ReplayBudget,
                Result<Box<PathState>, Diagnostic>,
                &ExpressionId,
                &StatusSourceId,
                StatusSourceId,
                Option<&StatusSourceId>,
                bool,
            )>(),
        "ordinary frontier block and singleton receiving",
    )
}

pub(super) fn state_for_edge(
    function: &ResolvedFunction,
    mut state: Box<PathState>,
    condition: &EdgeCondition,
    contract_sources: &BTreeMap<ExpressionId, StatusSourceId>,
    budget: &mut ReplayBudget,
) -> Result<Box<PathState>, Diagnostic> {
    // The owning block/group bank precedes formation of this call and its
    // Box argument. A refused edge drops that same allocation.
    let _ = budget;
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
    clippy::borrowed_box,
    reason = "validate the exact retained Box without rebuilding a tree"
)]
pub(super) fn validate_singleton(
    function: &ResolvedFunction,
    state: &Box<PathState>,
    block: BlockId,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    // The complete original validator sees precisely the empty sorted prefix
    // of the singleton. It still validates the history union and every field.
    validate_join_compatibility(function, std::iter::empty(), state.as_ref(), block, budget)
}
