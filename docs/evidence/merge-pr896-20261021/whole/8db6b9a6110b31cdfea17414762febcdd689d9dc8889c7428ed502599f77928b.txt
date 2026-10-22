//! Arena-backed immutable slices survive calls; no path-tail borrow survives a match.
use super::*;

// Exactly the accepted880 narrow primitive criterion: same function/budget,
// sequential nonescaping nonrecursive ZST-success/one-terminal-Diagnostic slots.
// Four expansions each have forward+segment work and STEP+LOAD+TAIL payment.
// Every payload below is separately paid on actual execution, never once.
pub(super) const PRIMITIVE_FRAMES: usize = 20 * PAYMENT_HEADER;
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
    + size_of::<bool>(); // loaded=true
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
        > OWNER_CONTROLS + 2 * LOAD + 2 * RESET + 4 * (STEP + EMPTY) + 2 * TAIL + 4 * TAIL_VIEW
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
