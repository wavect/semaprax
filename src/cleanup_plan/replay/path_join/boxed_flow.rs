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
    budget.reserve_bytes(
        function,
        size_of::<(
            &ResolvedFunction,
            usize,
            &mut ReplayBudget,
            Result<(), Diagnostic>,
            Vec<frontier::Incoming<PathState>>,
            &mut Vec<frontier::Incoming<PathState>>,
            fn() -> frontier::Incoming<PathState>,
        )>() + HELD_EDGE_TERMINAL,
        "ordinary frontier directory receiving",
    )?;
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

// Direct lexical producers remove the two forwarding Result wrappers and
// owned Box/Result<Box> edge boundary; every other original bank is retained.
pub(super) const BLOCK_CONTROLS: usize = 4 * size_of::<frontier::Incoming<PathState>>()
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
            + 2 * size_of::<&ResolvedFunction>()
            + 2 * size_of::<&mut ReplayBudget>()
            + 4 * size_of::<usize>()
            + 4 * size_of::<bool>()
            + size_of::<std::iter::Empty<&PathState>>()
            + size_of::<BlockId>()
            + size_of::<(
                &ResolvedFunction,
                &EdgeCondition,
                &BTreeMap<ExpressionId, StatusSourceId>,
                &mut ReplayBudget,
                &ExpressionId,
                &StatusSourceId,
                StatusSourceId,
                Option<&StatusSourceId>,
                bool,
            )>();

// One terminal local Diagnostic and its conversion operand, plus the actual
// and formal Box of explicit drop before converting the selected error.
pub(super) const HELD_EDGE_TERMINAL: usize =
    2 * size_of::<Diagnostic>() + 2 * size_of::<Box<PathState>>();

macro_rules! held_edge {
    ($function:expr, $state:ident, $condition:expr, $sources:ident) => {{
        if $state.pending_failure.is_some() || $state.selected_failure.is_some() || $state.published
        {
            let error = replay_error(
                $function,
                "conditional edge follows pending, selected, or published state",
            );
            // The removed callee dropped its owned Box before returning its
            // Diagnostic to the caller for E conversion. Keep that exact order.
            drop($state);
            return Err(error.into());
        }
        match $condition {
            EdgeCondition::Always => {}
            EdgeCondition::BooleanResult(expression, false) => {
                if let Some(source) = $sources.get(expression) {
                    $state.pending_failure = Some(source.clone());
                }
            }
            EdgeCondition::BooleanResult(_, true)
            | EdgeCondition::VariantCase { .. }
            | EdgeCondition::ArmSelected { .. }
            | EdgeCondition::StatusZero(_) => {}
            EdgeCondition::StatusNonzero(source) => {
                $state.pending_failure = Some(source.clone());
            }
        }
    }};
}
pub(super) use held_edge;
