//! Local immutable plan histories. HIR and retained/Oracle replay stay flat.
use super::*;
use std::cmp::Ordering;
use std::mem::size_of;
use std::rc::Rc;
mod producer;
use producer::produce as produce_inner;
#[cfg(test)]
mod before;
#[cfg(test)]
pub(crate) mod source_custody;
#[cfg(test)]
mod tests;

struct Segment {
    previous: Option<usize>,
    items: Vec<Rc<SkeletonObservation>>,
    depth: usize,
}
struct History {
    previous: Option<usize>,
    tail: Vec<Rc<SkeletonObservation>>,
}
struct PlanPath {
    history: History,
    terminal: SkeletonTerminal,
}
pub(super) struct HistorySet {
    arena: Vec<Segment>,
    paths: Vec<PlanPath>,
    max_depth: usize,
}

// Actual local aggregate/return, queue, loop and payment receivers. Nested
// backing is paid separately; moving a tail never refunds its original debit.
const PRODUCER_CONTROLS: usize = size_of::<HistorySet>()
    + size_of::<Result<HistorySet, Diagnostic>>()
    + size_of::<VecDeque<(BlockId, History)>>()
    + size_of::<Option<(BlockId, History)>>()
    + size_of::<History>()
    + size_of::<PlanPath>()
    + size_of::<Result<(), Diagnostic>>()
    + 4 * size_of::<usize>()
    + 4 * size_of::<&ResolvedFunction>()
    + 4 * size_of::<&mut ReplayBudget>()
    + 4 * size_of::<&str>();
const BRANCH_CONTROLS: usize = size_of::<Segment>()
    + size_of::<History>()
    + size_of::<Option<usize>>()
    + 8 * size_of::<usize>()
    + 6 * size_of::<Result<(), Diagnostic>>()
    + 6 * size_of::<&mut ReplayBudget>()
    + 6 * size_of::<&ResolvedFunction>()
    + 6 * size_of::<&str>()
    + 3 * size_of::<&mut Vec<Segment>>()
    + size_of::<&Rc<SkeletonObservation>>()
    + 2 * size_of::<bool>();
const SELECT_CONTROLS: usize = 3 * size_of::<&ResolvedFunction>()
    + 6 * size_of::<bool>()
    + 2 * size_of::<usize>()
    + size_of::<Result<bool, Diagnostic>>()
    + size_of::<Result<(), Diagnostic>>()
    + size_of::<&ResolvedFunction>()
    + size_of::<&mut ReplayBudget>()
    + size_of::<&str>();

fn eligible(function: &ResolvedFunction, budget: &ReplayBudget) -> bool {
    budget.admission == AdmissionMode::Ordinary
        && budget.payer == AllocationPayer::ActiveBuilder
        && !budget.lent_skeleton
        && !budget.merge_paths
        && function.cleanup_plan.lexical_control.is_none()
}
pub(super) fn select(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    // The payer predicate is the original authority boundary; it does not
    // mutate or debit Selected or independently funded Oracle routes.
    if budget.payer != AllocationPayer::ActiveBuilder {
        return Ok(false);
    }
    budget.reserve_bytes(function, SELECT_CONTROLS, "ordinary plan history selection")?;
    budget.charge(function, 1, "ordinary plan history selection")?;
    // Tiny and single-path functions retain their full old producer. The
    // bounded scan has its own free-work debit, never the held skeleton loan.
    if !eligible(function, budget) || function.cleanup_plan.blocks.len() < 8 {
        return Ok(false);
    }
    budget.charge(
        function,
        function.cleanup_plan.blocks.len(),
        "ordinary plan history selection scan",
    )?;
    Ok(function.cleanup_plan.blocks.iter().any(
        |block| matches!(&block.terminator, CleanupTerminator::Branch(edges) if edges.len() > 1),
    ))
}
pub(super) fn produce(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<HistorySet, Diagnostic> {
    budget.reserve_bytes(
        function,
        size_of::<Result<HistorySet, Diagnostic>>() + size_of::<HistorySet>(),
        "ordinary plan history caller receivers",
    )?;
    produce_inner(function, budget)
}

struct Cursor {
    links: Vec<usize>,
    segment: usize,
    item: usize,
}
impl Cursor {
    fn prepare(
        &mut self,
        history: &History,
        arena: &[Segment],
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        self.links.clear();
        self.segment = 0;
        self.item = 0;
        let mut previous = history.previous;
        while let Some(index) = previous {
            budget.charge(function, 1, "ordinary plan history cursor traversal")?;
            let segment = arena
                .get(index)
                .ok_or_else(|| replay_error(function, "ordinary plan history link is invalid"))?;
            if self.links.len() == self.links.capacity()
                || segment.previous.is_some_and(|parent| parent >= index)
                || segment.depth
                    != self.links.first().map_or(segment.depth, |first| {
                        arena[*first]
                            .depth
                            .checked_sub(self.links.len())
                            .unwrap_or(0)
                    })
                || segment.depth == 0
            {
                return Err(replay_error(
                    function,
                    "ordinary plan history depth is invalid",
                ));
            }
            self.links.push(index);
            previous = segment.previous;
        }
        if let Some(first) = self.links.first() {
            if arena[*first].depth != self.links.len() {
                return Err(replay_error(
                    function,
                    "ordinary plan history depth is invalid",
                ));
            }
        }
        Ok(())
    }
    fn next<'a>(
        &mut self,
        history: &'a History,
        arena: &'a [Segment],
    ) -> Option<&'a SkeletonObservation> {
        while self.segment < self.links.len() {
            let items = &arena[self.links[self.links.len() - self.segment - 1]].items;
            if let Some(value) = items.get(self.item) {
                self.item += 1;
                return Some(value);
            }
            self.segment += 1;
            self.item = 0;
        }
        let value = history.tail.get(self.item)?;
        self.item += 1;
        Some(value)
    }
}

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

fn order(
    a: &PlanPath,
    b: &PlanPath,
    arena: &[Segment],
    cursors: &mut [Cursor; 2],
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Ordering, Diagnostic> {
    budget.reserve_bytes(
        function,
        COMPARE_CONTROLS,
        "ordinary plan history comparison receivers",
    )?;
    cursors[0].prepare(&a.history, arena, function, budget)?;
    cursors[1].prepare(&b.history, arena, function, budget)?;
    loop {
        budget.charge(function, 1, "ordinary plan history observation comparison")?;
        let left = cursors[0].next(&a.history, arena);
        let right = cursors[1].next(&b.history, arena);
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
    budget.reserve_bytes(
        function,
        COMPARE_CONTROLS,
        "ordinary plan history sort receivers",
    )?;
    loop {
        budget.charge(function, 1, "ordinary plan history sort traversal")?;
        // A root with no children exits before multiplication can overflow.
        if end < 2 || root > (end - 2) / 2 {
            return Ok(());
        }
        let mut child = root * 2 + 1;
        if child + 1 < end
            && order(
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
        if order(
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
        root = child;
    }
}

pub(super) fn compare(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    mut actual: HistorySet,
    expected: &[SkeletonPath],
) -> Result<(), Diagnostic> {
    budget.reserve_bytes(
        function,
        COMPARE_CONTROLS + size_of::<HistorySet>(),
        "ordinary plan history comparator controls",
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
        budget.reserve_vec(
            function,
            &mut cursor.links,
            actual.max_depth,
            "ordinary plan history cursor backing",
        )?;
    }
    let len = actual.paths.len();
    for root in (0..len / 2).rev() {
        sift(
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
        sift(
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
        cursors[0].prepare(&path.history, &actual.arena, function, budget)?;
        let mut index = 0;
        loop {
            budget.charge(function, 1, "ordinary plan history HIR comparison")?;
            match (
                cursors[0].next(&path.history, &actual.arena),
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
