//! Complete immediate comparator and original tariffs; no candidate calls.
use super::*;

const COMPARE_CONTROLS: usize = 2 * size_of::<Cursor>()
    + 2 * size_of::<Result<(), Diagnostic>>()
    + size_of::<Result<Ordering, Diagnostic>>()
    + 2 * size_of::<Option<&SkeletonObservation>>()
    + 20 * size_of::<usize>()
    + 8 * size_of::<bool>()
    + 8 * size_of::<&ResolvedFunction>()
    + 8 * size_of::<&mut ReplayBudget>()
    + 8 * size_of::<&str>()
    + 6 * size_of::<&PlanPath>()
    + 4 * size_of::<&History>()
    + 4 * size_of::<&[Segment]>()
    + 2 * size_of::<&mut Cursor>()
    + size_of::<std::slice::Iter<'static, SkeletonPath>>()
    + size_of::<Option<usize>>()
    + size_of::<Ordering>();

// Caller-paid frames: all actual + formal reference receivers, scalar
// arguments, returned Results and named successful-value receivers.
const ORDER_CALL: usize = 4 * size_of::<&PlanPath>() // two actuals + two formals
    + 2 * size_of::<&[Segment]>() // arena actual/formal
    + 2 * size_of::<&mut [Cursor; 2]>() // cursor pair actual/formal
    + 2 * size_of::<&ResolvedFunction>() + 2 * size_of::<&mut ReplayBudget>()
    + size_of::<Result<Ordering, Diagnostic>>() + size_of::<Ordering>();
const SIFT_CALL: usize = 2 * size_of::<&mut [PlanPath]>() // paths actual/formal
    + 2 * size_of::<&[Segment]>() // arena actual/formal
    + 4 * size_of::<usize>() // root/end actuals + formals
    + 2 * size_of::<&mut [Cursor; 2]>() // cursor pair actual/formal
    + 2 * size_of::<&ResolvedFunction>() + 2 * size_of::<&mut ReplayBudget>()
    + size_of::<Result<(), Diagnostic>>();
const PREPARE_CALL: usize = 2 * size_of::<&mut Cursor>()
    + 2 * size_of::<&History>()
    + 2 * size_of::<&[Segment]>()
    + 2 * size_of::<&ResolvedFunction>()
    + 2 * size_of::<&mut ReplayBudget>()
    + size_of::<Result<(), Diagnostic>>();
const NEXT_CALL: usize = 2 * size_of::<&mut Cursor>()
    + 2 * size_of::<&History>() + 2 * size_of::<&[Segment]>()
    + 2 * size_of::<&ResolvedFunction>() + 2 * size_of::<&mut ReplayBudget>()
    + size_of::<Result<Option<&SkeletonObservation>, Diagnostic>>() // fallible forward step return
    + size_of::<Option<&SkeletonObservation>>(); // named successful item receiver

macro_rules! paid_order {
    ($a:expr, $b:expr, $arena:expr, $cursors:expr, $function:expr, $budget:expr $(,)?) => {{
        paid!(
            $budget,
            $function,
            ORDER_CALL,
            "ordinary plan history order call"
        )?;
        order($a, $b, $arena, $cursors, $function, $budget)
    }};
}
macro_rules! paid_sift {
    ($paths:expr, $arena:expr, $root:expr, $end:expr, $cursors:expr, $function:expr, $budget:expr $(,)?) => {{
        paid!(
            $budget,
            $function,
            SIFT_CALL,
            "ordinary plan history sift call"
        )?;
        sift($paths, $arena, $root, $end, $cursors, $function, $budget)
    }};
}

fn order(
    a: &PlanPath,
    b: &PlanPath,
    arena: &[Segment],
    cursors: &mut [Cursor; 2],
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Ordering, Diagnostic> {
    paid!(
        budget,
        function,
        COMPARE_CONTROLS,
        "ordinary plan history comparison receivers"
    )?;
    paid!(
        budget,
        function,
        PREPARE_CALL,
        "ordinary plan history left cursor prepare"
    )?;
    cursors[0].prepare(&a.history, arena, function, budget)?;
    paid!(
        budget,
        function,
        PREPARE_CALL,
        "ordinary plan history right cursor prepare"
    )?;
    cursors[1].prepare(&b.history, arena, function, budget)?;
    loop {
        budget.charge(function, 1, "ordinary plan history observation comparison")?;
        paid!(
            budget,
            function,
            2 * NEXT_CALL,
            "ordinary plan history paired cursor next"
        )?;
        let left = cursors[0].next(&a.history, arena, function, budget)?;
        let right = cursors[1].next(&b.history, arena, function, budget)?;
        match (left, right) {
            (Some(left), Some(right)) => {
                let cmp = left.cmp(right);
                if cmp != Ordering::Equal {
                    return Ok(cmp);
                }
            }
            (None, None) => return Ok(a.terminal.cmp(&b.terminal)),
            (None, Some(_)) => return Ok(Ordering::Less),
            (Some(_), None) => return Ok(Ordering::Greater),
        }
    }
}

// A genuinely fallible heapsort: no fabricated equality after paid refusal.
fn sift(
    paths: &mut [PlanPath],
    arena: &[Segment],
    mut root: usize,
    end: usize,
    cursors: &mut [Cursor; 2],
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    paid!(
        budget,
        function,
        COMPARE_CONTROLS,
        "ordinary plan history sort receivers"
    )?;
    loop {
        budget.charge(function, 1, "ordinary plan history sort traversal")?;
        // A root with no children exits before multiplication can overflow.
        if end < 2 || root > (end - 2) / 2 {
            return Ok(());
        }
        let mut child = root * 2 + 1;
        if child + 1 < end
            && paid_order!(
                &paths[child],
                &paths[child + 1],
                arena,
                cursors,
                function,
                budget,
            )? == Ordering::Less
        {
            child += 1;
        }
        if paid_order!(
            &paths[root],
            &paths[child],
            arena,
            cursors,
            function,
            budget,
        )? != Ordering::Less
        {
            return Ok(());
        }
        paths.swap(root, child);
        #[cfg(test)]
        frontiers::swapped();
        root = child;
    }
}

pub(super) fn compare(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    mut actual: HistorySet,
    expected: &[SkeletonPath],
) -> Result<(), Diagnostic> {
    #[cfg(test)]
    if skeleton_work::enumerated_prefix::original_selected() {
        return before_hir_compare::compare(function, budget, actual, expected);
    }
    paid!(
        budget,
        function,
        COMPARE_CONTROLS + size_of::<HistorySet>(),
        "ordinary plan history comparator controls"
    )?;
    let mut cursors = [
        Cursor {
            links: Vec::new(),
            segment: 0,
            item: 0,
        },
        Cursor {
            links: Vec::new(),
            segment: 0,
            item: 0,
        },
    ];
    for cursor in &mut cursors {
        backing!(
            budget,
            function,
            &mut cursor.links,
            actual.max_depth,
            "ordinary plan history cursor backing"
        )?;
    }
    let len = actual.paths.len();
    for root in (0..len / 2).rev() {
        paid_sift!(
            &mut actual.paths,
            &actual.arena,
            root,
            len,
            &mut cursors,
            function,
            budget,
        )?;
    }
    for end in (1..len).rev() {
        actual.paths.swap(0, end);
        #[cfg(test)]
        frontiers::swapped();
        paid_sift!(
            &mut actual.paths,
            &actual.arena,
            0,
            end,
            &mut cursors,
            function,
            budget,
        )?;
    }
    let mismatch = || {
        replay_error(
            function,
            "cleanup CFG decision or ownership-event sequence disagrees with typed HIR",
        )
    };
    if len != expected.len() {
        return Err(mismatch());
    }
    for (path, flat) in actual.paths.iter().zip(expected) {
        paid!(
            budget,
            function,
            PREPARE_CALL,
            "ordinary plan history HIR cursor prepare"
        )?;
        cursors[0].prepare(&path.history, &actual.arena, function, budget)?;
        budget.reserve_bytes(
            function,
            skeleton_work::enumerated_prefix::HIR_QUERY_CONTROLS,
            "ordinary PLAN shared HIR selector receivers",
        )?;
        if flat.observations.prefix.is_none() {
            let mut index = 0;
            loop {
                budget.charge(function, 1, "ordinary plan history HIR comparison")?;
                paid!(
                    budget,
                    function,
                    NEXT_CALL,
                    "ordinary plan history HIR cursor next"
                )?;
                match (
                    cursors[0].next(&path.history, &actual.arena, function, budget)?,
                    flat.observations.get(index),
                ) {
                    (Some(left), Some(right)) if left == right.as_ref() => {
                        index += 1;
                    }
                    (None, None) => break,
                    _ => return Err(mismatch()),
                }
            }
        } else {
            budget.reserve_bytes(
                function,
                skeleton_work::enumerated_prefix::CURSOR_STORAGE
                    + skeleton_work::enumerated_prefix::CURSOR_CALL,
                "ordinary PLAN shared HIR cursor preparation receivers",
            )?;
            let mut hir = skeleton_work::enumerated_prefix::Cursor::new();
            hir.prepare(&flat.observations, function, budget)?;
            loop {
                budget.charge(function, 1, "ordinary plan history HIR comparison")?;
                paid!(
                    budget,
                    function,
                    NEXT_CALL,
                    "ordinary plan history HIR cursor next"
                )?;
                budget.reserve_bytes(
                    function,
                    skeleton_work::enumerated_prefix::CURSOR_CALL,
                    "ordinary PLAN shared HIR cursor next receivers",
                )?;
                match (
                    cursors[0].next(&path.history, &actual.arena, function, budget)?,
                    hir.next(function, budget)?,
                ) {
                    (Some(left), Some(right)) if left == right => {}
                    (None, None) => break,
                    _ => return Err(mismatch()),
                }
            }
        }
        if path.terminal != flat.terminal {
            return Err(mismatch());
        }
    }
    Ok(())
}
