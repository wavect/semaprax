//! Arena-backed immutable slices survive calls; no path-tail borrow survives a match.
use super::*;

// Exactly the accepted880 narrow primitive criterion: same function/budget,
// sequential nonescaping nonrecursive ZST-success/one-terminal-Diagnostic slots.
// Four expansions each have forward+segment work and STEP+LOAD+TAIL payment.
// The shared immutable-step branch adds six payment and five work sites.
// Every payload below is separately paid on actual execution, never once.
pub(super) const PRIMITIVE_FRAMES: usize = 42 * PAYMENT_HEADER;
pub(super) const SINGLE_PRIMITIVE_FRAMES: usize = 5 * PAYMENT_HEADER;
// Before inspecting the already-funded owner length, fund the new payment
// site's complete primitive frame, match/local usize receivers and both
// numeric pattern decisions. No dynamic payload uses this primitive bank.
pub(super) const PRIMITIVE_SELECTION: usize =
    PAYMENT_HEADER + 3 * size_of::<usize>() + 2 * size_of::<bool>();
pub(super) const INITIALIZERS: usize = 2 * size_of::<&Vec<Segment>>() + 2 * size_of::<&[Segment]>() // arena as_slice/field initializer
    + 2 * size_of::<&[Rc<SkeletonObservation>]>() // two empty-slice initializers; fields in Owner
    + 2 * size_of::<bool>();
pub(super) const RESET: usize = size_of::<&[Rc<SkeletonObservation>]>() + size_of::<bool>();
// A fresh lexical slice pattern; no get Option, index, callee/formal, compound
// Result or items-view construction is hidden in a paid-once bank.
pub(super) const STEP: usize = 2 * size_of::<usize>() + size_of::<bool>() // retained segment extent and condition
    + 2 * size_of::<bool>() // loaded condition and negation
    + size_of::<&Rc<SkeletonObservation>>() // head pattern
    + 2 * size_of::<&[Rc<SkeletonObservation>]>() // rest pattern and assignment copy
    + 2 * size_of::<&Rc<SkeletonObservation>>() + size_of::<&SkeletonObservation>() // as_ref actual/formal/return
    + 2 * size_of::<Option<&SkeletonObservation>>() // Some initializer and caller-received output
    + size_of::<usize>(); // original item increment
                          // Only entering a new immutable segment forms these views/index receivers.
pub(super) const LIMIT: usize = 2 * size_of::<&Vec<usize>>() + 2 * size_of::<usize>();
// Prepared backward links are consumed from the back. Their original extent
// remains in the comparison owner; the cursor segment/item counters stay exact.
pub(super) const LOAD: usize = 2 * size_of::<&mut Vec<usize>>()
    + size_of::<Option<usize>>()
    + size_of::<usize>()
    + size_of::<bool>()
    + 2 * size_of::<&[Segment]>()
    + 2 * size_of::<usize>()
    + size_of::<&Segment>()
    + 2 * size_of::<&Vec<Rc<SkeletonObservation>>>()
    + 2 * size_of::<&[Rc<SkeletonObservation>]>()
    + size_of::<bool>() // loaded=true
    + size_of::<usize>(); // remember exact validated segment identity
                          // The original exhausted-segment update remains above. Each tail scope forms
                          // one immutable Vec slice; each yielded head/rest, assignment, as_ref output,
                          // Option transport and original cursor item increment is still paid below.
pub(super) const EMPTY: usize = 2 * size_of::<usize>() + size_of::<bool>(); // segment increment, item=0, loaded=false

pub(super) const TAIL_VIEW: usize =
    2 * size_of::<&Vec<Rc<SkeletonObservation>>>() + 2 * size_of::<&[Rc<SkeletonObservation>]>();
pub(super) const TAIL: usize = size_of::<&Rc<SkeletonObservation>>()
    + 2 * size_of::<&[Rc<SkeletonObservation>]>()
    + 2 * size_of::<&Rc<SkeletonObservation>>()
    + size_of::<&SkeletonObservation>()
    + 2 * size_of::<Option<&SkeletonObservation>>()
    + size_of::<usize>();
// Exact shipping880 literal expression, solely a static counterfactual bound;
// the actual independently compiled before producer and its tariff stay exact.
const SHIPPING_NEXT_CALL: usize = 2 * size_of::<&mut Cursor>()
    + 2 * size_of::<&History>()
    + 2 * size_of::<&[Segment]>()
    + 2 * size_of::<&ResolvedFunction>()
    + 2 * size_of::<&mut ReplayBudget>()
    + size_of::<Result<Option<&SkeletonObservation>, Diagnostic>>()
    + size_of::<Option<&SkeletonObservation>>();
const _: () = assert!(STEP + EMPTY < SHIPPING_NEXT_CALL);
// Conservative source-type opportunity for a pair with256 equal observations
// each in one full immutable segment, followed by empty tails. This pays the
// ENTIRE current owner (no old-owner fee credited), both transition/reset bills,
// and full exhausted sentinel/tail envelopes. It is not an application result.
const _: () = assert!(
    2 * 256 * (SHIPPING_NEXT_CALL - STEP - EMPTY)
        > OWNER_CONTROLS
            + PRIMITIVE_FRAMES
            + 2 * LOAD
            + 2 * RESET
            + 4 * (STEP + EMPTY)
            + 2 * TAIL
            + 4 * TAIL_VIEW
);

macro_rules! lexical_plan_next {
    ($owner:ident, $cursor:ident, $items:ident, $loaded:ident, $limit:ident,
     $tail:ident, $census:ident, $function:ident, $budget:ident, $phase:literal) => {{
        $budget.charge($function, 1, "ordinary plan history forward cursor step")?;
        'plan_yield: {
            loop {
                paid!(
                    $budget,
                    $function,
                    lexical_cursor::STEP + lexical_cursor::EMPTY,
                    $phase
                )?;
                #[cfg(test)]
                $census.step();
                if $owner.$cursor.segment >= $owner.$limit {
                    break;
                }
                $budget.charge(
                    $function,
                    1,
                    "ordinary plan history forward segment traversal",
                )?;
                if !$owner.$loaded {
                    paid!($budget, $function, lexical_cursor::LOAD, $phase)?;
                    #[cfg(test)]
                    $census.load();
                    let Some(link) = $owner.$cursor.links.pop() else {
                        unreachable!("prepared ordinary plan links");
                    };
                    remember_plan_segment!($owner, $cursor, link);
                    $owner.$items = $owner.arena[link].items.as_slice();
                    $owner.$loaded = true;
                }
                match $owner.$items {
                    [head, rest @ ..] => {
                        $owner.$items = rest;
                        $owner.$cursor.item += 1;
                        break 'plan_yield Some::<&SkeletonObservation>(head.as_ref());
                    }
                    [] => {
                        $owner.$cursor.segment += 1;
                        $owner.$cursor.item = 0;
                        $owner.$loaded = false;
                    }
                }
            }
            paid!($budget, $function, lexical_cursor::TAIL, $phase)?;
            #[cfg(test)]
            $census.tail();
            match $tail {
                [head, rest @ ..] => {
                    $tail = rest;
                    $owner.$cursor.item += 1;
                    Some::<&SkeletonObservation>(head.as_ref())
                }
                [] => None,
            }
        }
    }};
}

// The arena and its observations are immutable during comparison. A loaded
// view is exactly the remembered validated segment starting at cursor.item.
// Same segment and item therefore prove equality of the next observation.
macro_rules! remember_plan_segment {
    ($owner:ident, left_cursor, $link:ident) => {
        $owner.left_link = $link;
    };
    ($owner:ident, right_cursor, $link:ident) => {
        $owner.right_link = $link;
    };
}

// Six payment and five work sites have distinct complete primitive frames in
// the multipath bank. Every actual selector, slice receiver and scalar copy
// remains separately paid before formation. No observation Option/tuple/cmp
// payload is formed on this branch. Original left-before-right work is exact.
macro_rules! shared_plan_pair_step {
    ($owner:ident, $census:ident, $function:ident, $budget:ident, $phase:literal) => {
        paid!($budget, $function, 3 * size_of::<bool>(), $phase)?;
        #[cfg(test)]
        $census.shared_checks();
        if $owner.left_loaded && $owner.right_loaded {
            paid!($budget, $function, size_of::<bool>(), $phase)?;
            #[cfg(test)]
            $census.shared_patterns();
            if let [_, ..] = $owner.left_items {
                paid!(
                    $budget,
                    $function,
                    4 * size_of::<usize>() + 3 * size_of::<bool>(),
                    $phase
                )?;
                #[cfg(test)]
                $census.shared_metadata();
                if $owner.left_link == $owner.right_link
                    && $owner.left_cursor.item == $owner.right_cursor.item
                {
                    shared_plan_segment_run!($owner, $census, $function, $budget, $phase);
                }
            } else {
                shared_plan_boundary!($owner, $census, $function, $budget, $phase);
            }
        }
    };
}

// A loaded empty slice and a remaining validated link describe exactly one
// exhausted-to-next-segment transition. Inspect only that next immutable
// segment; empty or divergent successors use the complete original traversal.
// Four payment and seven work sites have distinct full multipath headers.
macro_rules! shared_plan_boundary {
    ($owner:ident, $census:ident, $function:ident, $budget:ident, $phase:literal) => {
        paid!($budget, $function, size_of::<bool>(), $phase)?;
        #[cfg(test)]
        $census.shared_empty_checks();
        if let [] = $owner.right_items {
            paid!(
                $budget,
                $function,
                4 * size_of::<&Vec<usize>>()
                    + 4 * size_of::<&[usize]>()
                    + 4 * size_of::<usize>()
                    + 3 * size_of::<bool>(),
                $phase
            )?;
            #[cfg(test)]
            $census.shared_links();
            let left_links = $owner.left_cursor.links.as_slice();
            let right_links = $owner.right_cursor.links.as_slice();
            if let &[.., left_next] = left_links {
                if let &[.., right_next] = right_links {
                    if left_next == right_next {
                        paid!(
                            $budget,
                            $function,
                            2 * size_of::<&[Segment]>()
                                + 2 * size_of::<usize>()
                                + size_of::<&Segment>()
                                + 2 * size_of::<&Vec<Rc<SkeletonObservation>>>()
                                + 3 * size_of::<&[Rc<SkeletonObservation>]>()
                                + size_of::<bool>(),
                            $phase
                        )?;
                        #[cfg(test)]
                        $census.shared_queries();
                        let next_items = $owner.arena[left_next].items.as_slice();
                        if let [_, rest @ ..] = next_items {
                            paid!(
                                $budget,
                                $function,
                                4 * size_of::<&mut Vec<usize>>()
                                    + 2 * size_of::<Option<usize>>()
                                    + 10 * size_of::<usize>()
                                    + 6 * size_of::<bool>()
                                    + 2 * size_of::<&[Rc<SkeletonObservation>]>()
                                    + size_of::<Ordering>(),
                                $phase
                            )?;
                            #[cfg(test)]
                            $census.shared_boundaries();
                            $budget.charge(
                                $function,
                                1,
                                "ordinary plan history forward cursor step",
                            )?;
                            #[cfg(test)]
                            $census.step();
                            $budget.charge(
                                $function,
                                1,
                                "ordinary plan history forward segment traversal",
                            )?;
                            $owner.left_cursor.segment += 1;
                            $owner.left_cursor.item = 0;
                            $owner.left_loaded = false;
                            #[cfg(test)]
                            $census.step();
                            $budget.charge(
                                $function,
                                1,
                                "ordinary plan history forward segment traversal",
                            )?;
                            #[cfg(test)]
                            $census.load();
                            let Some(left_link) = $owner.left_cursor.links.pop() else {
                                unreachable!("validated ordinary plan next segment");
                            };
                            $owner.left_link = left_link;
                            $owner.left_items = rest;
                            $owner.left_loaded = true;
                            $owner.left_cursor.item = 1;
                            $budget.charge(
                                $function,
                                1,
                                "ordinary plan history forward cursor step",
                            )?;
                            #[cfg(test)]
                            $census.step();
                            $budget.charge(
                                $function,
                                1,
                                "ordinary plan history forward segment traversal",
                            )?;
                            $owner.right_cursor.segment += 1;
                            $owner.right_cursor.item = 0;
                            $owner.right_loaded = false;
                            #[cfg(test)]
                            $census.step();
                            $budget.charge(
                                $function,
                                1,
                                "ordinary plan history forward segment traversal",
                            )?;
                            #[cfg(test)]
                            $census.load();
                            let Some(right_link) = $owner.right_cursor.links.pop() else {
                                unreachable!("validated ordinary plan next segment");
                            };
                            $owner.right_link = right_link;
                            $owner.right_items = rest;
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
                            continue;
                        }
                    }
                }
            }
        }
    };
}

// Both retained views name the same immutable segment at the same offset.
// Replay every original work request, but form only numeric loop carriers.
// Private borrowed views/item positions publish after the whole run. An error
// returns immediately: these non-owning fields cannot affect a Diagnostic,
// owned Drop, Rc count or any externally visible state before that return.
macro_rules! shared_plan_segment_run {
    ($owner:ident, $census:ident, $function:ident, $budget:ident, $phase:literal) => {
        paid!(
            $budget,
            $function,
            2 * size_of::<&[Rc<SkeletonObservation>]>() + 3 * size_of::<usize>(),
            $phase
        )?;
        #[cfg(test)]
        $census.shared_runs();
        let shared_length = $owner.left_items.len();
        let mut shared_remaining = shared_length;
        loop {
            // Full condition/decrement envelope, including the zero sentinel.
            paid!(
                $budget,
                $function,
                4 * size_of::<usize>() + size_of::<bool>(),
                $phase
            )?;
            #[cfg(test)]
            $census.shared_turns();
            if shared_remaining == 0 {
                break;
            }
            #[cfg(test)]
            $census.shared_steps();
            $budget.charge($function, 1, "ordinary plan history forward cursor step")?;
            #[cfg(test)]
            $census.step();
            $budget.charge(
                $function,
                1,
                "ordinary plan history forward segment traversal",
            )?;
            $budget.charge($function, 1, "ordinary plan history forward cursor step")?;
            #[cfg(test)]
            $census.step();
            $budget.charge(
                $function,
                1,
                "ordinary plan history forward segment traversal",
            )?;
            $budget.charge($function, 1, "ordinary plan history observation comparison")?;
            #[cfg(test)]
            $census.round();
            shared_remaining -= 1;
        }
        paid!(
            $budget,
            $function,
            4 * size_of::<&[Rc<SkeletonObservation>]>()
                + 4 * size_of::<usize>()
                + size_of::<Ordering>(),
            $phase
        )?;
        $owner.left_items = &[];
        $owner.right_items = &[];
        $owner.left_cursor.item += shared_length;
        $owner.right_cursor.item += shared_length;
        $owner.ordering = Ordering::Equal;
        continue;
    };
}
