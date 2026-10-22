//! Independent PLAN/HIR content comparison; no cross-input identity certificate.
use super::*;
// The driver eliminates PLAN Option/return transport. Keep every other STEP,
// EMPTY and TAIL role, including their original conservative empty envelopes.
pub(super) const STEP: usize = 2 * size_of::<usize>()
    + 3 * size_of::<bool>()
    + size_of::<&Rc<SkeletonObservation>>()
    + 2 * size_of::<&[Rc<SkeletonObservation>]>()
    + 2 * size_of::<&Rc<SkeletonObservation>>()
    + size_of::<&SkeletonObservation>()
    + size_of::<usize>();
pub(super) const TAIL: usize = size_of::<&Rc<SkeletonObservation>>()
    + 2 * size_of::<&[Rc<SkeletonObservation>]>()
    + 2 * size_of::<&Rc<SkeletonObservation>>()
    + size_of::<&SkeletonObservation>()
    + size_of::<usize>();
// Right head, independent full-content equality actual/formal pair, right
// as_ref actual/formal/return, and real rest pattern/assignment. Equality and
// the terminal empty pattern use the same mutually exclusive bool envelope.
pub(super) const FLAT_MATCH: usize = size_of::<&SkeletonObservation>() // left receiver, before right advance
    + size_of::<&Rc<SkeletonObservation>>()
    + 4 * size_of::<&SkeletonObservation>()
    + 2 * size_of::<&Rc<SkeletonObservation>>() + size_of::<&SkeletonObservation>()
    + 2 * size_of::<&[Rc<SkeletonObservation>]>() + size_of::<bool>();
// The HIR cursor's original Result/Option transport remains fully funded by
// CURSOR_CALL. This bank retains its separately received Option and Some
// binding, independent equality references and bool, but no PLAN return slots.
pub(super) const SHARED_MATCH: usize = size_of::<&SkeletonObservation>() // left receiver, before right advance
    + size_of::<Option<&SkeletonObservation>>()
    + size_of::<&SkeletonObservation>() + 4 * size_of::<&SkeletonObservation>()
    + size_of::<bool>();
// Each route now has segment-head, tail-head and empty-PLAN mismatch sites,
// rather than one common match fallback: four additional full helper packets.
// There is no new compiler owner, closure, callback, owned backing or drop.
// The two five-site lexical primitive banks correspond one-to-one to the old
// two final producers. Their complete old headers remain in OWNER_CONTROLS.
pub(super) const OWNER: usize = 4 * MISMATCH_CALL;

macro_rules! final_affine_admitted {
    ($census:ident, $method:ident, $site:ident, $bytes:expr) => {{
        #[cfg(test)]
        $census.$method();
        #[cfg(feature = "unstable-workflow-profiling")]
        crate::workflow_profile::plan_comparison::event(
            crate::workflow_profile::plan_comparison::Site::$site,
            $bytes,
        );
    }};
}
macro_rules! final_affine_round {
    ($census:ident, $function:ident, $budget:ident, flat) => {{
        paid!(
            $budget,
            $function,
            final_affine::FLAT_MATCH,
            "ordinary plan affine flat HIR match"
        )?;
        final_affine_admitted!(
            $census,
            final_flat,
            FinalFlatAdmitted,
            final_affine::FLAT_MATCH
        );
    }};
    ($census:ident, $function:ident, $budget:ident, shared) => {{
        paid!(
            $budget,
            $function,
            final_affine::SHARED_MATCH,
            "ordinary plan affine shared HIR match"
        )?;
        final_affine_admitted!(
            $census,
            final_shared,
            FinalSharedAdmitted,
            final_affine::SHARED_MATCH
        );
        $budget.reserve_bytes(
            $function,
            skeleton_work::enumerated_prefix::CURSOR_CALL,
            "ordinary PLAN shared HIR cursor next receivers",
        )?;
    }};
}
macro_rules! final_affine_head {
    ($head:ident, $hir:ident, $function:ident, $budget:ident, flat) => {{
        let left = $head.as_ref();
        match $hir {
            [right, rest @ ..] if left == right.as_ref() => {
                $hir = rest;
            }
            _ => {
                return Err(replay_error(
                    $function,
                    "cleanup CFG decision or ownership-event sequence disagrees with typed HIR",
                ));
            }
        }
    }};
    ($head:ident, $hir:ident, $function:ident, $budget:ident, shared) => {{
        let left = $head.as_ref();
        match $hir.next($function, $budget)? {
            Some(right) if left == right => {}
            _ => {
                return Err(replay_error(
                    $function,
                    "cleanup CFG decision or ownership-event sequence disagrees with typed HIR",
                ));
            }
        }
    }};
}
macro_rules! final_affine_empty {
    ($hir:ident, $function:ident, $budget:ident, flat) => {{
        match $hir {
            [] => {}
            _ => {
                return Err(replay_error(
                    $function,
                    "cleanup CFG decision or ownership-event sequence disagrees with typed HIR",
                ));
            }
        }
    }};
    ($hir:ident, $function:ident, $budget:ident, shared) => {{
        match $hir.next($function, $budget)? {
            None => {}
            _ => {
                return Err(replay_error(
                    $function,
                    "cleanup CFG decision or ownership-event sequence disagrees with typed HIR",
                ));
            }
        }
    }};
}
macro_rules! final_affine_driver {
    ($owner:ident, $tail:ident, $census:ident, $function:ident, $budget:ident,
     $route:ident, $hir:ident) => {{
        'final_hir: loop {
            $budget.charge($function, 1, "ordinary plan history HIR comparison")?;
            final_affine_round!($census, $function, $budget, $route);
            $budget.charge($function, 1, "ordinary plan history forward cursor step")?;
            loop {
                paid!(
                    $budget,
                    $function,
                    final_affine::STEP + lexical_cursor::EMPTY,
                    "ordinary plan affine HIR segment"
                )?;
                #[cfg(test)]
                $census.step();
                final_affine_admitted!(
                    $census,
                    final_step,
                    FinalStepAdmitted,
                    final_affine::STEP + lexical_cursor::EMPTY
                );
                if $owner.left_cursor.segment >= $owner.left_segments {
                    break;
                }
                $budget.charge(
                    $function,
                    1,
                    "ordinary plan history forward segment traversal",
                )?;
                if !$owner.left_loaded {
                    paid!(
                        $budget,
                        $function,
                        lexical_cursor::LOAD,
                        "ordinary plan history HIR cursor next"
                    )?;
                    #[cfg(test)]
                    $census.load();
                    let Some(link) = $owner.left_cursor.links.pop() else {
                        unreachable!("prepared ordinary plan links");
                    };
                    $owner.left_items = $owner.arena[link].items.as_slice();
                    $owner.left_loaded = true;
                }
                match $owner.left_items {
                    [head, rest @ ..] => {
                        $owner.left_items = rest;
                        $owner.left_cursor.item += 1;
                        final_affine_head!(head, $hir, $function, $budget, $route);
                        continue 'final_hir;
                    }
                    [] => {
                        $owner.left_cursor.segment += 1;
                        $owner.left_cursor.item = 0;
                        $owner.left_loaded = false;
                    }
                }
            }
            paid!(
                $budget,
                $function,
                final_affine::TAIL,
                "ordinary plan affine HIR tail"
            )?;
            #[cfg(test)]
            $census.tail();
            final_affine_admitted!($census, final_tail, FinalTailAdmitted, final_affine::TAIL);
            match $tail {
                [head, rest @ ..] => {
                    $tail = rest;
                    $owner.left_cursor.item += 1;
                    final_affine_head!(head, $hir, $function, $budget, $route);
                    continue 'final_hir;
                }
                [] => {
                    final_affine_empty!($hir, $function, $budget, $route);
                    break 'final_hir;
                }
            }
        }
    }};
}

// This complete-only composition uses actual admitted new banks. The immediate
// independently compiled producer control verifies equal counts for every
// route and lexical segment/tail visit; prefixes never use this cancellation.
#[cfg(test)]
pub(super) fn delta(row: &pair_scope_census::Row) -> (usize, usize) {
    assert!(row.complete && !row.overflow && !row.truncated && row.ended);
    let removed_options = 2 * size_of::<Option<&SkeletonObservation>>();
    assert_eq!(STEP + removed_options, lexical_cursor::STEP);
    assert_eq!(TAIL + removed_options, lexical_cursor::TAIL);
    let flat_removed = size_of::<(
        Option<&SkeletonObservation>,
        Option<&Rc<SkeletonObservation>>,
    )>() + 2 * size_of::<&[Rc<SkeletonObservation>]>()
        + 2 * size_of::<usize>()
        + size_of::<Option<&Rc<SkeletonObservation>>>();
    let flat_added = 2 * size_of::<&[Rc<SkeletonObservation>]>();
    assert_eq!(FLAT_MATCH + flat_removed, super::FLAT_MATCH + flat_added);
    let shared_removed = size_of::<Option<&SkeletonObservation>>();
    assert_eq!(SHARED_MATCH + shared_removed, super::SHARED_MATCH);
    (
        OWNER + flat_added * row.final_flat,
        removed_options * (row.final_steps + row.final_tails)
            + flat_removed * row.final_flat
            + shared_removed * row.final_shared,
    )
}
