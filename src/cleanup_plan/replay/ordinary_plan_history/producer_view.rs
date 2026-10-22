//! Fresh full-content PLAN loans over the existing immutable arena.
//!
//! Produced privately binds the real constructor result to its borrowed function.
//! Every operation takes the caller's actual budget reborrow; it is no lease or
//! cached execution authority. Callers prepay the exported call layouts before
//! forming their receivers, and use that same budget for the independent HIR side.
use super::super::*;

pub(in crate::cleanup_plan::replay::ordinary_plan_history) struct Produced<'function> {
    set: HistorySet,
    function: &'function ResolvedFunction,
}

pub(in crate::cleanup_plan::replay::ordinary_plan_history) struct PathCursor<'history> {
    path: &'history PlanPath,
    arena: &'history [Segment],
    function: &'history ResolvedFunction,
    cursor: Cursor,
}

// Actual/formal arguments, returned Result and named success bindings are
// distinct roles. Body payments below cover their internal calls and backing.
pub(in crate::cleanup_plan::replay::ordinary_plan_history) const PRODUCE_CALL: usize = 2
    * size_of::<&ResolvedFunction>()
    + 2 * size_of::<&mut ReplayBudget>()
    + size_of::<Result<Produced<'static>, Diagnostic>>()
    + size_of::<Produced<'static>>();
pub(in crate::cleanup_plan::replay::ordinary_plan_history) const COUNT_CALL: usize =
    2 * size_of::<&Produced<'static>>() + 2 * size_of::<usize>();
pub(in crate::cleanup_plan::replay::ordinary_plan_history) const PREPARE_CALL: usize = 2
    * size_of::<&Produced<'static>>()
    + 2 * size_of::<usize>()
    + 2 * size_of::<&mut ReplayBudget>()
    + size_of::<Result<PathCursor<'static>, Diagnostic>>()
    + size_of::<PathCursor<'static>>();
pub(in crate::cleanup_plan::replay::ordinary_plan_history) const NEXT_CALL: usize =
    2 * size_of::<&mut PathCursor<'static>>()
        + 2 * size_of::<&mut ReplayBudget>()
        + size_of::<Result<Option<&SkeletonObservation>, Diagnostic>>()
        + size_of::<Option<&SkeletonObservation>>();
pub(in crate::cleanup_plan::replay::ordinary_plan_history) const TERMINAL_CALL: usize =
    2 * size_of::<&PathCursor<'static>>() + 2 * size_of::<SkeletonTerminal>();
// Existing Cursor is shared verbatim; its old call invoices are test-only in
// the owner, so these production loans carry the same typed roles explicitly.
const CURSOR_PREPARE_CALL: usize = 2 * size_of::<&mut Cursor>()
    + 2 * size_of::<&History>()
    + 2 * size_of::<&[Segment]>()
    + 2 * size_of::<&ResolvedFunction>()
    + 2 * size_of::<&mut ReplayBudget>()
    + size_of::<Result<(), Diagnostic>>();
const CURSOR_NEXT_CALL: usize = 2 * size_of::<&mut Cursor>()
    + 2 * size_of::<&History>()
    + 2 * size_of::<&[Segment]>()
    + 2 * size_of::<&ResolvedFunction>()
    + 2 * size_of::<&mut ReplayBudget>()
    + size_of::<Result<Option<&SkeletonObservation>, Diagnostic>>()
    + size_of::<Option<&SkeletonObservation>>();
const PAYMENT: usize = 2 * size_of::<&mut ReplayBudget>()
    + 2 * size_of::<&ResolvedFunction>()
    + 2 * size_of::<usize>()
    + 2 * size_of::<&str>()
    + 2 * size_of::<Result<(), Diagnostic>>();
const PRODUCE_BODY: usize = PRODUCE_CALL_CONTROLS
    + 2 * size_of::<HistorySet>()
    + 2 * size_of::<Produced<'static>>()
    + size_of::<Result<Produced<'static>, Diagnostic>>()
    + 2 * PAYMENT;
const PREPARE_BODY: usize = 2 * size_of::<PathCursor<'static>>()
    + size_of::<Result<PathCursor<'static>, Diagnostic>>()
    + size_of::<Cursor>()
    + size_of::<Vec<usize>>()
    + size_of::<Option<&PlanPath>>()
    + 2 * size_of::<&HistorySet>()
    + 2 * size_of::<&PlanPath>()
    + 2 * size_of::<&[Segment]>()
    + ELIGIBLE_CALL_CONTROLS
    + 4 * size_of::<bool>()
    + 2 * size_of::<usize>()
    + 4 * PAYMENT;

pub(in crate::cleanup_plan::replay::ordinary_plan_history) fn produce<'function>(
    function: &'function ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Produced<'function>, Diagnostic> {
    paid!(
        budget,
        function,
        PRODUCE_BODY,
        "fresh PLAN view producer receivers"
    )?;
    let set = super::produce(function, budget)?;
    Ok(Produced { set, function })
}

pub(in crate::cleanup_plan::replay::ordinary_plan_history) fn path_count(
    produced: &Produced<'_>,
) -> usize {
    produced.set.paths.len()
}

pub(in crate::cleanup_plan::replay::ordinary_plan_history) fn prepare<'history>(
    produced: &'history Produced<'_>,
    index: usize,
    budget: &mut ReplayBudget,
) -> Result<PathCursor<'history>, Diagnostic> {
    let function = produced.function;
    paid!(
        budget,
        function,
        PREPARE_BODY,
        "fresh PLAN cursor receivers"
    )?;
    if !super::admission::eligible(function, budget) {
        return Err(replay_error(
            function,
            "fresh PLAN cursor requires its ordinary caller",
        ));
    }
    if produced.set.max_depth > produced.set.arena.len() {
        return Err(replay_error(
            function,
            "fresh PLAN arena depth exceeds its complete owner",
        ));
    }
    let path =
        produced.set.paths.get(index).ok_or_else(|| {
            replay_error(function, "fresh PLAN path is outside its complete owner")
        })?;
    // No recursive allocation, guessed depth threshold or reconstruction buffer.
    // The existing producer computes this exact maximum from every real branch.
    budget.reserve_array::<usize>(function, produced.set.max_depth, "fresh PLAN cursor links")?;
    let mut cursor = Cursor {
        links: Vec::with_capacity(produced.set.max_depth),
        segment: 0,
        item: 0,
    };
    paid!(
        budget,
        function,
        CURSOR_PREPARE_CALL,
        "fresh PLAN original prepare call"
    )?;
    cursor.prepare(&path.history, &produced.set.arena, function, budget)?;
    Ok(PathCursor {
        path,
        arena: &produced.set.arena,
        function,
        cursor,
    })
}

pub(in crate::cleanup_plan::replay::ordinary_plan_history) fn next<'history>(
    cursor: &mut PathCursor<'history>,
    budget: &mut ReplayBudget,
) -> Result<Option<&'history SkeletonObservation>, Diagnostic> {
    paid!(budget, cursor.function, PAYMENT, "fresh PLAN next receiver")?;
    paid!(
        budget,
        cursor.function,
        CURSOR_NEXT_CALL,
        "fresh PLAN original next call"
    )?;
    cursor
        .cursor
        .next(&cursor.path.history, cursor.arena, cursor.function, budget)
}

pub(in crate::cleanup_plan::replay::ordinary_plan_history) fn terminal(
    cursor: &PathCursor<'_>,
) -> SkeletonTerminal {
    cursor.path.terminal
}

#[cfg(test)]
#[path = "producer_view_tests.rs"]
mod tests;
