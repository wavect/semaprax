//! Complete frozen36 PLAN/HIR comparator, independently compiled.
use super::*;

pub(super) fn compare(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    mut actual: HistorySet,
    expected: &[SkeletonPath],
) -> Result<(), Diagnostic> {
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
        if path.terminal != flat.terminal {
            return Err(mismatch());
        }
    }
    Ok(())
}
