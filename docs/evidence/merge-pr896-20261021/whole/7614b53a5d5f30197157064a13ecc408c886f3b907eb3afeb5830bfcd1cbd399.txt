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
pub(super) const STEP: usize = 2 * size_of::<&Vec<usize>>() + size_of::<usize>() + size_of::<bool>() // unchanged len/condition
    + 2 * size_of::<bool>() // loaded condition and negation
    + size_of::<&Rc<SkeletonObservation>>() // head pattern
    + 2 * size_of::<&[Rc<SkeletonObservation>]>() // rest pattern and assignment copy
    + 2 * size_of::<&Rc<SkeletonObservation>>() + size_of::<&SkeletonObservation>() // as_ref actual/formal/return
    + 2 * size_of::<Option<&SkeletonObservation>>() // Some initializer and caller-received output
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
    + size_of::<&SkeletonObservation>()
    + 2 * size_of::<Option<&SkeletonObservation>>();
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
        > OWNER_CONTROLS + 2 * LOAD + 2 * RESET + 4 * (STEP + EMPTY) + 2 * TAIL
);

macro_rules! lexical_plan_next {
    ($owner:ident, $cursor:ident, $items:ident, $loaded:ident,
     $history:expr, $function:ident, $budget:ident, $phase:literal) => {{
        $budget.charge($function, 1, "ordinary plan history forward cursor step")?;
        'plan_yield: {
            loop {
                paid!(
                    $budget,
                    $function,
                    lexical_cursor::STEP + lexical_cursor::EMPTY,
                    $phase
                )?;
                if $owner.$cursor.segment >= $owner.$cursor.links.len() {
                    break;
                }
                $budget.charge(
                    $function,
                    1,
                    "ordinary plan history forward segment traversal",
                )?;
                if !$owner.$loaded {
                    paid!($budget, $function, lexical_cursor::LOAD, $phase)?;
                    let links = $owner.$cursor.links.as_slice();
                    let link = links[links.len() - $owner.$cursor.segment - 1];
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
            let history = &$history;
            let value = history.tail.get($owner.$cursor.item);
            if value.is_some() {
                $owner.$cursor.item += 1;
            }
            match value {
                Some(value) => Some::<&SkeletonObservation>(value.as_ref()),
                None => None,
            }
        }
    }};
}
