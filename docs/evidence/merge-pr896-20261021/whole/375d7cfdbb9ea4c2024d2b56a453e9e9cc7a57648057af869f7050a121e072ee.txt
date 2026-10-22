//! Both first-head continuations keep the original popped links lexical.
use super::{Ordering, PAYMENT_HEADER, Rc, SkeletonObservation, size_of};
// One extra right expansion (five sites), two duplicated first-equal work
// sites, event/probe/body payment sites, and five suffix work sites. No new
// owner fields, captures, len queries, reset flags or round-level detector.
pub(super) const OWNER: usize = 15 * PAYMENT_HEADER;
pub(super) const DETECTOR: usize = 2 * size_of::<usize>() + size_of::<bool>();
// A direct slice pattern: rest binding and its two actual field assignments.
// This complete branch envelope is also acquired for the terminal [] probe.
pub(super) const PROBE: usize = 3 * size_of::<&[Rc<SkeletonObservation>]>();
// Retain the two old successful-pattern reference invoices conservatively;
// only two actual item increments and the Equal initializer remain here.
pub(super) const REPLAY: usize = 2 * size_of::<&SkeletonObservation>()
    + 2 * size_of::<usize>() + size_of::<Ordering>();

macro_rules! shared_prefix_first_comparison {
    ($owner:ident, $left:ident, $right:ident, $function:ident, $budget:ident,
     $census:ident, $paired:lifetime, $equal:block) => {{
        // This is the same PAIR_MATCH-funded comparison/receiving boundary,
        // relocated into a mutually exclusive first-head continuation.
        $owner.ordering = $left.cmp($right);
        if $owner.ordering == Ordering::Equal {
            $budget.charge($function, 1, "ordinary plan history observation comparison")?;
            #[cfg(test)]
            $census.round();
            #[cfg(feature = "unstable-workflow-profiling")]
            $crate::workflow_profile::plan_comparison::event(
                $crate::workflow_profile::plan_comparison::Site::Round, 0);
            $equal
        }
        break $paired;
    }};
}
macro_rules! replay_shared_prefix {
    ($owner:ident, $left_link:ident, $right_link:ident, $function:ident,
     $budget:ident, $census:ident, $paired:lifetime) => {{
        // Both original first observations and their cmp have already run.
        // The next original comparison-work refusal precedes this new event.
        paid!($budget, $function, shared_prefix::DETECTOR,
            "ordinary plan fresh segment identity event")?;
        #[cfg(test)]
        $census.identity_detector();
        #[cfg(feature = "unstable-workflow-profiling")]
        $crate::workflow_profile::plan_comparison::event(
            $crate::workflow_profile::plan_comparison::Site::IdentityDetectorAdmitted,
            shared_prefix::DETECTOR);
        if $left_link != $right_link { continue $paired; }
        // Same arena + two lexical first heads prove identical remaining
        // slices. No certificate survives this loop, preparation or swap.
        #[cfg(test)]
        $census.identity_hit();
        #[cfg(feature = "unstable-workflow-profiling")]
        $crate::workflow_profile::plan_comparison::event(
            $crate::workflow_profile::plan_comparison::Site::IdentityHit, 0);
        loop {
            paid!($budget, $function, shared_prefix::PROBE,
                "ordinary plan shared suffix pattern probe")?;
            #[cfg(test)]
            $census.identity_probe();
            #[cfg(feature = "unstable-workflow-profiling")]
            $crate::workflow_profile::plan_comparison::event(
                $crate::workflow_profile::plan_comparison::Site::IdentityProbeAdmitted,
                shared_prefix::PROBE);
            match $owner.left_items {
                [_, rest @ ..] => {
                    paid!($budget, $function, shared_prefix::REPLAY,
                        "ordinary plan shared suffix scalar receivers")?;
                    #[cfg(test)] $census.identity_body();
                    #[cfg(feature = "unstable-workflow-profiling")]
                    $crate::workflow_profile::plan_comparison::event(
                        $crate::workflow_profile::plan_comparison::Site::IdentityReplayAdmitted,
                        shared_prefix::REPLAY);
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
                    $owner.left_items = rest;
                    $owner.left_cursor.item += 1;
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
                    $owner.right_items = rest;
                    $owner.right_cursor.item += 1;
                    $owner.ordering = Ordering::Equal;
                    #[cfg(test)] $census.identity_observation();
                    #[cfg(feature = "unstable-workflow-profiling")]
                    $crate::workflow_profile::plan_comparison::event(
                        $crate::workflow_profile::plan_comparison::Site::IdentityObservation, 0);
                    $budget.charge($function, 1, "ordinary plan history observation comparison")?;
                    #[cfg(test)] $census.identity_work();
                    #[cfg(test)] $census.round();
                    #[cfg(feature = "unstable-workflow-profiling")]
                    {
                        $crate::workflow_profile::plan_comparison::event(
                            $crate::workflow_profile::plan_comparison::Site::IdentityWork, 0);
                        $crate::workflow_profile::plan_comparison::event(
                            $crate::workflow_profile::plan_comparison::Site::Round, 0);
                    }
                }
                [] => {
                    #[cfg(test)] $census.identity_empty_probe();
                    #[cfg(feature = "unstable-workflow-profiling")]
                    $crate::workflow_profile::plan_comparison::event(
                        $crate::workflow_profile::plan_comparison::Site::IdentityEmptyProbe, 0);
                    break;
                }
            }
        }
        // Leave item/segment/loaded exactly at the old exhausted-segment
        // frontier. The normal next round owns empty segments and tails.
        continue $paired;
    }};
}
