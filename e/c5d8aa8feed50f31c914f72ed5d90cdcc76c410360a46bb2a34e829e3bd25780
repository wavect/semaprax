//! Loaded-segment remainder only. No graph walk, observation loan or content cmp.
use super::{size_of, Ordering, Rc, SkeletonObservation, PAYMENT_HEADER};
// The complete current owner size is already paid by OWNER_CONTROLS. Only
// these three scalar initializers and mode initializer are additive here.
// Seven distinct new reserve/work primitive sites remain independently paid.
pub(super) const OWNER: usize = 3 * size_of::<usize>() + size_of::<bool>() + 7 * PAYMENT_HEADER;
// Both sorting LOADs capture their actual popped link. Only the left LOAD
// receives Slice::len actual/formal/return; same-ID proves the right extent.
pub(super) const CAPTURE_LINK: usize = size_of::<usize>();
pub(super) const CAPTURE_EXTENT: usize =
    2 * size_of::<&[Rc<SkeletonObservation>]>() + size_of::<usize>();
macro_rules! shared_prefix_capture_price {
    ([]) => {
        shared_prefix::CAPTURE_LINK
    };
    ([left_extent]) => {
        shared_prefix::CAPTURE_LINK + shared_prefix::CAPTURE_EXTENT
    };
}
// Nested branches form no aggregate boolean or callback. Two loaded predicates,
// three scalar comparisons and six copied comparison operands; fallback resets
// the mode bit. All are paid even when an early predicate rejects the route.
pub(super) const DETECTOR: usize = 6 * size_of::<usize>() + 8 * size_of::<bool>();
// Two old successful-pattern reference invoices remain conservative admission.
// Actual replay forms two item increments, an Equal value, end comparison and
// its operands, and two empty-slice field values. Mode test/negation/set are
// distinct boolean values, with no transported Option/tuple/reference result.
pub(super) const REPLAY: usize = 2 * size_of::<&SkeletonObservation>()
    + 4 * size_of::<usize>()
    + size_of::<Ordering>()
    + 4 * size_of::<bool>()
    + 2 * size_of::<&[Rc<SkeletonObservation>]>();

macro_rules! replay_shared_prefix {
    ($owner:ident, $function:ident, $budget:ident, $census:ident, $paired:lifetime) => {{
        paid!($budget, $function, shared_prefix::DETECTOR,
            "ordinary plan shared prefix identity detector")?;
        #[cfg(test)]
        $census.identity_detector();
        #[cfg(feature = "unstable-workflow-profiling")]
        $crate::workflow_profile::plan_comparison::event(
            $crate::workflow_profile::plan_comparison::Site::IdentityDetectorAdmitted, shared_prefix::DETECTOR);
        'shared_certificate: {
            if !$owner.left_loaded { break 'shared_certificate; }
            if !$owner.right_loaded { break 'shared_certificate; }
            if $owner.left_link != $owner.right_link { break 'shared_certificate; }
            if $owner.left_cursor.item != $owner.right_cursor.item { break 'shared_certificate; }
            if $owner.left_cursor.item >= $owner.left_extent { break 'shared_certificate; }
            // The retained left extent describes the same immutable Segment
            // in both views. First observation and every boundary have
            // already used the ordinary path; only its nonempty remainder enters.
            paid!($budget, $function, shared_prefix::REPLAY,
                "ordinary plan shared prefix scalar receivers")?;
            #[cfg(feature = "unstable-workflow-profiling")]
            $crate::workflow_profile::plan_comparison::event(
                $crate::workflow_profile::plan_comparison::Site::IdentityReplayAdmitted,
                shared_prefix::REPLAY);
            if !$owner.prefix_replaying {
                #[cfg(test)]
                $census.identity_hit();
                #[cfg(feature = "unstable-workflow-profiling")]
                $crate::workflow_profile::plan_comparison::event(
                    $crate::workflow_profile::plan_comparison::Site::IdentityHit, 0);
                $owner.prefix_replaying = true;
            }
            $budget.charge($function, 1, "ordinary plan history forward cursor step")?;
            #[cfg(test)]
            $census.identity_work();
            #[cfg(feature = "unstable-workflow-profiling")]
            $crate::workflow_profile::plan_comparison::event(
                $crate::workflow_profile::plan_comparison::Site::IdentityWork, 0);
            $budget.charge($function, 1, "ordinary plan history forward segment traversal")?;
            #[cfg(test)]
            $census.identity_work();
            #[cfg(feature = "unstable-workflow-profiling")]
            $crate::workflow_profile::plan_comparison::event(
                $crate::workflow_profile::plan_comparison::Site::IdentityWork, 0);
            $owner.left_cursor.item += 1;
            $budget.charge($function, 1, "ordinary plan history forward cursor step")?;
            #[cfg(test)]
            $census.identity_work();
            #[cfg(feature = "unstable-workflow-profiling")]
            $crate::workflow_profile::plan_comparison::event(
                $crate::workflow_profile::plan_comparison::Site::IdentityWork, 0);
            $budget.charge($function, 1, "ordinary plan history forward segment traversal")?;
            #[cfg(test)]
            $census.identity_work();
            #[cfg(feature = "unstable-workflow-profiling")]
            $crate::workflow_profile::plan_comparison::event(
                $crate::workflow_profile::plan_comparison::Site::IdentityWork, 0);
            $owner.right_cursor.item += 1;
            $owner.ordering = Ordering::Equal;
            if $owner.left_cursor.item == $owner.left_extent {
                // While replaying, stale remaining slices are never read. Clear
                // them at run end, leaving loaded/segment unchanged so original
                // [] traversal, following empty segments and tails remain LTR.
                $owner.left_items = &[];
                $owner.right_items = &[];
            }
            #[cfg(test)]
            $census.identity_observation();
            #[cfg(feature = "unstable-workflow-profiling")]
            $crate::workflow_profile::plan_comparison::event(
                $crate::workflow_profile::plan_comparison::Site::IdentityObservation, 0);
            $budget.charge($function, 1, "ordinary plan history observation comparison")?;
            #[cfg(test)]
            $census.identity_work();
            #[cfg(feature = "unstable-workflow-profiling")]
            $crate::workflow_profile::plan_comparison::event(
                $crate::workflow_profile::plan_comparison::Site::IdentityWork, 0);
            #[cfg(test)]
            $census.round();
            #[cfg(feature = "unstable-workflow-profiling")]
            $crate::workflow_profile::plan_comparison::event(
                $crate::workflow_profile::plan_comparison::Site::Round, 0);
            continue $paired;
        }
        $owner.prefix_replaying = false;
    }};
}
