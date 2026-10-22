//! Bare pair-local reference yields; exhaustion consumes its lexical branch.
use super::*;
// One additional right expansion; all five real primitive frontiers are paid
// even for zero pairs. No frozen old invoice is credited or changed.
pub(super) const ADDITIONAL_PRIMITIVES: usize = 5 * PAYMENT_HEADER;
pub(super) const STEP: usize = 2 * size_of::<&Vec<usize>>() + size_of::<usize>() + size_of::<bool>() // unchanged len/condition
    + 2 * size_of::<bool>() // loaded condition and negation
    + size_of::<&Rc<SkeletonObservation>>() // head pattern
    + 2 * size_of::<&[Rc<SkeletonObservation>]>() // rest pattern and assignment copy
    + 2 * size_of::<&Rc<SkeletonObservation>>() + size_of::<&SkeletonObservation>() // as_ref actual/formal/return
    + size_of::<usize>(); // original item increment
                          // Only entering a new immutable segment forms these views/index receivers.
pub(super) const LOAD: usize = 2 * size_of::<&Vec<usize>>() + 2 * size_of::<&[usize]>() // as_slice actual/formal/return/local
    + 2 * size_of::<&[usize]>() + size_of::<usize>() // len actual/formal/return
    + 2 * size_of::<usize>() // subtraction intermediates
    + 2 * size_of::<&[usize]>() + 2 * size_of::<usize>() // slice index actual/formal
    + size_of::<&usize>() + size_of::<usize>() // returned reference and copied link
    + 2 * size_of::<&[Segment]>() + 2 * size_of::<usize>() + size_of::<&Segment>()
    + 2 * size_of::<&Vec<Rc<SkeletonObservation>>>() // items as_slice actual/formal
    + 2 * size_of::<&[Rc<SkeletonObservation>]>() // as_slice return and stored field copy
    + size_of::<bool>(); // loaded=true
                         // The original exhausted-segment update and fresh tail remain honestly paid.
pub(super) const EMPTY: usize = 2 * size_of::<usize>() + size_of::<bool>(); // segment increment, item=0, loaded=false
pub(super) const TAIL: usize = size_of::<&History>()
    + 2 * size_of::<&Vec<Rc<SkeletonObservation>>>()
    + 3 * size_of::<&[Rc<SkeletonObservation>]>()
    + 2 * size_of::<usize>()
    + 3 * size_of::<Option<&Rc<SkeletonObservation>>>()
    + 2 * size_of::<&Option<&Rc<SkeletonObservation>>>()
    + size_of::<bool>()
    + size_of::<usize>()
    + size_of::<&Rc<SkeletonObservation>>()
    + 2 * size_of::<&Rc<SkeletonObservation>>()
    + size_of::<&SkeletonObservation>();

macro_rules! lexical_plan_value {
    ($owner:ident, $cursor:ident, $items:ident, $loaded:ident,
     $history:expr, $function:ident, $budget:ident, $phase:literal, $census:ident, $empty:block) => {{
        $budget.charge($function, 1, "ordinary plan history forward cursor step")?;
        'plan_yield: {
            loop {
                paid!(
                    $budget,
                    $function,
                    paired_value::STEP + paired_value::EMPTY,
                    $phase
                )?;
                #[cfg(test)]
                $census.value_step();
                #[cfg(feature = "unstable-workflow-profiling")]
                $crate::workflow_profile::plan_comparison::event(
                    $crate::workflow_profile::plan_comparison::Site::StepAdmitted,
                    paired_value::STEP + paired_value::EMPTY,
                );
                if $owner.$cursor.segment >= $owner.$cursor.links.len() {
                    break;
                }
                $budget.charge(
                    $function,
                    1,
                    "ordinary plan history forward segment traversal",
                )?;
                if !$owner.$loaded {
                    paid!($budget, $function, paired_value::LOAD, $phase)?;
                    #[cfg(feature = "unstable-workflow-profiling")]
                    $crate::workflow_profile::plan_comparison::event(
                        $crate::workflow_profile::plan_comparison::Site::LoadAdmitted,
                        paired_value::LOAD,
                    );
                    let links = $owner.$cursor.links.as_slice();
                    let link = links[links.len() - $owner.$cursor.segment - 1];
                    $owner.$items = $owner.arena[link].items.as_slice();
                    $owner.$loaded = true;
                }
                match $owner.$items {
                    [head, rest @ ..] => {
                        #[cfg(feature = "unstable-workflow-profiling")]
                        $crate::workflow_profile::plan_comparison::event(
                            if stringify!($cursor) == "left_cursor" {
                                $crate::workflow_profile::plan_comparison::Site::LeftHead
                            } else {
                                $crate::workflow_profile::plan_comparison::Site::RightHead
                            },
                            0,
                        );
                        $owner.$items = rest;
                        $owner.$cursor.item += 1;
                        break 'plan_yield head.as_ref();
                    }
                    [] => {
                        #[cfg(feature = "unstable-workflow-profiling")]
                        $crate::workflow_profile::plan_comparison::event(
                            $crate::workflow_profile::plan_comparison::Site::EmptySegment,
                            0,
                        );
                        $owner.$cursor.segment += 1;
                        $owner.$cursor.item = 0;
                        $owner.$loaded = false;
                    }
                }
            }
            paid!($budget, $function, paired_value::TAIL, $phase)?;
            #[cfg(test)]
            $census.value_tail();
            #[cfg(feature = "unstable-workflow-profiling")]
            $crate::workflow_profile::plan_comparison::event(
                $crate::workflow_profile::plan_comparison::Site::TailAdmitted,
                paired_value::TAIL,
            );
            let history = &$history;
            let value = history.tail.get($owner.$cursor.item);
            if value.is_some() {
                #[cfg(feature = "unstable-workflow-profiling")]
                $crate::workflow_profile::plan_comparison::event(
                    if stringify!($cursor) == "left_cursor" {
                        $crate::workflow_profile::plan_comparison::Site::LeftTailSome
                    } else {
                        $crate::workflow_profile::plan_comparison::Site::RightTailSome
                    },
                    0,
                );
                $owner.$cursor.item += 1;
            }
            match value {
                Some(value) => value.as_ref(),
                None => $empty,
            }
        }
    }};
}
