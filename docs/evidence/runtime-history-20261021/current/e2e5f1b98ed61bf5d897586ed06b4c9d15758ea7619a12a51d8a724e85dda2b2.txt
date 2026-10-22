//! One comparison invocation owns sort and pair state. No nested order/sift calls.
#![cfg_attr(
    test,
    allow(
        clippy::too_many_arguments,
        clippy::manual_bits,
        clippy::items_after_test_module,
        clippy::assertions_on_constants,
        reason = "Historical comparisons retain complete parameter matrices, byte-role invoices (not bit counts), source order, and constant negative-path assertions."
    )
)]
use super::*;
#[allow(
    unused_macros,
    reason = "complete immediate final Option producer remains a frozen control"
)]
#[macro_use]
mod lexical_cursor;
#[macro_use]
mod final_affine;
#[cfg(test)]
#[rustfmt::skip]
mod before_final_affine;
#[macro_use]
mod paired_value;
#[allow(
    unused_macros,
    reason = "complete immediate suffix macro remains available to frozen generations"
)]
#[macro_use]
mod shared_prefix;
#[macro_use]
mod certified_heads;
#[cfg(test)]
#[rustfmt::skip]
mod before_empty_selector;
#[cfg(test)]
#[rustfmt::skip]
mod before_paired_tail;
#[cfg(test)]
#[rustfmt::skip]
mod before_main_b692;
#[cfg(test)]
#[rustfmt::skip]
mod before_main487;
#[cfg(test)]
#[rustfmt::skip]
mod before_pair_links;
#[cfg(test)]
#[rustfmt::skip]
mod before_main001;
#[cfg(test)]
#[rustfmt::skip]
mod before_certified_heads;
#[cfg(test)]
mod before_event_prefix;
#[cfg(test)]
mod before_shared_steps;
#[cfg(test)]
#[rustfmt::skip]
mod before_shared_runs;
#[cfg(test)]
#[allow(unused_imports, reason = "complete frozen generation retains its exact original namespace imports")]
#[rustfmt::skip]
mod before_reachable_primitives;
#[cfg(test)]
mod before_flat_view;
#[cfg(test)]
mod before_fused_flat;
#[cfg(test)]
mod before_identity_prefix;
#[cfg(test)]
mod before_lexical_cursor;
#[cfg(test)]
mod before_pair_scope;
#[cfg(test)]
#[rustfmt::skip]
mod before_prepared_pairing;
#[cfg(test)]
mod before_retained_tail;
#[cfg(test)]
pub(super) mod pair_scope_census;

#[derive(Clone, Copy)]
enum Step {
    Build,
    Drain,
    Sift,
    PairPrepare,
    PairNext,
    Siblings,
    Root,
}
struct ComparisonOwner<'history> {
    arena: &'history [Segment],
    left_items: &'history [Rc<SkeletonObservation>],
    right_items: &'history [Rc<SkeletonObservation>],
    left_loaded: bool,
    right_loaded: bool,
    tail_mask: u8,
    left_cursor: Cursor,
    right_cursor: Cursor,
    left_segments: usize,
    right_segments: usize,
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
// exempts any surviving call. Prepare remains a real independently paid call;
// lexical PLAN next pays all remaining actual dynamic payloads separately.
const PAYMENT_HEADER: usize = 2 * size_of::<&ResolvedFunction>()
    + 2 * size_of::<&mut ReplayBudget>()
    + 2 * size_of::<&str>()
    + 2 * size_of::<usize>()
    + 2 * size_of::<Result<(), Diagnostic>>();
const PATH_INDEX: usize =
    2 * size_of::<&Vec<PlanPath>>() + 2 * size_of::<usize>() + size_of::<&PlanPath>();
// Two original index call/return tariffs plus two distinct local receiving refs.
const PAIR_SCOPE: usize = 2 * PATH_INDEX + 2 * size_of::<&PlanPath>();
// Scope-reserve and additional inner-loop work sites: complete primitive frames.
const PAIR_PRIMITIVE: usize = 2 * PAYMENT_HEADER;
const PATH_SWAP: usize =
    2 * size_of::<&mut Vec<PlanPath>>() + 3 * size_of::<&mut [PlanPath]>() + 4 * size_of::<usize>();
// Both ordinary and certified fallback publish content through genuine main487
// Rc loans. The left producer's legacy head/as_ref envelope remains paid.
const PAIR_MATCH: usize = paired_value::PAIR_CONTENT;
#[cfg(any(test, feature = "unstable-workflow-profiling"))]
const NEW_PAIR_REFERENCES: usize = 2 * size_of::<&Rc<SkeletonObservation>>();
type FinalZip =
    std::iter::Zip<std::slice::Iter<'static, PlanPath>, std::slice::Iter<'static, SkeletonPath>>;
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
#[cfg(test)]
const FLAT_MATCH: usize = size_of::<(Option<&SkeletonObservation>, Option<&Rc<SkeletonObservation>>)>()
    + size_of::<&SkeletonObservation>() + size_of::<&Rc<SkeletonObservation>>()
    + 4 * size_of::<&SkeletonObservation>() // equality actual/formal pair
    + 2 * size_of::<&Rc<SkeletonObservation>>() + size_of::<&SkeletonObservation>()
    + 2 * size_of::<&[Rc<SkeletonObservation>]>() // retained slice get actual/formal
    + 2 * size_of::<usize>() + size_of::<Option<&Rc<SkeletonObservation>>>()
    + size_of::<bool>();
// Borrow once per flat path: Vec::as_slice actual/formal, returned slice and
// its distinct retained receiver. Dynamic get/Option/match payloads stay above.
const FLAT_VIEW: usize =
    2 * size_of::<&Vec<Rc<SkeletonObservation>>>() + 2 * size_of::<&[Rc<SkeletonObservation>]>();
#[cfg(test)]
const SHARED_MATCH: usize =
    size_of::<(Option<&SkeletonObservation>, Option<&SkeletonObservation>)>()
        + 2 * size_of::<&SkeletonObservation>()
        + 4 * size_of::<&SkeletonObservation>()
        + size_of::<bool>();
// Prefix is_none and shared-HIR Cursor new/prepare/next retain their ORIGINAL
// HIR_QUERY_CONTROLS/CURSOR_STORAGE/CURSOR_CALL debits below, not this bank.
const LEN_QUERIES: usize = 2 * size_of::<&Vec<PlanPath>>()
    + size_of::<usize>()
    + 2 * size_of::<&[SkeletonPath]>()
    + size_of::<usize>();
const SCALAR_TRANSPORT: usize = size_of::<usize>() // initial build quotient
    + 2 * size_of::<usize>() // decremented build/drain values
    + 2 * size_of::<usize>() // leaf test subtraction/quotient
    + 2 * size_of::<usize>() // child multiplication/addition
    + 2 * size_of::<usize>() // child+1 guard and right-index assignment
    + size_of::<usize>() // sibling increment
    + size_of::<usize>() // final len binding
    + size_of::<usize>(); // final flat index increment
const MISMATCH_CALL: usize =
    2 * size_of::<&ResolvedFunction>() + 2 * size_of::<&str>() + size_of::<Diagnostic>(); // direct helper actual/formal + Diagnostic return, no closure
const STATE_TESTS: usize = 9 * size_of::<bool>(); // build,drain,leaf2+OR,child,Eq,sibling,root
                                                  // Retained primitive caller slots, not dynamic compound payload credit:
                                                  // four charge sites + six original paid sites + three direct HIR reserve sites
                                                  // + two swap payment sites + one Zip-next payment site = sixteen original sites.
                                                  // The new flat-view payment adds a seventeenth site. Each is a
                                                  // nonrecursive sequential Result<(), Diagnostic> receiving frame: success has
                                                  // no owned payload, failure leaves the invocation immediately with one owned
                                                  // Diagnostic. Function/budget places never change and no Result escapes or
                                                  // overlaps another activation of its lexical site. Distinct compound Option,
                                                  // tuple, comparison/index/swap payloads below are paid on EVERY execution.
                                                  // Two explicit backing sites have distinct prepaid primitive frames; no
                                                  // iterator, IterMut::next or Option<&mut Cursor> initializer remains.
                                                  // The first acquisition replaces the identical old compare-entry call and is
                                                  // already caller-received under COMPARE_CALL_CONTROLS; no new payment ABI.
const OWNER_CONTROLS: usize = size_of::<ComparisonOwner<'static>>()
    + lexical_cursor::PRIMITIVE_SELECTION + paired_value::ADDITIONAL_PRIMITIVES
    + lexical_cursor::INITIALIZERS + PAIR_PRIMITIVE
    + size_of::<HistorySet>() // retained original owning input header
    + 16 * PAYMENT_HEADER // original thirteen, two swap sites and final Zip-next site
    + PAYMENT_HEADER // one new sequential flat-view payment site, no dynamic payload credit
    + 2 * (PAYMENT_HEADER + 2 * size_of::<&mut Vec<usize>>()) // explicit two backing sites
    + FINAL_ITERATORS + size_of::<bool>() + 4 * MISMATCH_CALL
    + LEN_QUERIES + SCALAR_TRANSPORT + STATE_TESTS
    + 2 * size_of::<usize>() // initial retained segment extents
    + 3 * size_of::<Step>() // copied state transports at return/schedule sites
    + shared_prefix::OWNER + certified_heads::OWNER + paired_value::TAIL_OWNER
    + final_affine::OWNER; // actual initializers and new lexical primitive sites

const PREPARE_CALL: usize = 2 * size_of::<&mut Cursor>()
    + 2 * size_of::<&History>()
    + 2 * size_of::<&[Segment]>()
    + 2 * size_of::<&ResolvedFunction>()
    + 2 * size_of::<&mut ReplayBudget>()
    + size_of::<Result<(), Diagnostic>>();

pub(in crate::cleanup_plan::replay) fn compare(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    mut actual: HistorySet,
    expected: &[SkeletonPath],
) -> Result<(), Diagnostic> {
    #[cfg(feature = "unstable-workflow-profiling")]
    let _paid_phase =
        budget.paid_phase(crate::workflow_profile::plan_comparison::PaidPhase::Comparison);

    #[cfg(test)]
    if skeleton_work::enumerated_prefix::original_selected() {
        return before_hir_compare::compare(function, budget, actual, expected);
    }
    #[cfg(test)]
    let census = pair_scope_census::begin(function, budget, false);
    #[cfg(feature = "unstable-workflow-profiling")]
    let mut diagnostic = crate::workflow_profile::plan_comparison::begin(
        function.id.as_str(),
        function.span,
        match budget.payer {
            AllocationPayer::LegacyUnobserved => {
                crate::workflow_profile::plan_comparison::Payer::Unobserved
            }
            AllocationPayer::ActiveBuilder => {
                crate::workflow_profile::plan_comparison::Payer::ActiveBuilder
            }
            AllocationPayer::SelectedReservation => {
                crate::workflow_profile::plan_comparison::Payer::SelectedReservation
            }
            #[cfg(test)]
            AllocationPayer::Oracle(_) => crate::workflow_profile::plan_comparison::Payer::Oracle,
        },
        budget.admission == allocation::AdmissionMode::SelectedRetained,
        crate::workflow_profile::plan_comparison::Invoice {
            payment_header: PAYMENT_HEADER,
            owner: OWNER_CONTROLS,
            pair_primitives: PAIR_PRIMITIVE,
            paired_primitives: paired_value::ADDITIONAL_PRIMITIVES,
            path_index: PATH_INDEX,
            pair_scope: PAIR_SCOPE,
            pair_match: PAIR_MATCH,
            new_pair_references: NEW_PAIR_REFERENCES,
            step: paired_value::STEP,
            load: paired_value::LOAD,
            empty: paired_value::EMPTY,
            tail: paired_value::TAIL,
        },
    );
    paid!(
        budget,
        function,
        OWNER_CONTROLS,
        "ordinary plan retained comparison owner"
    )?;
    #[cfg(test)]
    census.owner_admitted();
    #[cfg(feature = "unstable-workflow-profiling")]
    crate::workflow_profile::plan_comparison::event(
        crate::workflow_profile::plan_comparison::Site::OwnerAdmitted,
        OWNER_CONTROLS,
    );
    let mut owner = ComparisonOwner {
        arena: actual.arena.as_slice(),
        left_items: &[],
        right_items: &[],
        left_loaded: false,
        right_loaded: false,
        tail_mask: 0,
        left_cursor: Cursor {
            links: Vec::new(),
            segment: 0,
            item: 0,
        },
        right_cursor: Cursor {
            links: Vec::new(),
            segment: 0,
            item: 0,
        },
        left_segments: 0,
        right_segments: 0,
        step: Step::Build,
        return_to: Step::Build,
        after_pair: Step::Root,
        len: actual.paths.len(),
        build: 0,
        drain: 0,
        root: 0,
        end: 0,
        child: 0,
        left: 0,
        right: 0,
        ordering: Ordering::Equal,
    };
    // Zero paths enter no cursor expansion. One path skips sorting and enters
    // exactly one of the mutually exclusive flat/shared final expansions.
    // Larger sets retain all four independent primitive-site banks. Every
    // selected bank is acquired before any cursor expansion can execute.
    let cursor_primitives = match owner.len {
        0 => 0,
        1 => lexical_cursor::SINGLE_PRIMITIVE_FRAMES,
        _ => lexical_cursor::PRIMITIVE_FRAMES,
    };
    paid!(
        budget,
        function,
        cursor_primitives,
        "ordinary plan reachable cursor primitive frames"
    )?;
    owner.build = owner.len / 2;
    owner.drain = owner.len;
    backing!(
        budget,
        function,
        &mut owner.left_cursor.links,
        actual.max_depth,
        "ordinary plan history cursor backing"
    )?;
    backing!(
        budget,
        function,
        &mut owner.right_cursor.links,
        actual.max_depth,
        "ordinary plan history cursor backing"
    )?;
    // These states replace the old two sift call sites and their two order
    // call sites. Each transition is the corresponding old call/return/loop
    // branch; no graph traversal or work request is added/removed.
    loop {
        match owner.step {
            Step::Build => {
                if owner.build == 0 {
                    owner.step = Step::Drain;
                    continue;
                }
                owner.build -= 1;
                owner.root = owner.build;
                owner.end = owner.len;
                owner.return_to = Step::Build;
                owner.step = Step::Sift;
            }
            Step::Drain => {
                if owner.drain <= 1 {
                    break;
                }
                owner.drain -= 1;
                paid!(
                    budget,
                    function,
                    PATH_SWAP,
                    "ordinary plan retained drain swap receivers"
                )?;
                actual.paths.swap(0, owner.drain);
                #[cfg(test)]
                frontiers::swapped();
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
                paid!(
                    budget,
                    function,
                    PREPARE_CALL + PATH_INDEX + lexical_cursor::RESET + lexical_cursor::LIMIT,
                    "ordinary plan history left cursor prepare"
                )?;
                #[cfg(feature = "unstable-workflow-profiling")]
                crate::workflow_profile::plan_comparison::event(
                    crate::workflow_profile::plan_comparison::Site::LeftPrepareAdmitted,
                    PREPARE_CALL + PATH_INDEX + lexical_cursor::RESET + lexical_cursor::LIMIT,
                );
                owner.left_cursor.prepare(
                    &actual.paths[owner.left].history,
                    &actual.arena,
                    function,
                    budget,
                )?;
                #[cfg(feature = "unstable-workflow-profiling")]
                crate::workflow_profile::plan_comparison::event(
                    crate::workflow_profile::plan_comparison::Site::LeftResetCompleted,
                    0,
                );
                owner.left_segments = owner.left_cursor.links.len();
                owner.left_items = &[];
                owner.left_loaded = false;
                paid!(
                    budget,
                    function,
                    PREPARE_CALL + PATH_INDEX + lexical_cursor::RESET + lexical_cursor::LIMIT,
                    "ordinary plan history right cursor prepare"
                )?;
                #[cfg(feature = "unstable-workflow-profiling")]
                crate::workflow_profile::plan_comparison::event(
                    crate::workflow_profile::plan_comparison::Site::RightPrepareAdmitted,
                    PREPARE_CALL + PATH_INDEX + lexical_cursor::RESET + lexical_cursor::LIMIT,
                );
                owner.right_cursor.prepare(
                    &actual.paths[owner.right].history,
                    &actual.arena,
                    function,
                    budget,
                )?;
                #[cfg(feature = "unstable-workflow-profiling")]
                crate::workflow_profile::plan_comparison::event(
                    crate::workflow_profile::plan_comparison::Site::RightResetCompleted,
                    0,
                );
                owner.right_segments = owner.right_cursor.links.len();
                owner.right_items = &[];
                owner.right_loaded = false;
                #[cfg(test)]
                census.pair();
                #[cfg(feature = "unstable-workflow-profiling")]
                crate::workflow_profile::plan_comparison::event(
                    crate::workflow_profile::plan_comparison::Site::PairPrepared,
                    0,
                );
                owner.step = Step::PairNext;
            }
            Step::PairNext => {
                // Preserve the first original work refusal before acquiring
                // references. The subsequent work site is separately prepaid.
                budget.charge(function, 1, "ordinary plan history observation comparison")?;
                #[cfg(test)]
                census.round();
                #[cfg(feature = "unstable-workflow-profiling")]
                crate::workflow_profile::plan_comparison::event(
                    crate::workflow_profile::plan_comparison::Site::Round,
                    0,
                );
                paid!(
                    budget,
                    function,
                    PAIR_SCOPE
                        + 2 * paired_value::TAIL_VIEW
                        + 2 * paired_value::LINK_VIEW
                        + size_of::<u8>(),
                    "ordinary plan history pair-local paths"
                )?;
                #[cfg(feature = "unstable-workflow-profiling")]
                crate::workflow_profile::plan_comparison::event(
                    crate::workflow_profile::plan_comparison::Site::PairScopeAdmitted,
                    PAIR_SCOPE
                        + 2 * paired_value::TAIL_VIEW
                        + 2 * paired_value::LINK_VIEW
                        + size_of::<u8>(),
                );
                {
                    owner.tail_mask = 0;
                    let left_path = &actual.paths[owner.left];
                    let right_path = &actual.paths[owner.right];
                    let mut left_tail = left_path.history.tail.as_slice();
                    let mut right_tail = right_path.history.tail.as_slice();
                    let mut left_links = owner.left_cursor.links.as_slice();
                    let mut right_links = owner.right_cursor.links.as_slice();
                    'paired: loop {
                        fused_plan_tails!(owner, left_path, right_path, left_tail, right_tail,
                            function, budget, census, 'paired);
                        'certificate: {
                            certified_boundary!(owner, left_links, right_links, function, budget, census, 'certificate);
                            // Both Some paths converge at the bare reference yield;
                            // no Option/tuple or owner payload crosses this frontier.
                            let left = certified_left_value!(
                                owner,
                                left_cursor,
                                left_items,
                                left_loaded,
                                left_segments,
                                fresh_left_link,
                                [left_head => {
                                    certified_right_value!(owner, left_head, fresh_left_link,
                                        right_tail, right_links, function, budget, census, 'paired, 'certificate);
                                }],
                                left_tail,
                                left_links,
                                function,
                                budget,
                                "ordinary plan history paired cursor next",
                                census,
                                {
                                    #[cfg(feature = "unstable-workflow-profiling")]
                                    crate::workflow_profile::plan_comparison::event(
                                        crate::workflow_profile::plan_comparison::Site::LeftExhausted,
                                        0,
                                    );
                                    borrowed_right_value!(owner, right_tail, right_links,
                                        function, budget, "ordinary plan history paired cursor next", census,
                                        [_ => {
                                            owner.ordering = Ordering::Less;
                                            break 'paired;
                                        }], {
                                            #[cfg(feature = "unstable-workflow-profiling")]
                                            crate::workflow_profile::plan_comparison::event(crate::workflow_profile::plan_comparison::Site::RightExhausted, 0);
                                            #[cfg(feature = "unstable-workflow-profiling")]
                                            crate::workflow_profile::plan_comparison::event(crate::workflow_profile::plan_comparison::Site::TerminalCompared, 0);
                                            owner.ordering = left_path.terminal.cmp(&right_path.terminal);
                                            break 'paired;
                                        });
                                }
                            );
                            borrowed_right_value!(owner, right_tail, right_links,
                            function, budget, "ordinary plan history paired cursor next", census,
                            [right => {
                                owner.ordering = left.as_ref().cmp(right.as_ref());
                                if owner.ordering == Ordering::Equal {
                                    budget.charge(function, 1, "ordinary plan history observation comparison")?;
                                    #[cfg(test)] census.round();
                                    #[cfg(feature = "unstable-workflow-profiling")]
                                    crate::workflow_profile::plan_comparison::event(crate::workflow_profile::plan_comparison::Site::Round, 0);
                                    continue 'paired;
                                }
                                break 'paired;
                            }], {
                                #[cfg(feature = "unstable-workflow-profiling")]
                                crate::workflow_profile::plan_comparison::event(crate::workflow_profile::plan_comparison::Site::RightExhausted, 0);
                                owner.ordering = Ordering::Greater;
                                break 'paired;
                            });
                        }
                        certified_suffix!(owner, function, budget, census, 'paired);
                    }
                } // Both PlanPath loans end BEFORE any scheduling or swap.
                owner.step = owner.after_pair;
            }
            Step::Siblings => {
                if owner.ordering == Ordering::Less {
                    owner.child += 1;
                }
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
                paid!(
                    budget,
                    function,
                    PATH_SWAP,
                    "ordinary plan retained sift swap receivers"
                )?;
                actual.paths.swap(owner.root, owner.child);
                #[cfg(test)]
                frontiers::swapped();
                owner.root = owner.child;
                owner.step = Step::Sift;
            }
        }
    }
    let len = owner.len;
    if len != expected.len() {
        return Err(replay_error(
            function,
            "cleanup CFG decision or ownership-event sequence disagrees with typed HIR",
        ));
    }
    let mut pairs = actual.paths.iter().zip(expected);
    loop {
        paid!(
            budget,
            function,
            FINAL_NEXT,
            "ordinary plan retained final Zip next receivers"
        )?;
        let Some((path, flat)) = pairs.next() else {
            break;
        };
        paid!(
            budget,
            function,
            PREPARE_CALL
                + lexical_cursor::RESET
                + lexical_cursor::TAIL_VIEW
                + lexical_cursor::LIMIT,
            "ordinary plan history HIR cursor prepare"
        )?;
        owner
            .left_cursor
            .prepare(&path.history, &actual.arena, function, budget)?;
        let mut plan_tail = path.history.tail.as_slice();
        owner.left_segments = owner.left_cursor.links.len();
        owner.left_items = &[];
        owner.left_loaded = false;
        budget.reserve_bytes(
            function,
            skeleton_work::enumerated_prefix::HIR_QUERY_CONTROLS,
            "ordinary PLAN shared HIR selector receivers",
        )?;
        if flat.observations.prefix.is_none() {
            paid!(
                budget,
                function,
                FLAT_VIEW,
                "ordinary plan history flat HIR retained slice"
            )?;
            let mut hir_items = flat.observations.items.as_slice();
            final_affine_driver!(owner, plan_tail, census, function, budget, flat, hir_items);
        } else {
            budget.reserve_bytes(
                function,
                skeleton_work::enumerated_prefix::CURSOR_STORAGE
                    + skeleton_work::enumerated_prefix::CURSOR_CALL,
                "ordinary PLAN shared HIR cursor preparation receivers",
            )?;
            let mut hir = skeleton_work::enumerated_prefix::Cursor::new();
            hir.prepare(&flat.observations, function, budget)?;
            final_affine_driver!(owner, plan_tail, census, function, budget, shared, hir);
        }
        if path.terminal != flat.terminal {
            return Err(replay_error(
                function,
                "cleanup CFG decision or ownership-event sequence disagrees with typed HIR",
            ));
        }
    }
    #[cfg(test)]
    census.success();
    #[cfg(feature = "unstable-workflow-profiling")]
    diagnostic.success();
    Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
fn reachable_stage(path: &str, sha: &str, current: &str) -> String {
    crate::source_reflows_merge::main_e177_final_inputs::value_direct(path, sha, current)
}
