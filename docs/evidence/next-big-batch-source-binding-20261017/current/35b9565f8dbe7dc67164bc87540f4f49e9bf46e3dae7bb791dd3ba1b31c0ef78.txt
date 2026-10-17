//! One comparison invocation owns sort and pair state. No nested order/sift calls.
use super::*;

#[derive(Clone, Copy)]
enum Step { Build, Drain, Sift, PairPrepare, PairNext, Siblings, Root }
struct ComparisonOwner {
    cursors: [Cursor; 2],
    step: Step,
    return_to: Step,
    after_pair: Step,
    len: usize,
    build: usize,
    drain: usize,
    root: usize,
    end: usize,
    child: usize,
    left: usize,
    right: usize,
    ordering: Ordering,
}

// Each listed bank names a distinct real lexical receiver. The owner and its
// sort/pair slots stay live for this one compare invocation; no per-call flag
// exempts any surviving call. Cursor prepare/next call tariffs remain separate.
const PAYMENT_HEADER: usize = 2 * size_of::<&ResolvedFunction>()
    + 2 * size_of::<&mut ReplayBudget>() + 2 * size_of::<&str>()
    + 2 * size_of::<usize>() + 2 * size_of::<Result<(), Diagnostic>>();
const PATH_INDEX: usize = 2 * size_of::<&Vec<PlanPath>>()
    + 2 * size_of::<usize>() + size_of::<&PlanPath>();
const PATH_SWAP: usize = 2 * size_of::<&mut Vec<PlanPath>>()
    + 3 * size_of::<&mut [PlanPath]>() + 4 * size_of::<usize>();
const PAIR_MATCH: usize = size_of::<(Option<&SkeletonObservation>, Option<&SkeletonObservation>)>()
    + 2 * size_of::<&SkeletonObservation>() // successful match bindings
    + 4 * size_of::<&SkeletonObservation>() // content cmp actual/formal pairs
    + 4 * size_of::<&SkeletonTerminal>() // terminal cmp actual/formal pairs
    + 2 * size_of::<Ordering>(); // two lexical cmp returns, received in owner
type FinalZip = std::iter::Zip<std::slice::Iter<'static, PlanPath>, std::slice::Iter<'static, SkeletonPath>>;
const FINAL_ITERATORS: usize = 2 * size_of::<std::slice::Iter<'static, PlanPath>>()
    + 2 * size_of::<std::slice::Iter<'static, SkeletonPath>>()
    + 2 * size_of::<FinalZip>()
    + 2 * size_of::<&Vec<PlanPath>>() + size_of::<&[PlanPath]>() // Vec deref actual/formal/return
    + 2 * size_of::<&[PlanPath]>() + 2 * size_of::<&[SkeletonPath]>();
const FINAL_NEXT: usize = 2 * size_of::<&mut FinalZip>()
    + 2 * size_of::<Option<(&PlanPath, &SkeletonPath)>>()
    + 2 * size_of::<(&PlanPath, &SkeletonPath)>()
    + size_of::<usize>() // flat index local, per path (also prepaid for sentinel)
    + 4 * size_of::<&SkeletonTerminal>() + size_of::<bool>();
const FLAT_MATCH: usize = size_of::<(Option<&SkeletonObservation>, Option<&Rc<SkeletonObservation>>)>()
    + size_of::<&SkeletonObservation>() + size_of::<&Rc<SkeletonObservation>>()
    + 4 * size_of::<&SkeletonObservation>() // equality actual/formal pair
    + 2 * size_of::<&Rc<SkeletonObservation>>() + size_of::<&SkeletonObservation>()
    + 2 * size_of::<&Observations>() + 3 * size_of::<&[Rc<SkeletonObservation>]>()
    + 2 * size_of::<usize>() + size_of::<Option<&Rc<SkeletonObservation>>>()
    + size_of::<bool>();
const SHARED_MATCH: usize = size_of::<(Option<&SkeletonObservation>, Option<&SkeletonObservation>)>()
    + 2 * size_of::<&SkeletonObservation>()
    + 4 * size_of::<&SkeletonObservation>() + size_of::<bool>();
// Prefix is_none and shared-HIR Cursor new/prepare/next retain their ORIGINAL
// HIR_QUERY_CONTROLS/CURSOR_STORAGE/CURSOR_CALL debits below, not this bank.
const LEN_QUERIES: usize = 2 * size_of::<&Vec<PlanPath>>() + size_of::<usize>()
    + 2 * size_of::<&[SkeletonPath]>() + size_of::<usize>();
const SCALAR_TRANSPORT: usize = size_of::<usize>() // initial build quotient
    + 2 * size_of::<usize>() // decremented build/drain values
    + 2 * size_of::<usize>() // leaf test subtraction/quotient
    + 2 * size_of::<usize>() // child multiplication/addition
    + 2 * size_of::<usize>() // child+1 guard and right-index assignment
    + size_of::<usize>() // sibling increment
    + size_of::<usize>() // final len binding
    + size_of::<usize>(); // final flat index increment
const MISMATCH_CALL: usize = 2 * size_of::<&ResolvedFunction>() + 2 * size_of::<&str>()
    + size_of::<Diagnostic>(); // direct helper actual/formal + Diagnostic return, no closure
const STATE_TESTS: usize = 9 * size_of::<bool>(); // build,drain,leaf2+OR,child,Eq,sibling,root
// Retained primitive caller slots, not dynamic compound payload credit:
// four charge sites + six original paid sites + three direct HIR reserve sites
// + two swap payment sites + one Zip-next payment site = sixteen. Each is a
// nonrecursive sequential Result<(), Diagnostic> receiving frame: success has
// no owned payload, failure leaves the invocation immediately with one owned
// Diagnostic. Function/budget places never change and no Result escapes or
// overlaps another activation of its lexical site. Distinct compound Option,
// tuple, comparison/index/swap payloads below are paid on EVERY execution.
// Two explicit backing sites have distinct prepaid primitive frames; no
// iterator, IterMut::next or Option<&mut Cursor> initializer remains.
// The first acquisition replaces the identical old compare-entry call and is
// already caller-received under COMPARE_CALL_CONTROLS; no new payment ABI.
const OWNER_CONTROLS: usize = size_of::<ComparisonOwner>()
    + size_of::<HistorySet>() // retained original owning input header
    + 16 * PAYMENT_HEADER // original thirteen, two swap sites and final Zip-next site
    + 2 * (PAYMENT_HEADER + 2 * size_of::<&mut Vec<usize>>()) // explicit two backing sites
    + FINAL_ITERATORS + size_of::<bool>() + 4 * MISMATCH_CALL
    + LEN_QUERIES + SCALAR_TRANSPORT + STATE_TESTS
    + 3 * size_of::<Step>(); // copied state transports at return/schedule sites

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


pub(in crate::cleanup_plan::replay) fn compare(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    mut actual: HistorySet,
    expected: &[SkeletonPath],
) -> Result<(), Diagnostic> {
    #[cfg(test)]
    if skeleton_work::enumerated_prefix::original_selected() {
        return before_hir_compare::compare(function, budget, actual, expected);
    }
    paid!(budget, function, OWNER_CONTROLS, "ordinary plan retained comparison owner")?;
    let mut owner = ComparisonOwner {
        cursors: [Cursor { links: Vec::new(), segment: 0, item: 0 }, Cursor { links: Vec::new(), segment: 0, item: 0 }],
        step: Step::Build, return_to: Step::Build, after_pair: Step::Root,
        len: actual.paths.len(), build: 0, drain: 0, root: 0, end: 0, child: 0,
        left: 0, right: 0, ordering: Ordering::Equal,
    };
    owner.build = owner.len / 2;
    owner.drain = owner.len;
    backing!(budget, function, &mut owner.cursors[0].links, actual.max_depth, "ordinary plan history cursor backing")?;
    backing!(budget, function, &mut owner.cursors[1].links, actual.max_depth, "ordinary plan history cursor backing")?;
    // These states replace the old two sift call sites and their two order
    // call sites. Each transition is the corresponding old call/return/loop
    // branch; no graph traversal or work request is added/removed.
    loop {
        match owner.step {
            Step::Build => {
                if owner.build == 0 { owner.step = Step::Drain; continue; }
                owner.build -= 1;
                owner.root = owner.build;
                owner.end = owner.len;
                owner.return_to = Step::Build;
                owner.step = Step::Sift;
            }
            Step::Drain => {
                if owner.drain <= 1 { break; }
                owner.drain -= 1;
                paid!(budget, function, PATH_SWAP, "ordinary plan retained drain swap receivers")?;
                actual.paths.swap(0, owner.drain);
                #[cfg(test)] frontiers::swapped();
                owner.root = 0;
                owner.end = owner.drain;
                owner.return_to = Step::Drain;
                owner.step = Step::Sift;
            }
            Step::Sift => {
                budget.charge(function, 1, "ordinary plan history sort traversal")?;
                if owner.end < 2 || owner.root > (owner.end - 2) / 2 {
                    owner.step = owner.return_to;
                    continue;
                }
                owner.child = owner.root * 2 + 1;
                if owner.child + 1 < owner.end {
                    owner.left = owner.child;
                    owner.right = owner.child + 1;
                    owner.after_pair = Step::Siblings;
                } else {
                    owner.left = owner.root;
                    owner.right = owner.child;
                    owner.after_pair = Step::Root;
                }
                owner.step = Step::PairPrepare;
            }
            Step::PairPrepare => {
                paid!(budget, function, PREPARE_CALL + PATH_INDEX, "ordinary plan history left cursor prepare")?;
                owner.cursors[0].prepare(&actual.paths[owner.left].history, &actual.arena, function, budget)?;
                paid!(budget, function, PREPARE_CALL + PATH_INDEX, "ordinary plan history right cursor prepare")?;
                owner.cursors[1].prepare(&actual.paths[owner.right].history, &actual.arena, function, budget)?;
                owner.step = Step::PairNext;
            }
            Step::PairNext => {
                budget.charge(function, 1, "ordinary plan history observation comparison")?;
                paid!(budget, function, 2 * NEXT_CALL + 4 * PATH_INDEX + PAIR_MATCH, "ordinary plan history paired cursor next")?;
                let left = owner.cursors[0].next(&actual.paths[owner.left].history, &actual.arena, function, budget)?;
                let right = owner.cursors[1].next(&actual.paths[owner.right].history, &actual.arena, function, budget)?;
                match (left, right) {
                    (Some(left), Some(right)) => {
                        owner.ordering = left.cmp(right);
                        if owner.ordering == Ordering::Equal { continue; }
                    }
                    (None, None) => owner.ordering = actual.paths[owner.left].terminal.cmp(&actual.paths[owner.right].terminal),
                    (None, Some(_)) => owner.ordering = Ordering::Less,
                    (Some(_), None) => owner.ordering = Ordering::Greater,
                }
                owner.step = owner.after_pair;
            }
            Step::Siblings => {
                if owner.ordering == Ordering::Less { owner.child += 1; }
                owner.left = owner.root;
                owner.right = owner.child;
                owner.after_pair = Step::Root;
                owner.step = Step::PairPrepare;
            }
            Step::Root => {
                if owner.ordering != Ordering::Less {
                    owner.step = owner.return_to;
                    continue;
                }
                paid!(budget, function, PATH_SWAP, "ordinary plan retained sift swap receivers")?;
                actual.paths.swap(owner.root, owner.child);
                #[cfg(test)] frontiers::swapped();
                owner.root = owner.child;
                owner.step = Step::Sift;
            }
        }
    }
    let len = owner.len;
    if len != expected.len() {
        return Err(replay_error(function, "cleanup CFG decision or ownership-event sequence disagrees with typed HIR"));
    }
    let mut pairs = actual.paths.iter().zip(expected);
    loop {
        paid!(budget, function, FINAL_NEXT, "ordinary plan retained final Zip next receivers")?;
        let Some((path, flat)) = pairs.next() else { break; };
        paid!(
            budget,
            function,
            PREPARE_CALL,
            "ordinary plan history HIR cursor prepare"
        )?;
        owner.cursors[0].prepare(&path.history, &actual.arena, function, budget)?;
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
                    NEXT_CALL + FLAT_MATCH,
                    "ordinary plan history HIR cursor next"
                )?;
                match (
                    owner.cursors[0].next(&path.history, &actual.arena, function, budget)?,
                    flat.observations.get(index),
                ) {
                    (Some(left), Some(right)) if left == right.as_ref() => {
                        index += 1;
                    }
                    (None, None) => break,
                    _ => return Err(replay_error(function, "cleanup CFG decision or ownership-event sequence disagrees with typed HIR")),
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
                    NEXT_CALL + SHARED_MATCH,
                    "ordinary plan history HIR cursor next"
                )?;
                budget.reserve_bytes(
                    function,
                    skeleton_work::enumerated_prefix::CURSOR_CALL,
                    "ordinary PLAN shared HIR cursor next receivers",
                )?;
                match (
                    owner.cursors[0].next(&path.history, &actual.arena, function, budget)?,
                    hir.next(function, budget)?,
                ) {
                    (Some(left), Some(right)) if left == right => {}
                    (None, None) => break,
                    _ => return Err(replay_error(function, "cleanup CFG decision or ownership-event sequence disagrees with typed HIR")),
                }
            }
        }
        if path.terminal != flat.terminal {
            return Err(replay_error(function, "cleanup CFG decision or ownership-event sequence disagrees with typed HIR"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
