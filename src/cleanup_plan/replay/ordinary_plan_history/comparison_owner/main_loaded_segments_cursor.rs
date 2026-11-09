//! Arena-backed immutable slices survive calls; no path-tail borrow survives a match.
use super::*;

// Exactly the accepted880 narrow primitive criterion: same function/budget,
// sequential nonescaping nonrecursive ZST-success/one-terminal-Diagnostic slots.
// Two final-HIR readers and three paired branches each retain every work site.
// The paired branches transport plain immutable Rc loans without Option outputs.
// The shared immutable-step branch adds five payment and five work sites.
// Every payload below is separately paid on actual execution, never once.
// Paired-tail selection adds one payment site; its bulk loop adds one
// payload payment and three work sites, each with a complete primitive frame.
// The fused fallback replaces two readers with left/right-some/right-none readers:
// five extra primitive sites, separately funded even for mutually exclusive arms.
pub(super) const PRIMITIVE_FRAMES: usize = 59 * PAYMENT_HEADER;
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

// A fixed run-local Some(())/terminal-None caller frame. Both variants carry
// no owned or borrowed data. This is separate from the accepted Result rule.
pub(super) type SharedUnitDriver = std::iter::Take<std::iter::Repeat<()>>;
pub(super) const RUN_SETUP: usize = 2 * size_of::<&[Rc<SkeletonObservation>]>() + 2 * size_of::<usize>() // len actual/formal/result/receiver
    + 2 * size_of::<usize>() + 6 * size_of::<SharedUnitDriver>() // take constructor and IntoIterator transfers
    + 2 * size_of::<&mut SharedUnitDriver>() + 2 * size_of::<Option<()>>(); // one retained next frame
const _: () = assert!(size_of::<()>() == 0 && !std::mem::needs_drop::<SharedUnitDriver>());
pub(super) const PAIR_LINK_VIEW: usize = 2 * size_of::<&Vec<usize>>() + 2 * size_of::<&[usize]>();
pub(super) const PAIR_LOAD: usize = 2 * size_of::<&[usize]>() + 2 * size_of::<usize>() + size_of::<bool>() // suffix pattern, assignment, copied/remembered link
    + 2 * size_of::<&[Segment]>() + 2 * size_of::<usize>() + size_of::<&Segment>()
    + 2 * size_of::<&Vec<Rc<SkeletonObservation>>>() + 2 * size_of::<&[Rc<SkeletonObservation>]>()
    + size_of::<bool>(); // loaded=true

pub(super) const PAIR_LEFT_STEP: usize = 2 * size_of::<usize>()
    + 3 * size_of::<bool>()
    + size_of::<&Rc<SkeletonObservation>>()
    + 2 * size_of::<&[Rc<SkeletonObservation>]>()
    + 2 * size_of::<&Rc<SkeletonObservation>>()
    + size_of::<usize>()
    + EMPTY;
pub(super) const PAIR_RIGHT_STEP: usize = 2 * size_of::<usize>()
    + 3 * size_of::<bool>()
    + size_of::<&Rc<SkeletonObservation>>()
    + 2 * size_of::<&[Rc<SkeletonObservation>]>()
    + size_of::<usize>()
    + EMPTY;
pub(super) const PAIR_LEFT_TAIL: usize = size_of::<&Rc<SkeletonObservation>>()
    + 2 * size_of::<&[Rc<SkeletonObservation>]>()
    + 2 * size_of::<&Rc<SkeletonObservation>>()
    + size_of::<usize>()
    + 3 * size_of::<u8>();
pub(super) const PAIR_RIGHT_TAIL: usize = size_of::<&Rc<SkeletonObservation>>()
    + 2 * size_of::<&[Rc<SkeletonObservation>]>()
    + size_of::<usize>()
    + 3 * size_of::<u8>();
pub(super) const PAIR_CONTENT: usize = 4 * size_of::<&Rc<SkeletonObservation>>()
    + 2 * size_of::<&SkeletonObservation>()
    + 4 * size_of::<&SkeletonObservation>()
    + 4 * size_of::<&SkeletonTerminal>()
    + 2 * size_of::<Ordering>();
const _: () =
    assert!(PAIR_LEFT_STEP + PAIR_RIGHT_STEP + PAIR_CONTENT < 2 * (STEP + EMPTY) + PAIR_MATCH);

macro_rules! lexical_plan_next {
    ($budget:ident, $function:ident, $census:ident, $phase:literal) => {
        paid!($budget, $function, lexical_cursor::PAIR_CONTENT, $phase)?;
        #[cfg(test)]
        $census.pair_receivers();
    };
    ($owner:ident, $left_path:ident, $right_path:ident, $left_tail:ident, $right_tail:ident,
     $left_links:ident, $right_links:ident, $census:ident, $function:ident, $budget:ident,
     $pair_loop:lifetime, $phase:literal) => {
        paired_plan_observations!(
            $owner,
            $left_path,
            $right_path,
            $left_tail,
            $right_tail,
            $left_links,
            $right_links,
            $census,
            $function,
            $budget,
            $pair_loop,
            $phase
        );
    };
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

// Five payment and five work sites have distinct complete primitive frames in
// the multipath bank. Every actual selector, slice receiver and scalar copy
// remains separately paid before formation. No observation Option/tuple/cmp
// payload is formed on this branch. Original left-before-right work is exact.
macro_rules! shared_plan_pair_step {
    ($owner:ident, $left_path:ident, $right_path:ident, $left_tail:ident, $right_tail:ident, $left_links:ident, $right_links:ident, $census:ident, $function:ident, $budget:ident, $pair_loop:lifetime, $phase:literal) => {
        paid!($budget, $function, 3 * size_of::<bool>(), $phase)?;
        #[cfg(test)]
        $census.mask_checks();
        if $owner.tail_mask == 3 {
            paired_plan_tails!(
                $owner,
                $left_path,
                $right_path,
                $left_tail,
                $right_tail,
                $census,
                $function,
                $budget,
                $phase
            );
        }
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
                independent_plan_segment_run!(
                    $owner, $census, $function, $budget, $pair_loop, $phase
                );
            } else {
                shared_plan_boundary!(
                    $owner,
                    $left_links,
                    $right_links,
                    $census,
                    $function,
                    $budget,
                    $phase
                );
            }
        }
    };
}

// Compare already-loaded immutable segments without rebuilding the normal
// cursor transport or sharing decisions for each interior observation. Every
// original semantic work request remains in its original order.
pub(super) const INDEPENDENT_RUN_SETUP: usize =
    RUN_SETUP + 2 * size_of::<&[Rc<SkeletonObservation>]>() + size_of::<[usize; 8]>();
pub(super) const INDEPENDENT_RUN_PAYLOAD: usize = 2 * size_of::<&Rc<SkeletonObservation>>()
    + 4 * size_of::<&[Rc<SkeletonObservation>]>()
    + 4 * size_of::<&Rc<SkeletonObservation>>()
    + 2 * size_of::<&SkeletonObservation>()
    + 4 * size_of::<&SkeletonObservation>()
    + 2 * size_of::<usize>()
    + 4 * size_of::<Ordering>()
    + 3 * size_of::<bool>();
macro_rules! independent_plan_segment_run {
    ($owner:ident, $census:ident, $function:ident, $budget:ident, $pair_loop:lifetime, $phase:literal) => {
        paid!($budget, $function, 3 * size_of::<bool>(), $phase)?;
        #[cfg(test)]
        $census.independent_checks();
        if matches!($owner.left_items, [_, _, _, _, ..])
            && matches!($owner.right_items, [_, _, _, _, ..])
        {
            paid!(
                $budget,
                $function,
                lexical_cursor::INDEPENDENT_RUN_SETUP,
                $phase
            )?;
            #[cfg(test)]
            $census.independent_runs();
            let paired_length = $owner.left_items.len().min($owner.right_items.len());
            let paired_work_driver: lexical_cursor::SharedUnitDriver =
                std::iter::repeat(()).take(paired_length);
            for () in paired_work_driver {
                paid!(
                    $budget,
                    $function,
                    lexical_cursor::INDEPENDENT_RUN_PAYLOAD,
                    $phase
                )?;
                #[cfg(test)]
                $census.independent_turns();
                $budget.charge($function, 1, "ordinary plan history forward cursor step")?;
                #[cfg(test)]
                $census.step();
                $budget.charge(
                    $function,
                    1,
                    "ordinary plan history forward segment traversal",
                )?;
                let [left, left_rest @ ..] = $owner.left_items else {
                    unreachable!("immutable prepared left segment length")
                };
                $owner.left_items = left_rest;
                $owner.left_cursor.item += 1;
                $budget.charge($function, 1, "ordinary plan history forward cursor step")?;
                #[cfg(test)]
                $census.step();
                $budget.charge(
                    $function,
                    1,
                    "ordinary plan history forward segment traversal",
                )?;
                let [right, right_rest @ ..] = $owner.right_items else {
                    unreachable!("immutable prepared right segment length")
                };
                $owner.right_items = right_rest;
                $owner.right_cursor.item += 1;
                $owner.ordering = left.as_ref().cmp(right.as_ref());
                if $owner.ordering != Ordering::Equal {
                    break $pair_loop;
                }
                $budget.charge($function, 1, "ordinary plan history observation comparison")?;
                #[cfg(test)]
                $census.round();
            }
            continue;
        }
    };
}

// A loaded empty slice and a remaining validated link describe exactly one
// exhausted-to-next-segment transition. Inspect only that next immutable
// segment; empty or divergent successors use the complete original traversal.
// Four payment and seven work sites have distinct full multipath headers.
macro_rules! shared_plan_boundary {
    ($owner:ident, $left_links:ident, $right_links:ident, $census:ident, $function:ident, $budget:ident, $phase:literal) => {
        paid!($budget, $function, size_of::<bool>(), $phase)?;
        #[cfg(test)]
        $census.shared_empty_checks();
        if let [] = $owner.right_items {
            paid!(
                $budget,
                $function,
                2 * size_of::<&[usize]>() + 4 * size_of::<usize>() + 3 * size_of::<bool>(),
                $phase
            )?;
            #[cfg(test)]
            $census.shared_links();

            if let &[ref left_links_rest @ .., left_next] = $left_links {
                if let &[ref right_links_rest @ .., right_next] = $right_links {
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
                                2 * size_of::<&[usize]>()
                                    + 2 * size_of::<&[Rc<SkeletonObservation>]>()
                                    + (4 * size_of::<usize>() + 4 * size_of::<usize>())
                                    + 4 * size_of::<bool>()
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
                            $left_links = left_links_rest;
                            $owner.left_link = left_next;
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
                            $right_links = right_links_rest;
                            $owner.right_link = right_next;
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
        paid!($budget, $function, lexical_cursor::RUN_SETUP, $phase)?;
        #[cfg(test)]
        $census.shared_runs();
        let shared_length = $owner.left_items.len();
        let shared_work_driver: lexical_cursor::SharedUnitDriver =
            std::iter::repeat(()).take(shared_length);
        for () in shared_work_driver {
            #[cfg(test)]
            $census.shared_turns();
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
        }
        #[cfg(test)]
        $census.shared_turns(); // sole terminal None
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

// Only non-owning tails, after both original prepared chains finish.
macro_rules! paired_plan_tails {
    ($owner:ident, $left_path:ident, $right_path:ident, $left_tail:ident, $right_tail:ident, $census:ident, $function:ident, $budget:ident, $phase:literal) => {
        loop {
            // All pattern, slice, comparison and scalar payloads, including
            // mutually exclusive terminal cases, are paid on each turn.
            paid!(
                $budget,
                $function,
                2 * size_of::<&Rc<SkeletonObservation>>()
                    + 4 * size_of::<&[Rc<SkeletonObservation>]>()
                    + 4 * size_of::<&Rc<SkeletonObservation>>()
                    + 2 * size_of::<&SkeletonObservation>()
                    + 4 * size_of::<&SkeletonObservation>()
                    + 4 * size_of::<&SkeletonTerminal>()
                    + 2 * size_of::<usize>()
                    + 4 * size_of::<Ordering>()
                    + 3 * size_of::<bool>(),
                $phase
            )?;
            #[cfg(test)]
            $census.fused_turns();
            $budget.charge($function, 1, "ordinary plan history forward cursor step")?;
            #[cfg(test)]
            $census.step();
            $budget.charge($function, 1, "ordinary plan history forward cursor step")?;
            #[cfg(test)]
            $census.step();
            #[cfg(test)]
            {
                $census.tail();
                $census.tail();
            }
            match $left_tail {
                [left, left_rest @ ..] => {
                    $left_tail = left_rest;
                    $owner.left_cursor.item += 1;
                    match $right_tail {
                        [right, right_rest @ ..] => {
                            $right_tail = right_rest;
                            $owner.right_cursor.item += 1;
                            $owner.ordering = left.as_ref().cmp(right.as_ref());
                            if $owner.ordering == Ordering::Equal {
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
                        [] => $owner.ordering = Ordering::Greater,
                    }
                }
                [] => match $right_tail {
                    [_, right_rest @ ..] => {
                        $right_tail = right_rest;
                        $owner.right_cursor.item += 1;
                        $owner.ordering = Ordering::Less;
                    }
                    [] => $owner.ordering = $left_path.terminal.cmp(&$right_path.terminal),
                },
            }
            break;
        }
        break;
    };
}

macro_rules! remember_plan_tail {
    ($owner:ident, left_cursor) => {
        $owner.tail_mask |= 1;
    };
    ($owner:ident, right_cursor) => {
        $owner.tail_mask |= 2;
    };
}

macro_rules! paired_plan_observations {
 ($owner:ident, $left_path:ident, $right_path:ident, $left_tail:ident, $right_tail:ident,
  $left_links:ident, $right_links:ident, $census:ident, $function:ident, $budget:ident,
  $pair_loop:lifetime, $phase:literal) => {
  $budget.charge($function, 1, "ordinary plan history forward cursor step")?;
  let left_head = 'left_loan: {
   loop {
    paid!($budget, $function, lexical_cursor::PAIR_LEFT_STEP, $phase)?;
  #[cfg(test)]
  $census.pair_left_steps();
    #[cfg(test)]
    $census.step();
    if $owner.left_cursor.segment >= $owner.left_segments { break; }
    $budget.charge($function, 1, "ordinary plan history forward segment traversal")?;
    if !$owner.left_loaded {
     paid!($budget, $function, lexical_cursor::PAIR_LOAD, $phase)?;
     #[cfg(test)]
     $census.load();
     let &[ref links_rest @ .., link] = $left_links else { unreachable!("prepared ordinary pair-local links"); };
     $left_links = links_rest;
     remember_plan_segment!($owner, left_cursor, link);
     $owner.left_items = $owner.arena[link].items.as_slice();
     $owner.left_loaded = true;
    }
    match $owner.left_items {
     [head, rest @ ..] => { $owner.left_items = rest; $owner.left_cursor.item += 1; break 'left_loan head; }
     [] => { $owner.left_cursor.segment += 1; $owner.left_cursor.item = 0; $owner.left_loaded = false; }
    }
   }
   paid!($budget, $function, lexical_cursor::PAIR_LEFT_TAIL, $phase)?;
  #[cfg(test)]
  $census.pair_left_tails();
   #[cfg(test)]
   { $census.tail_marks(); $census.tail(); }
   remember_plan_tail!($owner, left_cursor);
   match $left_tail {
    [head, rest @ ..] => { $left_tail = rest; $owner.left_cursor.item += 1; break 'left_loan head; }
    [] =>
{
 $budget.charge($function, 1, "ordinary plan history forward cursor step")?;
 loop {
  paid!($budget, $function, lexical_cursor::PAIR_RIGHT_STEP, $phase)?;
  #[cfg(test)]
  $census.pair_right_steps();
  #[cfg(test)]
  $census.step();
  if $owner.right_cursor.segment >= $owner.right_segments { break; }
  $budget.charge($function, 1, "ordinary plan history forward segment traversal")?;
  if !$owner.right_loaded {
   paid!($budget, $function, lexical_cursor::PAIR_LOAD, $phase)?;
   #[cfg(test)]
   $census.load();
   let &[ref links_rest @ .., link] = $right_links else { unreachable!("prepared ordinary pair-local links"); };
   $right_links = links_rest;
   remember_plan_segment!($owner, right_cursor, link);
   $owner.right_items = $owner.arena[link].items.as_slice();
   $owner.right_loaded = true;
  }
  match $owner.right_items {
   [_, rest @ ..] => {
    $owner.right_items = rest;
    $owner.right_cursor.item += 1;
    $owner.ordering = Ordering::Less;
break $pair_loop;
   }
   [] => { $owner.right_cursor.segment += 1; $owner.right_cursor.item = 0; $owner.right_loaded = false; }
  }
 }
 paid!($budget, $function, lexical_cursor::PAIR_RIGHT_TAIL, $phase)?;
  #[cfg(test)]
  $census.pair_right_tails();
 #[cfg(test)]
 { $census.tail_marks(); $census.tail(); }
 remember_plan_tail!($owner, right_cursor);
 match $right_tail {
  [_, rest @ ..] => { $right_tail = rest; $owner.right_cursor.item += 1; $owner.ordering = Ordering::Less;
break $pair_loop; }
  [] => { $owner.ordering = $left_path.terminal.cmp(&$right_path.terminal);
break $pair_loop; }
 }
}

   }
  };

{
 $budget.charge($function, 1, "ordinary plan history forward cursor step")?;
 loop {
  paid!($budget, $function, lexical_cursor::PAIR_RIGHT_STEP, $phase)?;
  #[cfg(test)]
  $census.pair_right_steps();
  #[cfg(test)]
  $census.step();
  if $owner.right_cursor.segment >= $owner.right_segments { break; }
  $budget.charge($function, 1, "ordinary plan history forward segment traversal")?;
  if !$owner.right_loaded {
   paid!($budget, $function, lexical_cursor::PAIR_LOAD, $phase)?;
   #[cfg(test)]
   $census.load();
   let &[ref links_rest @ .., link] = $right_links else { unreachable!("prepared ordinary pair-local links"); };
   $right_links = links_rest;
   remember_plan_segment!($owner, right_cursor, link);
   $owner.right_items = $owner.arena[link].items.as_slice();
   $owner.right_loaded = true;
  }
  match $owner.right_items {
   [right_head, rest @ ..] => {
    $owner.right_items = rest;
    $owner.right_cursor.item += 1;
    $owner.ordering = left_head.as_ref().cmp(right_head.as_ref());
if $owner.ordering == Ordering::Equal {
    $budget.charge($function, 1, "ordinary plan history observation comparison")?;
    #[cfg(test)]
    $census.round();
    continue $pair_loop;
}
break $pair_loop;
   }
   [] => { $owner.right_cursor.segment += 1; $owner.right_cursor.item = 0; $owner.right_loaded = false; }
  }
 }
 paid!($budget, $function, lexical_cursor::PAIR_RIGHT_TAIL, $phase)?;
  #[cfg(test)]
  $census.pair_right_tails();
 #[cfg(test)]
 { $census.tail_marks(); $census.tail(); }
 remember_plan_tail!($owner, right_cursor);
 match $right_tail {
  [right_head, rest @ ..] => { $right_tail = rest; $owner.right_cursor.item += 1; $owner.ordering = left_head.as_ref().cmp(right_head.as_ref());
if $owner.ordering == Ordering::Equal {
    $budget.charge($function, 1, "ordinary plan history observation comparison")?;
    #[cfg(test)]
    $census.round();
    continue $pair_loop;
}
break $pair_loop; }
  [] => { $owner.ordering = Ordering::Greater;
break $pair_loop; }
 }
}

 };
}
