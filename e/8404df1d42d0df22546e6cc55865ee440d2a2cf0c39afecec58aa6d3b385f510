//! Local immutable plan histories. HIR and retained/Oracle replay stay flat.
use super::*;
use std::cmp::Ordering;
use std::mem::size_of;
use std::rc::Rc;

macro_rules! paid {
    ($budget:expr, $function:expr, $bytes:expr, $phase:literal $(,)?) => {{
        #[cfg(not(test))]
        {
            $budget.reserve_bytes($function, $bytes, $phase)
        }
        #[cfg(test)]
        {
            frontiers::before($phase);
            let comparison_ticket = crate::bounded_output::stage_debits::route_debits::comparison_before($phase, $bytes);
            let result = $budget.reserve_bytes($function, $bytes, $phase);
            crate::bounded_output::stage_debits::route_debits::comparison_after(comparison_ticket, result.is_ok());
            if result.is_ok() {
                frontiers::after();
            }
            result
        }
    }};
}
macro_rules! backing {
    ($budget:expr, $function:expr, $values:expr, $additional:expr, $phase:literal $(,)?) => {{
        #[cfg(not(test))]
        {
            $budget.reserve_vec($function, $values, $additional, $phase)
        }
        #[cfg(test)]
        {
            frontiers::before($phase);
            let result = $budget.reserve_vec($function, $values, $additional, $phase);
            if result.is_ok() {
                frontiers::after();
            }
            result
        }
    }};
}
macro_rules! history_queue {
    ($budget:expr, $function:expr, $queue:expr, $value:expr, $phase:literal $(,)?) => {{
        // The larger owned tuple actual and helper formal exist before the
        // generic queue callee can charge backing; acquire both here.
        paid!(
            $budget,
            $function,
            QUEUE_RECEIVERS,
            "ordinary plan history queue receivers"
        )?;
        #[cfg(not(test))]
        {
            skeleton_queue_push($budget, $function, $queue, $value, $phase)
        }
        #[cfg(test)]
        {
            frontiers::before($phase);
            let result = skeleton_queue_push($budget, $function, $queue, $value, $phase);
            if result.is_ok() {
                frontiers::after();
            }
            result
        }
    }};
}

macro_rules! history_path {
    ($budget:expr, $function:expr, $paths:expr, $value:expr, $phase:literal $(,)?) => {{
        paid!(
            $budget,
            $function,
            PATH_RECEIVERS,
            "ordinary plan history path receivers"
        )?;
        #[cfg(not(test))]
        {
            skeleton_push($budget, $function, $paths, $value, $phase)
        }
        #[cfg(test)]
        {
            frontiers::before($phase);
            let result = skeleton_push($budget, $function, $paths, $value, $phase);
            if result.is_ok() {
                frontiers::after();
            }
            result
        }
    }};
}

mod producer;
pub(super) use producer::produce;
#[cfg(test)]
mod before;
#[cfg(test)]
mod frontier_tests;
#[cfg(test)]
mod frontiers;
#[cfg(test)]
mod owned_tests;
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

// Exact new owned receiver types. The predecessor used
// `(BlockId, Vec<Rc<SkeletonObservation>>)` and `SkeletonPath` at these same
// actual/formal boundaries. Their work/payment calls remain independent;
// no spare scalar/segment controls are credited toward these wider headers.
const QUEUE_RECEIVERS: usize = 2 * size_of::<(BlockId, History)>() // owned actual + formal tuple
    + 2 * size_of::<&mut VecDeque<(BlockId, History)>>() // queue actual/formal
    + 2 * size_of::<&mut ReplayBudget>() + 2 * size_of::<&ResolvedFunction>()
    + 2 * size_of::<&str>() + size_of::<Result<(), Diagnostic>>();
const PATH_RECEIVERS: usize = 2 * size_of::<PlanPath>() // owned actual + formal path
    + 2 * size_of::<&mut Vec<PlanPath>>() // output actual/formal
    + 2 * size_of::<&mut ReplayBudget>() + 2 * size_of::<&ResolvedFunction>()
    + 2 * size_of::<&str>() + size_of::<Result<(), Diagnostic>>();

// Lexical call-frontier census: actual and formal references are distinct
// receiver roles; Result and its named successful receiver are also distinct.
pub(super) const SELECT_CALL_CONTROLS: usize = 2 * size_of::<&ResolvedFunction>() // caller actual + select formal function
    + 2 * size_of::<&mut ReplayBudget>() // actual + select formal budget
    + size_of::<Result<bool, Diagnostic>>() // returned selector Result
    + size_of::<bool>(); // root condition successful-value receiver
pub(super) const PRODUCE_CALL_CONTROLS: usize = 2 * size_of::<&ResolvedFunction>() // actual + producer formal function
    + 2 * size_of::<&mut ReplayBudget>() // actual + producer formal budget
    + size_of::<Result<HistorySet, Diagnostic>>() // producer return Result
    + size_of::<HistorySet>(); // root's actual binding before owned transfer
pub(super) const COMPARE_CALL_CONTROLS: usize = 2 * size_of::<&ResolvedFunction>() // actual + compare formal function
    + 2 * size_of::<&mut ReplayBudget>() // actual + compare formal budget
    + 2 * size_of::<HistorySet>() // moved argument + owned compare formal
    + 2 * size_of::<&[SkeletonPath]>() // expected actual slice + formal slice
    + size_of::<Result<(), Diagnostic>>(); // root return receiver

// Actual local aggregate/return, queue, loop and payment receivers. Nested
// backing is paid separately; moving a tail never refunds its original debit.
// The eligibility function is a real new boundary in both callers. Its
// immutable budget actual/formal and returned/received Boolean are not funded
// by branch controls or by unrelated payment receiver slots.
const ELIGIBLE_CALL_CONTROLS: usize = 2 * size_of::<&ResolvedFunction>() // actual and immutable function formal
    + 2 * size_of::<&ReplayBudget>() // actual shared reborrow and immutable formal
    + 5 * size_of::<bool>() // five predicate results inside eligible
    + size_of::<bool>() // eligible's returned Boolean
    + size_of::<bool>(); // caller's successful Boolean receiver
const PRODUCER_CONTROLS: usize = ELIGIBLE_CALL_CONTROLS
    + size_of::<bool>() // producer entry guard's negated eligibility result
    + size_of::<HistorySet>()
    + size_of::<Result<HistorySet, Diagnostic>>()
    + size_of::<VecDeque<(BlockId, History)>>()
    + size_of::<Option<(BlockId, History)>>()
    + size_of::<History>()
    + size_of::<PlanPath>()
    + size_of::<Result<(), Diagnostic>>()
    + 4 * size_of::<usize>() // max_depth, block index, root index, control price
    + 2 * size_of::<&ResolvedFunction>() // payment actual/formal; eligibility is separately typed above
    + 2 * size_of::<&mut ReplayBudget>() // payment actual/formal; eligible uses immutable references
    + 4 * size_of::<&str>(); // payment/work actual/formal phase receivers
const BRANCH_CONTROLS: usize = size_of::<Segment>()
    + size_of::<History>()
    + size_of::<Option<usize>>()
    + size_of::<[usize; 8]>() // old depth, new depth, index, arena len, max depth, tail len, addition, price
    + 6 * size_of::<Result<(), Diagnostic>>() // controls/work/arena/tail/zero/conversion Results
    + 6 * size_of::<&mut ReplayBudget>() // corresponding payment/work budget receivers
    + 6 * size_of::<&ResolvedFunction>() // corresponding live function receivers
    + 6 * size_of::<&str>() // corresponding static phase receivers
    + 3 * size_of::<&mut Vec<Segment>>() // arena reserve actual/formal + push receiver
    + size_of::<&Rc<SkeletonObservation>>() // observation conversion census receiver
    + 2 * size_of::<bool>(); // nonempty-tail predicate and depth/max comparison
const SELECT_CONTROLS: usize = ELIGIBLE_CALL_CONTROLS
    + size_of::<&ResolvedFunction>() // existing function's block query receiver
    + 4 * size_of::<bool>() // eligibility negation, tiny-length predicate, short-circuit condition, structural branch result
    + 2 * size_of::<usize>() // block length and scan-work amount
    + size_of::<Result<bool, Diagnostic>>() // selector's own returned Result
    + size_of::<Result<(), Diagnostic>>() // local payment/work Result
    + size_of::<&ResolvedFunction>() // local payment function receiver
    + size_of::<&mut ReplayBudget>() // local payment budget receiver
    + size_of::<&str>(); // local static payment/work phase receiver

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
                        arena[*first].depth.saturating_sub(self.links.len())
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
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Option<&'a SkeletonObservation>, Diagnostic> {
        budget.charge(function, 1, "ordinary plan history forward cursor step")?;
        while self.segment < self.links.len() {
            budget.charge(
                function,
                1,
                "ordinary plan history forward segment traversal",
            )?;
            let items = &arena[self.links[self.links.len() - self.segment - 1]].items;
            if let Some(value) = items.get(self.item) {
                self.item += 1;
                return Ok(Some(value));
            }
            self.segment += 1;
            self.item = 0;
        }
        let value = history.tail.get(self.item);
        if value.is_some() {
            self.item += 1;
        }
        Ok(value.map(|value| value.as_ref()))
    }
}

#[cfg(test)]
mod before_comparison;
mod comparison_owner;
pub(super) use comparison_owner::compare;

// Frozen HIR comparator inherits these original test-only lexical helpers.
#[cfg(test)]
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
#[cfg(test)]
const ORDER_CALL: usize = 4 * size_of::<&PlanPath>() // two actuals + two formals
    + 2 * size_of::<&[Segment]>() // arena actual/formal
    + 2 * size_of::<&mut [Cursor; 2]>() // cursor pair actual/formal
    + 2 * size_of::<&ResolvedFunction>() + 2 * size_of::<&mut ReplayBudget>()
    + size_of::<Result<Ordering, Diagnostic>>() + size_of::<Ordering>();
#[cfg(test)]
const SIFT_CALL: usize = 2 * size_of::<&mut [PlanPath]>() // paths actual/formal
    + 2 * size_of::<&[Segment]>() // arena actual/formal
    + 4 * size_of::<usize>() // root/end actuals + formals
    + 2 * size_of::<&mut [Cursor; 2]>() // cursor pair actual/formal
    + 2 * size_of::<&ResolvedFunction>() + 2 * size_of::<&mut ReplayBudget>()
    + size_of::<Result<(), Diagnostic>>();
#[cfg(test)]
const PREPARE_CALL: usize = 2 * size_of::<&mut Cursor>()
    + 2 * size_of::<&History>()
    + 2 * size_of::<&[Segment]>()
    + 2 * size_of::<&ResolvedFunction>()
    + 2 * size_of::<&mut ReplayBudget>()
    + size_of::<Result<(), Diagnostic>>();
#[cfg(test)]
const NEXT_CALL: usize = 2 * size_of::<&mut Cursor>()
    + 2 * size_of::<&History>() + 2 * size_of::<&[Segment]>()
    + 2 * size_of::<&ResolvedFunction>() + 2 * size_of::<&mut ReplayBudget>()
    + size_of::<Result<Option<&SkeletonObservation>, Diagnostic>>() // fallible forward step return
    + size_of::<Option<&SkeletonObservation>>(); // named successful item receiver

#[cfg(test)]
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
#[cfg(test)]
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

#[cfg(test)]
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
#[cfg(test)]
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

#[cfg(test)]
mod before_hir_compare;
