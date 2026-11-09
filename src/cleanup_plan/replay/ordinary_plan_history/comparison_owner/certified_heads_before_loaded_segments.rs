//! Two lexical first heads certify an immutable segment, never a saved verdict.
use super::{Ordering, PAYMENT_HEADER, Rc, Segment, SkeletonObservation, size_of};
// Additional actual reserve/work sites relative to the complete d458 owner:
// split right producer +4, relocated MATCH +3, first-head certificate +1.
pub(super) const OWNER: usize = 23 * PAYMENT_HEADER;
// One place-pattern boolean. No copied slice, receiver call or saved verdict.
pub(super) const EMPTY_SELECT: usize = size_of::<bool>();
// Additional actual sites beyond the first-head split: thirteen boundary
// sites (six payments and seven work calls), one numeric-driver payment,
// and one empty-selector payment at the sole common suffix expansion.
pub(super) const BOUNDARY_CHECK: usize = 3 * size_of::<bool>();
pub(super) const BOUNDARY_EMPTY: usize = size_of::<bool>();
pub(super) const BOUNDARY_LINKS: usize =
    2 * size_of::<&[usize]>() + 4 * size_of::<usize>() + 3 * size_of::<bool>();
pub(super) const BOUNDARY_VIEW: usize = 2 * size_of::<&[Segment]>()
    + 2 * size_of::<usize>()
    + size_of::<&Segment>()
    + 2 * size_of::<&Vec<Rc<SkeletonObservation>>>()
    + 3 * size_of::<&[Rc<SkeletonObservation>]>()
    + size_of::<bool>();
// Keep both conservative remembered-link slots and every other incoming role.
// VIEW already paid next_items/rest; BODY now forms only the left rest assignment.
// The old right rest was unused until RUN_PUBLISH overwrote both slices with [].
pub(super) const BOUNDARY_BODY: usize = 2 * size_of::<&[usize]>()
    + size_of::<[usize; 8]>()
    + 4 * size_of::<bool>()
    + size_of::<&[Rc<SkeletonObservation>]>()
    + size_of::<Ordering>();
pub(super) type SharedUnitDriver = std::iter::Take<std::iter::Repeat<()>>;
// Exact closed unit-only run transport, including constructor/IntoIterator and
// one retained next frame with its sole terminal None. No observation payload.
pub(super) const RUN_CAPTURE: usize = 2 * size_of::<&[Rc<SkeletonObservation>]>()
    + 4 * size_of::<usize>()
    + 6 * size_of::<SharedUnitDriver>()
    + 2 * size_of::<&mut SharedUnitDriver>()
    + 2 * size_of::<Option<()>>();
#[cfg(test)]
pub(super) const RUN_TURN: usize = 0;
const _: () = assert!(size_of::<()>() == 0 && !std::mem::needs_drop::<SharedUnitDriver>());
pub(super) const RUN_PUBLISH: usize =
    4 * size_of::<&[Rc<SkeletonObservation>]>() + 4 * size_of::<usize>() + size_of::<Ordering>();
pub(super) const BASE: usize =
    2 * size_of::<usize>() + 3 * size_of::<bool>() + super::paired_value::EMPTY;
pub(super) const POP: usize =
    2 * size_of::<&[usize]>() + size_of::<usize>() + 2 * size_of::<bool>();
pub(super) const VIEW_HEAD: usize = 2 * size_of::<&[Segment]>()
    + 2 * size_of::<usize>()
    + size_of::<&Segment>()
    + 2 * size_of::<&Vec<Rc<SkeletonObservation>>>()
    + 2 * size_of::<&[Rc<SkeletonObservation>]>()
    + HEAD;
pub(super) const HEAD: usize = size_of::<&Rc<SkeletonObservation>>()
    + 2 * size_of::<&[Rc<SkeletonObservation>]>()
    + 2 * size_of::<&Rc<SkeletonObservation>>()
    + size_of::<&SkeletonObservation>()
    + size_of::<usize>();
// The old as_ref invoice remains conservative admission on the identity branch;
// it is not a claim that an observation receiver is physically produced there.
pub(super) const RHS: usize = size_of::<&[Rc<SkeletonObservation>]>()
    + size_of::<usize>()
    + 2 * size_of::<&Rc<SkeletonObservation>>()
    + size_of::<&SkeletonObservation>();
pub(super) const FIRST: usize = 2 * size_of::<&SkeletonObservation>() + size_of::<Ordering>();

macro_rules! certified_match {
    ($budget:ident, $function:ident, $census:ident) => {{
        paid!(
            $budget,
            $function,
            PAIR_MATCH,
            "ordinary plan history paired cursor next"
        )?;
        #[cfg(test)]
        {
            $census.pair_match_admitted();
            $census.pair_receivers();
        }
        #[cfg(feature = "unstable-workflow-profiling")]
        $crate::workflow_profile::plan_comparison::event(
            $crate::workflow_profile::plan_comparison::Site::MatchAdmitted,
            PAIR_MATCH,
        );
    }};
}
macro_rules! certified_right_value {
    ($owner:ident, $left_head:ident, $left_link:ident, $tail:ident, $links:ident,
     $function:ident, $budget:ident, $census:ident, $paired:lifetime, $certificate:lifetime) => {{
        // Original observation receiving slots, formed only on fallback. There
        // is no new Option, tuple, callback, result or Rc alias transport.
        let left;
        let right;
        'right_observation: {
            $budget.charge($function, 1, "ordinary plan history forward cursor step")?;
            loop {
                paid!($budget, $function, certified_heads::BASE,
                    "ordinary plan fresh right segment base")?;
                #[cfg(test)] $census.certified_base();
                    #[cfg(feature = "unstable-workflow-profiling")]
                    $crate::workflow_profile::plan_comparison::event(
                        $crate::workflow_profile::plan_comparison::Site::CertifiedBaseAdmitted,
                        certified_heads::BASE);
                if $owner.right_cursor.segment >= $owner.right_segments {
                    // Preserve the complete old unused head/as_ref envelope on
                    // this noncertified sentinel, before the original tail fee.
                    paid!($budget, $function, certified_heads::HEAD,
                        "ordinary plan exhausted right step retained envelope")?;
                    #[cfg(test)] $census.certified_head();
                    #[cfg(feature = "unstable-workflow-profiling")]
                    $crate::workflow_profile::plan_comparison::event(
                        $crate::workflow_profile::plan_comparison::Site::CertifiedHeadAdmitted,
                        certified_heads::HEAD);
                    break;
                }
                $budget.charge($function, 1, "ordinary plan history forward segment traversal")?;
                if !$owner.right_loaded {
                    paid!($budget, $function, certified_heads::POP,
                        "ordinary plan fresh right link pop")?;
                    #[cfg(test)] $census.certified_pop();
                    #[cfg(feature = "unstable-workflow-profiling")]
                    $crate::workflow_profile::plan_comparison::event(
                        $crate::workflow_profile::plan_comparison::Site::CertifiedPopAdmitted,
                        certified_heads::POP);
                    let &[ref right_links_rest @ .., right_link] = $links else {
                        unreachable!("prepared ordinary plan links");
                    };
                    $links = right_links_rest;
                    paid!($budget, $function, shared_prefix::DETECTOR,
                        "ordinary plan fresh segment identity event")?;
                    #[cfg(test)] $census.identity_detector();
                    #[cfg(feature = "unstable-workflow-profiling")]
                    $crate::workflow_profile::plan_comparison::event(
                        $crate::workflow_profile::plan_comparison::Site::IdentityDetectorAdmitted,
                        shared_prefix::DETECTOR);
                    if $left_link == right_link {
                        paid!($budget, $function, certified_heads::RHS,
                            "ordinary plan certified right slice assignment")?;
                        #[cfg(test)] $census.certified_rhs();
                    #[cfg(feature = "unstable-workflow-profiling")]
                    $crate::workflow_profile::plan_comparison::event(
                        $crate::workflow_profile::plan_comparison::Site::CertifiedRhsAdmitted,
                        certified_heads::RHS);
                        $owner.right_items = $owner.left_items;
                        $owner.right_loaded = true;
                        $owner.right_cursor.item += 1;
                        paid!($budget, $function, certified_heads::FIRST,
                            "ordinary plan certified first observation")?;
                        #[cfg(test)] $census.certified_first();
                    #[cfg(feature = "unstable-workflow-profiling")]
                    $crate::workflow_profile::plan_comparison::event(
                        $crate::workflow_profile::plan_comparison::Site::CertifiedFirstAdmitted,
                        certified_heads::FIRST);
                        $owner.ordering = Ordering::Equal;
                        // This is the original comparison work, after both
                        // cursor advances and before any suffix probe.
                        $budget.charge($function, 1, "ordinary plan history observation comparison")?;
                        #[cfg(test)] $census.round();
                        #[cfg(test)] $census.identity_hit();
                        #[cfg(feature = "unstable-workflow-profiling")]
                        {
                            $crate::workflow_profile::plan_comparison::event(
                                $crate::workflow_profile::plan_comparison::Site::Round, 0);
                            $crate::workflow_profile::plan_comparison::event(
                                $crate::workflow_profile::plan_comparison::Site::IdentityHit, 0);
                        }
                        break $certificate;
                    }
                    paid!($budget, $function, certified_heads::VIEW_HEAD,
                        "ordinary plan foreign right segment view and head")?;
                    #[cfg(test)] $census.certified_view();
                    #[cfg(feature = "unstable-workflow-profiling")]
                    $crate::workflow_profile::plan_comparison::event(
                        $crate::workflow_profile::plan_comparison::Site::CertifiedViewAdmitted,
                        certified_heads::VIEW_HEAD);
                    $owner.right_items = $owner.arena[right_link].items.as_slice();
                    $owner.right_loaded = true;
                } else {
                    paid!($budget, $function, certified_heads::HEAD,
                        "ordinary plan loaded right head")?;
                    #[cfg(test)] $census.certified_head();
                    #[cfg(feature = "unstable-workflow-profiling")]
                    $crate::workflow_profile::plan_comparison::event(
                        $crate::workflow_profile::plan_comparison::Site::CertifiedHeadAdmitted,
                        certified_heads::HEAD);
                }
                match $owner.right_items {
                    [head, rest @ ..] => {
                        $owner.right_items = rest;
                        $owner.right_cursor.item += 1;
                        certified_match!($budget, $function, $census);
                        left = $left_head.as_ref();
                        right = head.as_ref();
                        break 'right_observation;
                    }
                    [] => {
                        $owner.right_cursor.segment += 1;
                        $owner.right_cursor.item = 0;
                        $owner.right_loaded = false;
                    }
                }
            }
            paid!($budget, $function, paired_value::TAIL + paired_value::TAIL_MARK,
                "ordinary plan history paired cursor next")?;
            mark_plan_tail!($owner, right_cursor);
            #[cfg(test)] $census.tail_marks();
            #[cfg(feature = "unstable-workflow-profiling")]
            $crate::workflow_profile::plan_comparison::event(
                $crate::workflow_profile::plan_comparison::Site::TailMarkAdmitted, paired_value::TAIL_MARK);
            #[cfg(test)] $census.value_tail();
            #[cfg(feature = "unstable-workflow-profiling")]
            $crate::workflow_profile::plan_comparison::event(
                $crate::workflow_profile::plan_comparison::Site::TailAdmitted,
                paired_value::TAIL);
            certified_match!($budget, $function, $census);
            match $tail {
                [head, rest @ ..] => {
                    $tail = rest;
                    $owner.right_cursor.item += 1;
                    left = $left_head.as_ref();
                    right = head.as_ref();
                }
                [] => { $owner.ordering = Ordering::Greater; break $paired; }
            }
        }
        shared_prefix_first_comparison!($owner, left, right, $function, $budget,
            $census, $paired, { continue $paired; });
    }};
}

// BEGIN LEFT FIRST-HEAD PRODUCER
// Left first-head continuation retains the original Rc pattern loan.
macro_rules! certified_left_value {
    ($owner:ident, $cursor:ident, $items:ident, $loaded:ident, $limit:ident, $link:ident, [$($head_value:ident => $entry:block)?],
     $tail:ident, $links:ident, $function:ident, $budget:ident, $phase:literal, $census:ident, $empty:block) => {{
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
                #[cfg(test)]
                $census.step();
                #[cfg(feature = "unstable-workflow-profiling")]
                $crate::workflow_profile::plan_comparison::event(
                    $crate::workflow_profile::plan_comparison::Site::StepAdmitted,
                    paired_value::STEP + paired_value::EMPTY,
                );
                if $owner.$cursor.segment >= $owner.$limit {
                    break;
                }
                $budget.charge(
                    $function,
                    1,
                    "ordinary plan history forward segment traversal",
                )?;
                if !$owner.$loaded {
                    paid!(
                        $budget,
                        $function,
                        paired_value::PAIR_LOAD,
                        $phase
                    )?;
                    #[cfg(feature = "unstable-workflow-profiling")]
                    $crate::workflow_profile::plan_comparison::event(
                        $crate::workflow_profile::plan_comparison::Site::LoadAdmitted,
                        paired_value::PAIR_LOAD,
                    );
                    #[cfg(test)]
                    $census.load();
                    let &[ref links_rest @ .., $link] = $links else {
                        unreachable!("prepared ordinary plan links");
                    };
                    $links = links_rest;
                    $owner.$items = $owner.arena[$link].items.as_slice();
                    $owner.$loaded = true;
                    $(
                        // First-head provenance and the already-paid pop's link
                        // remain lexical. The continuation always leaves this
                        // successful head; [] still takes the original path.
                        match $owner.$items {
                            [$head_value, rest @ ..] => {
                                #[cfg(feature = "unstable-workflow-profiling")]
                                $crate::workflow_profile::plan_comparison::event(
                                    if stringify!($cursor) == "left_cursor" {
                                        $crate::workflow_profile::plan_comparison::Site::LeftHead
                                    } else {
                                        $crate::workflow_profile::plan_comparison::Site::RightHead
                                    }, 0);
                                $owner.$items = rest;
                                $owner.$cursor.item += 1;
                                $entry
                            }
                            [] => {}
                        }
                    )?
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
                        certified_match!($budget, $function, $census);
                        break 'plan_yield head;
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
            paid!($budget, $function, paired_value::TAIL + paired_value::TAIL_MARK, $phase)?;
            mark_plan_tail!($owner, $cursor);
            #[cfg(test)] $census.tail_marks();
            #[cfg(feature = "unstable-workflow-profiling")]
            $crate::workflow_profile::plan_comparison::event(
                $crate::workflow_profile::plan_comparison::Site::TailMarkAdmitted, paired_value::TAIL_MARK);
            #[cfg(test)]
            $census.value_tail();
            #[cfg(feature = "unstable-workflow-profiling")]
            $crate::workflow_profile::plan_comparison::event(
                $crate::workflow_profile::plan_comparison::Site::TailAdmitted,
                paired_value::TAIL,
            );
            certified_match!($budget, $function, $census);
            match $tail {
                [head, rest @ ..] => {
                    $tail = rest;
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
                    head
                }
                [] => $empty,
            }
        }
    }};
}

// BEGIN CERTIFIED SUFFIX
// A certificate reaches this single expansion from either the two fresh heads
// or the direct loaded-empty boundary. No link, flag or certificate is stored.
macro_rules! certified_suffix {
    ($owner:ident, $function:ident, $budget:ident, $census:ident, $paired:lifetime) => {{
        paid!($budget, $function, certified_heads::EMPTY_SELECT,
            "ordinary plan certified suffix empty selector")?;
        #[cfg(test)] $census.empty_selector();
        #[cfg(feature = "unstable-workflow-profiling")]
        $crate::workflow_profile::plan_comparison::event(
            $crate::workflow_profile::plan_comparison::Site::EmptySelectorAdmitted,
            certified_heads::EMPTY_SELECT);
        if let [] = $owner.left_items {
            #[cfg(test)] $census.empty_selector_skip();
            #[cfg(feature = "unstable-workflow-profiling")]
            $crate::workflow_profile::plan_comparison::event(
                $crate::workflow_profile::plan_comparison::Site::EmptySelectorSkipped, 0);
            // The entry certificate proves the right suffix is the same slice.
            // Advance both ordinary producers; this is NOT terminal equality.
            continue $paired;
        }
        {
            paid!($budget, $function, shared_prefix::PROBE,
                "ordinary plan shared suffix pattern probe")?;
            #[cfg(test)] $census.identity_probe();
            #[cfg(feature = "unstable-workflow-profiling")]
            $crate::workflow_profile::plan_comparison::event(
                $crate::workflow_profile::plan_comparison::Site::IdentityProbeAdmitted,
                shared_prefix::PROBE);
                    paid!($budget, $function, certified_heads::RUN_CAPTURE,
                        "ordinary plan certified numeric run capture")?;
                    #[cfg(test)] $census.shared_runs();
                    #[cfg(feature = "unstable-workflow-profiling")]
                    $crate::workflow_profile::plan_comparison::event(
                        $crate::workflow_profile::plan_comparison::Site::NumericCaptureAdmitted,
                        certified_heads::RUN_CAPTURE);
                    let shared_length = $owner.left_items.len();
                    #[allow(clippy::manual_repeat_n, reason = "closed Take<Repeat<()>> invoice and independent original driver")]
                    let shared_work_driver: certified_heads::SharedUnitDriver =
                        std::iter::repeat(()).take(shared_length);
                    for () in shared_work_driver {
                        #[cfg(test)] $census.shared_turns();
                    #[cfg(feature = "unstable-workflow-profiling")]
                    $crate::workflow_profile::plan_comparison::event(
                        $crate::workflow_profile::plan_comparison::Site::NumericTurnAdmitted,
                        0);
                        $budget.charge($function, 1, "ordinary plan history forward cursor step")?;
                        #[cfg(test)] $census.identity_work();
                        #[cfg(feature = "unstable-workflow-profiling")]
                        $crate::workflow_profile::plan_comparison::event(
                            $crate::workflow_profile::plan_comparison::Site::IdentityWork, 0);
                        $budget.charge($function, 1, "ordinary plan history forward segment traversal")?;
                        #[cfg(test)] $census.identity_work();
                        #[cfg(feature = "unstable-workflow-profiling")]
                        $crate::workflow_profile::plan_comparison::event(
                            $crate::workflow_profile::plan_comparison::Site::IdentityWork, 0);
                        $budget.charge($function, 1, "ordinary plan history forward cursor step")?;
                        #[cfg(test)] $census.identity_work();
                        #[cfg(feature = "unstable-workflow-profiling")]
                        $crate::workflow_profile::plan_comparison::event(
                            $crate::workflow_profile::plan_comparison::Site::IdentityWork, 0);
                        $budget.charge($function, 1, "ordinary plan history forward segment traversal")?;
                        #[cfg(test)] $census.identity_work();
                        #[cfg(feature = "unstable-workflow-profiling")]
                        $crate::workflow_profile::plan_comparison::event(
                            $crate::workflow_profile::plan_comparison::Site::IdentityWork, 0);
                        #[cfg(test)] $census.identity_observation();
                        #[cfg(feature = "unstable-workflow-profiling")]
                        $crate::workflow_profile::plan_comparison::event(
                            $crate::workflow_profile::plan_comparison::Site::IdentityObservation, 0);
                        $budget.charge($function, 1, "ordinary plan history observation comparison")?;
                        #[cfg(test)] $census.identity_work();
                        #[cfg(feature = "unstable-workflow-profiling")]
                        $crate::workflow_profile::plan_comparison::event(
                            $crate::workflow_profile::plan_comparison::Site::IdentityWork, 0);
                        #[cfg(test)] $census.round();
                        #[cfg(feature = "unstable-workflow-profiling")]
                        {
                            $crate::workflow_profile::plan_comparison::event(
                                $crate::workflow_profile::plan_comparison::Site::Round, 0);
                        }
                    }
                    #[cfg(test)] $census.shared_turns(); // the retained driver's sole terminal None
                    #[cfg(feature = "unstable-workflow-profiling")]
                    $crate::workflow_profile::plan_comparison::event(
                        $crate::workflow_profile::plan_comparison::Site::NumericTurnAdmitted, 0);
                    paid!($budget, $function, certified_heads::RUN_PUBLISH,
                        "ordinary plan certified numeric run publication")?;
                    #[cfg(test)] $census.shared_publishes();
                    #[cfg(feature = "unstable-workflow-profiling")]
                    $crate::workflow_profile::plan_comparison::event(
                        $crate::workflow_profile::plan_comparison::Site::NumericPublishAdmitted,
                        certified_heads::RUN_PUBLISH);
                    $owner.left_items = &[];
                    $owner.right_items = &[];
                    $owner.left_cursor.item += shared_length;
                    $owner.right_cursor.item += shared_length;
                    $owner.ordering = Ordering::Equal;
        }
        // Publication has set BOTH retained slices to &[]; no second pattern,
        // probe, work, or result is needed to advance the ordinary producers.
        continue $paired;
    }};
}

// Both cursors have already exhausted one segment. The immutable prepared link
// stacks identify the SAME next nonempty segment, including on paths whose
// previous segments differed. Empty/different successors take the old route.
macro_rules! certified_boundary {
    ($owner:ident, $left_links:ident, $right_links:ident, $function:ident, $budget:ident, $census:ident, $certificate:lifetime) => {{
        paid!(
            $budget,
            $function,
            certified_heads::BOUNDARY_CHECK,
            "ordinary plan certified boundary loaded guards"
        )?;
        #[cfg(test)]
        $census.shared_checks();
        #[cfg(feature = "unstable-workflow-profiling")]
        $crate::workflow_profile::plan_comparison::event(
            $crate::workflow_profile::plan_comparison::Site::BoundaryChecksAdmitted,
            certified_heads::BOUNDARY_CHECK,
        );
        if $owner.left_loaded && $owner.right_loaded {
            paid!(
                $budget,
                $function,
                certified_heads::BOUNDARY_EMPTY,
                "ordinary plan certified boundary left empty"
            )?;
            #[cfg(test)]
            $census.shared_metadata();
            #[cfg(feature = "unstable-workflow-profiling")]
            $crate::workflow_profile::plan_comparison::event(
                $crate::workflow_profile::plan_comparison::Site::BoundaryLeftEmptyAdmitted,
                certified_heads::BOUNDARY_EMPTY,
            );
            if let [] = $owner.left_items {
                paid!(
                    $budget,
                    $function,
                    certified_heads::BOUNDARY_EMPTY,
                    "ordinary plan certified boundary right empty"
                )?;
                #[cfg(test)]
                $census.shared_empty_checks();
                #[cfg(feature = "unstable-workflow-profiling")]
                $crate::workflow_profile::plan_comparison::event(
                    $crate::workflow_profile::plan_comparison::Site::BoundaryRightEmptyAdmitted,
                    certified_heads::BOUNDARY_EMPTY,
                );
                if let [] = $owner.right_items {
                    paid!(
                        $budget,
                        $function,
                        certified_heads::BOUNDARY_LINKS,
                        "ordinary plan certified boundary next links"
                    )?;
                    #[cfg(test)]
                    $census.shared_links();
                    #[cfg(feature = "unstable-workflow-profiling")]
                    $crate::workflow_profile::plan_comparison::event(
                        $crate::workflow_profile::plan_comparison::Site::BoundaryLinksAdmitted,
                        certified_heads::BOUNDARY_LINKS,
                    );
                    if let &[ref left_links_rest @ .., left_next] = $left_links {
                        if let &[ref right_links_rest @ .., right_next] = $right_links {
                            if left_next == right_next {
                                paid!(
                                    $budget,
                                    $function,
                                    certified_heads::BOUNDARY_VIEW,
                                    "ordinary plan certified boundary next view"
                                )?;
                                #[cfg(test)]
                                $census.shared_queries();
                                #[cfg(feature = "unstable-workflow-profiling")]
                        $crate::workflow_profile::plan_comparison::event(
                            $crate::workflow_profile::plan_comparison::Site::BoundaryViewAdmitted,
                            certified_heads::BOUNDARY_VIEW);
                                let next_items = $owner.arena[left_next].items.as_slice();
                                if let [_, rest @ ..] = next_items {
                                    paid!(
                                        $budget,
                                        $function,
                                        certified_heads::BOUNDARY_BODY,
                                        "ordinary plan certified boundary transition"
                                    )?;
                                    #[cfg(test)]
                                    $census.shared_boundaries();
                                    #[cfg(feature = "unstable-workflow-profiling")]
                        $crate::workflow_profile::plan_comparison::event(
                            $crate::workflow_profile::plan_comparison::Site::BoundaryBodyAdmitted,
                            certified_heads::BOUNDARY_BODY);
                                    $budget.charge(
                                        $function,
                                        1,
                                        "ordinary plan history forward cursor step",
                                    )?;
                                    $budget.charge(
                                        $function,
                                        1,
                                        "ordinary plan history forward segment traversal",
                                    )?;
                                    $owner.left_cursor.segment += 1;
                                    $owner.left_cursor.item = 0;
                                    $owner.left_loaded = false;
                                    $budget.charge(
                                        $function,
                                        1,
                                        "ordinary plan history forward segment traversal",
                                    )?;
                                    $left_links = left_links_rest;
                                    $owner.left_items = rest;
                                    $owner.left_loaded = true;
                                    $owner.left_cursor.item = 1;
                                    $budget.charge(
                                        $function,
                                        1,
                                        "ordinary plan history forward cursor step",
                                    )?;
                                    $budget.charge(
                                        $function,
                                        1,
                                        "ordinary plan history forward segment traversal",
                                    )?;
                                    $owner.right_cursor.segment += 1;
                                    $owner.right_cursor.item = 0;
                                    $owner.right_loaded = false;
                                    $budget.charge(
                                        $function,
                                        1,
                                        "ordinary plan history forward segment traversal",
                                    )?;
                                    $right_links = right_links_rest;
                                    $owner.right_loaded = true;
                                    $owner.right_cursor.item = 1;
                                    $owner.ordering = Ordering::Equal;
                                    $budget.charge(
                                        $function,
                                        1,
                                        "ordinary plan history observation comparison",
                                    )?;
                                    #[cfg(test)]
                                    $census.round();
                                    #[cfg(test)]
                                    $census.shared_boundary_complete();
                                    #[cfg(feature = "unstable-workflow-profiling")]
                        $crate::workflow_profile::plan_comparison::event(
                            $crate::workflow_profile::plan_comparison::Site::BoundaryCompleted,
                            0);
                                    break $certificate;
                                }
                            }
                        }
                    }
                }
            }
        }
    }};
}
