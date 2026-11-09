//! Bare pair-local reference yields; exhaustion consumes its lexical branch.
use super::*;
// Original additional right expansion retains all five primitive frames;
// duplicating the ordinary right-success continuation adds one work frame.
// All six are paid even for zero pairs; no frozen old bank is credited.
pub(super) const ADDITIONAL_PRIMITIVES: usize = 6 * PAYMENT_HEADER;
pub(super) const STEP: usize = 2 * size_of::<usize>() + size_of::<bool>() // retained segment extent and condition
    + 2 * size_of::<bool>() // loaded condition and negation
    + size_of::<&Rc<SkeletonObservation>>() // head pattern
    + 2 * size_of::<&[Rc<SkeletonObservation>]>() // rest pattern and assignment copy
    + 2 * size_of::<&Rc<SkeletonObservation>>() + size_of::<&SkeletonObservation>() // as_ref actual/formal/return
    + size_of::<usize>(); // original item increment
// Only entering a new immutable segment forms these views/index receivers.
pub(super) const LOAD: usize = 2 * size_of::<&mut Vec<usize>>()
    + size_of::<Option<usize>>()
    + size_of::<usize>()
    + size_of::<bool>()
    + 2 * size_of::<&[Segment]>()
    + 2 * size_of::<usize>()
    + size_of::<&Segment>()
    + 2 * size_of::<&Vec<Rc<SkeletonObservation>>>()
    + 2 * size_of::<&[Rc<SkeletonObservation>]>()
    + size_of::<bool>(); // loaded=true
// Pair-local borrowed suffix: ref-rest pattern plus assignment, copied link,
// failed-pattern branch and unchanged arena/index/view/loaded receivers.
pub(super) const PAIR_LOAD: usize = 2 * size_of::<&[usize]>()
    + size_of::<usize>()
    + size_of::<bool>()
    + 2 * size_of::<&[Segment]>()
    + 2 * size_of::<usize>()
    + size_of::<&Segment>()
    + 2 * size_of::<&Vec<Rc<SkeletonObservation>>>()
    + 2 * size_of::<&[Rc<SkeletonObservation>]>()
    + size_of::<bool>();
const _: () = assert!(PAIR_LOAD == LOAD);
// The original exhausted-segment update and fresh tail remain honestly paid.
pub(super) const EMPTY: usize = 2 * size_of::<usize>() + size_of::<bool>(); // segment increment, item=0, loaded=false
pub(super) const LINK_VIEW: usize = 2 * size_of::<&Vec<usize>>() + 2 * size_of::<&[usize]>();
pub(super) const TAIL_VIEW: usize =
    2 * size_of::<&Vec<Rc<SkeletonObservation>>>() + 2 * size_of::<&[Rc<SkeletonObservation>]>();
pub(super) const TAIL: usize = size_of::<&Rc<SkeletonObservation>>()
    + 2 * size_of::<&[Rc<SkeletonObservation>]>()
    + 2 * size_of::<&Rc<SkeletonObservation>>()
    + size_of::<&SkeletonObservation>()
    + size_of::<usize>();

// Exact replacement within the original left head-yield frontier only:
// the retired as_ref actual/formal plus returned Observation reference bank
// funds the bare loop-break Rc reference and its distinct outer let receiver.
// It is not reused by PAIR_CONTENT's actual as_ref calls at comparison. The
// remaining old word is conservatively retained; no left fee is discounted.
#[cfg(test)]
pub(super) const LEFT_RETIRED_AS_REF: usize =
    2 * size_of::<&Rc<SkeletonObservation>>() + size_of::<&SkeletonObservation>();
#[cfg(test)]
pub(super) const LEFT_YIELD_AND_RECEIVE: usize = 2 * size_of::<&Rc<SkeletonObservation>>();
const _: () = assert!(
    2 * size_of::<&Rc<SkeletonObservation>>()
        <= 2 * size_of::<&Rc<SkeletonObservation>>() + size_of::<&SkeletonObservation>()
);

// The left producer retains its entire old envelope. A right branch now keeps
// only its Rc pattern until the actual content comparison; exhaustion produces
// no observation reference, Option, return value, or ignored right local.
pub(super) const RIGHT_STEP: usize = 2 * size_of::<usize>()
    + 3 * size_of::<bool>()
    + size_of::<&Rc<SkeletonObservation>>()
    + 2 * size_of::<&[Rc<SkeletonObservation>]>()
    + size_of::<usize>()
    + EMPTY;
pub(super) const RIGHT_TAIL: usize = size_of::<&Rc<SkeletonObservation>>()
    + 2 * size_of::<&[Rc<SkeletonObservation>]>()
    + size_of::<usize>();
// Complete genuine main487 content transport: both Rc reborrows and received
// observation views, then the unchanged content/terminal comparison envelopes.
pub(super) const PAIR_CONTENT: usize = 4 * size_of::<&Rc<SkeletonObservation>>()
    + 2 * size_of::<&SkeletonObservation>()
    + 4 * size_of::<&SkeletonObservation>()
    + 4 * size_of::<&SkeletonTerminal>()
    + 2 * size_of::<Ordering>();
macro_rules! borrowed_right_value {
    ($owner:ident, $tail:ident, $links:ident, $function:ident, $budget:ident,
     $phase:literal, $census:ident, [$head:pat => $some:block], $empty:block) => {{
        $budget.charge($function, 1, "ordinary plan history forward cursor step")?;
        loop {
            paid!($budget, $function, paired_value::RIGHT_STEP, $phase)?;
            #[cfg(test)]
            {
                $census.value_step();
                $census.step();
                $census.pair_right_steps();
            }
            #[cfg(feature = "unstable-workflow-profiling")]
            $crate::workflow_profile::plan_comparison::event(
                $crate::workflow_profile::plan_comparison::Site::StepAdmitted,
                paired_value::RIGHT_STEP,
            );
            #[cfg(feature = "unstable-workflow-profiling")]
            $crate::workflow_profile::plan_comparison::event(
                $crate::workflow_profile::plan_comparison::Site::BorrowedRightStep,
                0,
            );
            if $owner.right_cursor.segment >= $owner.right_segments {
                break;
            }
            $budget.charge(
                $function,
                1,
                "ordinary plan history forward segment traversal",
            )?;
            if !$owner.right_loaded {
                paid!($budget, $function, paired_value::PAIR_LOAD, $phase)?;
                #[cfg(test)]
                $census.load();
                #[cfg(feature = "unstable-workflow-profiling")]
                $crate::workflow_profile::plan_comparison::event(
                    $crate::workflow_profile::plan_comparison::Site::LoadAdmitted,
                    paired_value::PAIR_LOAD,
                );
                let &[ref links_rest @ .., link] = $links else {
                    unreachable!("prepared ordinary plan links");
                };
                $links = links_rest;
                $owner.right_items = $owner.arena[link].items.as_slice();
                $owner.right_loaded = true;
            }
            match $owner.right_items {
                [$head, rest @ ..] => {
                    $owner.right_items = rest;
                    $owner.right_cursor.item += 1;
                    #[cfg(feature = "unstable-workflow-profiling")]
                    $crate::workflow_profile::plan_comparison::event(
                        $crate::workflow_profile::plan_comparison::Site::RightHead,
                        0,
                    );
                    $some
                }
                [] => {
                    #[cfg(feature = "unstable-workflow-profiling")]
                    $crate::workflow_profile::plan_comparison::event(
                        $crate::workflow_profile::plan_comparison::Site::EmptySegment,
                        0,
                    );
                    $owner.right_cursor.segment += 1;
                    $owner.right_cursor.item = 0;
                    $owner.right_loaded = false;
                }
            }
        }
        paid!(
            $budget,
            $function,
            paired_value::RIGHT_TAIL + paired_value::TAIL_MARK,
            $phase
        )?;
        mark_plan_tail!($owner, right_cursor);
        #[cfg(test)]
        {
            $census.tail_marks();
            $census.value_tail();
            $census.pair_right_tails();
        }
        #[cfg(feature = "unstable-workflow-profiling")]
        {
            $crate::workflow_profile::plan_comparison::event(
                $crate::workflow_profile::plan_comparison::Site::TailMarkAdmitted,
                paired_value::TAIL_MARK,
            );
            $crate::workflow_profile::plan_comparison::event(
                $crate::workflow_profile::plan_comparison::Site::TailAdmitted,
                paired_value::RIGHT_TAIL,
            );
            $crate::workflow_profile::plan_comparison::event(
                $crate::workflow_profile::plan_comparison::Site::BorrowedRightTail,
                0,
            );
        }
        match $tail {
            [$head, rest @ ..] => {
                $tail = rest;
                $owner.right_cursor.item += 1;
                #[cfg(feature = "unstable-workflow-profiling")]
                $crate::workflow_profile::plan_comparison::event(
                    $crate::workflow_profile::plan_comparison::Site::RightTailSome,
                    0,
                );
                $some
            }
            [] => $empty,
        }
    }};
}

// Five actual new primitive sites: selection reserve, bulk reserve and three
// original work calls. No old bank is reduced. The field itself is in sizeof
// ComparisonOwner; this bank additionally owns its literal initializer.
pub(super) const TAIL_OWNER: usize = 5 * PAYMENT_HEADER + size_of::<u8>();
pub(super) const TAIL_SELECT: usize = 2 * size_of::<u8>() + size_of::<bool>();
pub(super) const TAIL_MARK: usize = 3 * size_of::<u8>();
pub(super) const TAIL_TURN: usize = 2 * size_of::<&Rc<SkeletonObservation>>()
    + 4 * size_of::<&[Rc<SkeletonObservation>]>()
    + 4 * size_of::<&Rc<SkeletonObservation>>()
    + 2 * size_of::<&SkeletonObservation>()
    + 4 * size_of::<&SkeletonObservation>()
    + 4 * size_of::<&SkeletonTerminal>()
    + 2 * size_of::<usize>()
    + 4 * size_of::<Ordering>()
    + 3 * size_of::<bool>();
macro_rules! mark_plan_tail {
    ($owner:ident, left_cursor) => {
        $owner.tail_mask |= 1;
    };
    ($owner:ident, right_cursor) => {
        $owner.tail_mask |= 2;
    };
}
macro_rules! fused_plan_tails {
    ($owner:ident, $left_path:ident, $right_path:ident, $left_tail:ident, $right_tail:ident,
     $function:ident, $budget:ident, $census:ident, $paired:lifetime) => {{
        paid!($budget, $function, paired_value::TAIL_SELECT, "ordinary plan paired tail selection")?;
        #[cfg(test)] $census.mask_checks();
        #[cfg(feature = "unstable-workflow-profiling")]
        $crate::workflow_profile::plan_comparison::event(
            $crate::workflow_profile::plan_comparison::Site::TailMaskAdmitted, paired_value::TAIL_SELECT);
        if $owner.tail_mask == 3 {
            loop {
                paid!($budget, $function, paired_value::TAIL_TURN, "ordinary plan paired tail turn")?;
                #[cfg(test)] $census.fused_turns();
                #[cfg(feature = "unstable-workflow-profiling")]
                $crate::workflow_profile::plan_comparison::event(
                    $crate::workflow_profile::plan_comparison::Site::TailTurnAdmitted, paired_value::TAIL_TURN);
                $budget.charge($function, 1, "ordinary plan history forward cursor step")?;
                #[cfg(test)] $census.tail_work();
                #[cfg(feature = "unstable-workflow-profiling")]
                $crate::workflow_profile::plan_comparison::event(
                    $crate::workflow_profile::plan_comparison::Site::TailWork, 0);
                #[cfg(test)] pair_scope_census::maybe_unwind_tail_work();
                $budget.charge($function, 1, "ordinary plan history forward cursor step")?;
                #[cfg(test)] $census.tail_work();
                #[cfg(feature = "unstable-workflow-profiling")]
                $crate::workflow_profile::plan_comparison::event(
                    $crate::workflow_profile::plan_comparison::Site::TailWork, 0);
                #[cfg(test)] pair_scope_census::maybe_unwind_tail_work();
                match $left_tail {
                    [left, left_rest @ ..] => {
                        $left_tail = left_rest;
                        $owner.left_cursor.item += 1;
                        #[cfg(feature = "unstable-workflow-profiling")]
                        $crate::workflow_profile::plan_comparison::event(
                            $crate::workflow_profile::plan_comparison::Site::LeftTailSome, 0);
                        match $right_tail {
                            [right, right_rest @ ..] => {
                                $right_tail = right_rest;
                                $owner.right_cursor.item += 1;
                                #[cfg(feature = "unstable-workflow-profiling")]
                                $crate::workflow_profile::plan_comparison::event(
                                    $crate::workflow_profile::plan_comparison::Site::RightTailSome, 0);
                                $owner.ordering = left.as_ref().cmp(right.as_ref());
                                if $owner.ordering == Ordering::Equal {
                                    $budget.charge($function, 1, "ordinary plan history observation comparison")?;
                                    #[cfg(test)] { $census.tail_work(); $census.round(); }
                                    #[cfg(feature = "unstable-workflow-profiling")]
                                    { $crate::workflow_profile::plan_comparison::event(
                                        $crate::workflow_profile::plan_comparison::Site::TailWork, 0);
                                      $crate::workflow_profile::plan_comparison::event(
                                        $crate::workflow_profile::plan_comparison::Site::Round, 0); }
                                    #[cfg(test)] pair_scope_census::maybe_unwind_tail_work();
                                    continue;
                                }
                            }
                            [] => {
                                $owner.ordering = Ordering::Greater;
                                #[cfg(feature = "unstable-workflow-profiling")]
                                $crate::workflow_profile::plan_comparison::event(
                                    $crate::workflow_profile::plan_comparison::Site::RightExhausted, 0);
                            }
                        }
                    }
                    [] => {
                        #[cfg(feature = "unstable-workflow-profiling")]
                        $crate::workflow_profile::plan_comparison::event(
                            $crate::workflow_profile::plan_comparison::Site::LeftExhausted, 0);
                        match $right_tail {
                            [_, right_rest @ ..] => {
                                $right_tail = right_rest;
                                $owner.right_cursor.item += 1;
                                $owner.ordering = Ordering::Less;
                                #[cfg(feature = "unstable-workflow-profiling")]
                                $crate::workflow_profile::plan_comparison::event(
                                    $crate::workflow_profile::plan_comparison::Site::RightTailSome, 0);
                            }
                            [] => {
                                $owner.ordering = $left_path.terminal.cmp(&$right_path.terminal);
                                #[cfg(feature = "unstable-workflow-profiling")]
                                { $crate::workflow_profile::plan_comparison::event(
                                    $crate::workflow_profile::plan_comparison::Site::RightExhausted, 0);
                                  $crate::workflow_profile::plan_comparison::event(
                                    $crate::workflow_profile::plan_comparison::Site::TerminalCompared, 0); }
                            }
                        }
                    }
                }
                break $paired;
            }
        }
    }};
}
