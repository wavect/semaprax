//! One lexical Exit admission; original shared query/API/tariffs stay intact.
use super::*;
use std::cmp::Ordering;
use std::mem::size_of;

pub(super) struct OneQuery<'a> {
    pub(super) accepted: u64,
    pub(super) label: &'a Label,
    pub(super) child: &'a usize,
}
const _: () =
    assert!(size_of::<OneQuery<'static>>() == size_of::<intern_arena::SignatureQuery<'static>>());
const _: () =
    assert!(size_of::<&OneQuery<'static>>() == size_of::<&intern_arena::SignatureQuery<'static>>());
// Same Query::compare ABI as the complete old receiver; no constructor helper,
// forwarding Self/Diagnostic return, payload copy or additional lookup exists.
impl intern_arena::Query<intern_arena::Signature> for OneQuery<'_> {
    fn compare(&self, stored: &intern_arena::Signature) -> Ordering {
        let order = self.accepted.cmp(&stored.0);
        if order != Ordering::Equal {
            return order;
        }
        if stored.1.is_empty() {
            return Ordering::Greater;
        }
        let order = self.label.cmp(&stored.1[0].0);
        if order != Ordering::Equal {
            return order;
        }
        let order = self.child.cmp(&stored.1[0].1);
        if order != Ordering::Equal {
            return order;
        }
        1_usize.cmp(&stored.1.len())
    }
}
// One uninitialized retained usize receiving place; one conservative
// constant-pattern usize place and two overwritten branch predicate slots;
// both original warm-flag bool slots. No Vec/Self/Option/query is retained.
pub(super) const LOCAL_CONTROLS: usize = 2 * size_of::<usize>() + 4 * size_of::<bool>();
// Exactly the full conservative primitive ABI: actual/formal arguments and
// returned/received unit-success Result slots. Only this nonrecursive lexical
// primitive place is retained; no compound query/Option/key/iterator is reused.
pub(super) const PRIMITIVE: usize =
    2 * size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>()
        + 2 * size_of::<Result<(), Diagnostic>>();
const QUERY: usize =
    2 * size_of::<OneQuery<'static>>() + 2 * size_of::<&Label>() + 2 * size_of::<&usize>();
// Complete old successor iterator/borrow bank remains fresh. Its first next
// call replaces the original loop's first next with identical Option/item ABI.
const ITERATOR: usize = 2 * size_of::<std::slice::Iter<'static, (Label, SharedWeighted)>>()
    + 2 * size_of::<&Vec<(Label, SharedWeighted)>>()
    + size_of::<&[(Label, SharedWeighted)]>();
// Vec::remove has both mutable receiver/index arguments, returned/received
// owned row, bounds length/predicate and the copied scalar child key. Its
// inner row is dropped before signature insertion, as in the original loop.
const MOVE: usize = 2 * size_of::<(Label, SharedWeighted)>()
    + 2 * size_of::<&mut Vec<(Label, SharedWeighted)>>()
    + 4 * size_of::<usize>()
    + 2 * size_of::<bool>()
    + 2 * size_of::<usize>();
const SELECT: usize = 4 * size_of::<bool>() + 2 * size_of::<usize>();
// Only the one-edge route constructs these fresh receivers. Its duplicate
// match-len call is gone: the work frontier already received the same length
// into the prepaid scalar place. Empty/general keep all old tariffs.
pub(super) const FRESH: usize = QUERY + ITERATOR + MOVE;
pub(super) const WARM: usize = FRESH + SELECT;
pub(super) const COLD: usize = FRESH + SELECT + PRIMITIVE;
#[cfg(test)]
mod tests;

#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub(super) struct Event {
    pub(super) cold: bool,
    pub(super) requested: usize,
    pub(super) paid: usize,
    pub(super) remaining: usize,
    pub(super) skeleton_remaining: usize,
    pub(super) skeleton_spent: usize,
}
#[cfg(test)]
thread_local! {
    static EVENTS: std::cell::Cell<(bool, [Option<Event>; 16], usize)> = const {
        std::cell::Cell::new((false, [None; 16], 0))
    };
}
#[cfg(test)]
pub(super) fn note(budget: &ReplayBudget, warm: bool) {
    EVENTS.with(|slot| {
        let (active, mut events, count) = slot.get();
        if !active {
            return;
        }
        if count < events.len() {
            events[count] = Some(Event {
                cold: !warm,
                requested: if warm { WARM } else { COLD },
                paid: budget.physical.as_ref().unwrap().charged(),
                remaining: budget.remaining,
                skeleton_remaining: budget.skeleton_remaining,
                skeleton_spent: budget.skeleton_spent,
            });
        }
        slot.set((active, events, count.saturating_add(1)));
    });
}

// Test-only exact role equations; no runtime fee helper or external authority.
#[cfg(test)]
impl Canonicalizer<'_> {
    pub(in crate::cleanup_plan::replay::factored::canonical_shared) const BORROWED_EXIT_PRIMITIVE: usize = PRIMITIVE;
    pub(in crate::cleanup_plan::replay::factored::canonical_shared) const BORROWED_EXIT_WARM:
        usize = WARM;
    pub(in crate::cleanup_plan::replay::factored::canonical_shared) const BORROWED_EXIT_LOCAL:
        usize = LOCAL_CONTROLS;
}
