//! Feature-only source-event/request census. No allocator or compiler authority.
use super::{access, lost, observed_active, Budget, State};
use std::rc::Rc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Family {
    Residual,
    Ordinary,
    Selected,
}
pub const LABELS: [&str; 3] = ["residual", "ordinary", "selected"];
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Event {
    Body,
    Empty,
    One,
    Many,
    OwnedBox,
    HeldTransfer,
    Promotion,
    Duplicate,
    TreeInsert,
    InputEmpty,
    InputOne,
    InputMany,
}
pub const EVENTS: [&str; 12] = [
    "body",
    "empty_publication",
    "one_promotion",
    "many_publication",
    "owned_box_formation",
    "held_box_transfer",
    "promotion",
    "equal_duplicate",
    "tree_insert_request",
    "incoming_empty",
    "incoming_one",
    "incoming_many",
];
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Row {
    pub entries: usize,
    pub successes: usize,
    pub errors: usize,
    pub unwinds: usize,
    pub refusals: usize,
    pub requests: usize,
    pub requested_bytes: usize,
    pub admitted_requests: usize,
    pub admitted_bytes: usize,
    pub debits: usize,
    pub debit_bytes: usize,
    pub terminal_paths: usize,
    pub successful_paths: usize,
    pub events: [usize; 12],
    pub foreign_events: usize,
    pub descendant_requests: usize,
    pub descendant_requested_bytes: usize,
    pub descendant_admitted_requests: usize,
    pub descendant_admitted_bytes: usize,
    pub descendant_entries: usize,
    pub descendant_events: [usize; 12],
}
impl Row {
    pub(super) const EMPTY: Self = Self {
        entries: 0,
        successes: 0,
        errors: 0,
        unwinds: 0,
        refusals: 0,
        requests: 0,
        requested_bytes: 0,
        admitted_requests: 0,
        admitted_bytes: 0,
        debits: 0,
        debit_bytes: 0,
        terminal_paths: 0,
        successful_paths: 0,
        events: [0; 12],
        foreign_events: 0,
        descendant_requests: 0,
        descendant_requested_bytes: 0,
        descendant_admitted_requests: 0,
        descendant_admitted_bytes: 0,
        descendant_entries: 0,
        descendant_events: [0; 12],
    };
}
fn add(target: &mut usize, amount: usize) -> bool {
    if let Some(value) = target.checked_add(amount) {
        *target = value;
        true
    } else {
        false
    }
}
// Select the closest still-live attempt whose real ledger is this payer or
// its ancestor. A known descendant without an attempt is never forged as the
// parent's exact payer; its raw requests/events carry explicit subset counts.
fn locate(s: &State, owner: usize) -> Option<(usize, usize, bool)> {
    let ledger = s.ledgers[..s.ledger_depth]
        .iter()
        .position(|l| l.owner == owner)?;
    for depth in (0..s.depth).rev() {
        let index = s.stack[depth];
        if let Some(root) = s.ledgers[..s.ledger_depth]
            .iter()
            .position(|l| l.owner == s.frames[index].owner)
        {
            if root <= ledger {
                return Some((ledger, index, root != ledger));
            }
        }
    }
    None
}
fn update(s: &mut State, index: usize, family: Family, f: impl FnOnce(&mut Row) -> bool) {
    if !f(&mut s.report.attempts[index].path_census[family as usize]) {
        s.report.overflow = true;
        s.report.attempts[index].invalid = true;
        s.invalid();
    }
}
fn foreign(s: &mut State) {
    if s.depth == 0 {
        return;
    }
    let index = s.stack[s.depth - 1];
    update(s, index, Family::Residual, |r| {
        add(&mut r.foreign_events, 1)
    });
    s.report.attempts[index].invalid = true;
    s.invalid();
}
pub(super) fn add_debit(s: &mut State, index: usize, bytes: usize) {
    let owner = s.frames[index].owner;
    let family = s.ledgers[..s.ledger_depth]
        .iter()
        .find(|l| l.owner == owner)
        .and_then(|l| l.path_family)
        .unwrap_or(Family::Residual);
    update(s, index, family, |r| {
        add(&mut r.debits, 1) & add(&mut r.debit_bytes, bytes)
    });
}
pub(crate) fn request(budget: Option<&Budget>, bytes: usize, admitted: bool) {
    let Some(budget) = budget else {
        return;
    };
    let owner = budget as *const Budget as usize;
    access(|s| {
        if !s.enabled || s.depth == 0 {
            return;
        }
        let Some((ledger, index, nested)) = locate(s, owner) else {
            foreign(s);
            return;
        };
        let family = s.ledgers[ledger].path_family.unwrap_or(Family::Residual);
        update(s, index, family, |r| {
            add(&mut r.requests, 1)
                & add(&mut r.requested_bytes, bytes)
                & (!admitted
                    || (add(&mut r.admitted_requests, 1) & add(&mut r.admitted_bytes, bytes)))
                & (!nested
                    || (add(&mut r.descendant_requests, 1)
                        & add(&mut r.descendant_requested_bytes, bytes)
                        & (!admitted
                            || (add(&mut r.descendant_admitted_requests, 1)
                                & add(&mut r.descendant_admitted_bytes, bytes)))))
        });
    });
}
// Borrowed actual payer, not an ACTIVE substitute. Drop records every actual
// optional request, including floor/overflow/sticky failure, before returning.
pub(crate) struct Request<'a> {
    budget: Option<&'a Budget>,
    bytes: usize,
    admitted: bool,
}
impl<'a> Request<'a> {
    pub(crate) fn new(budget: Option<&'a Budget>, bytes: usize) -> Self {
        Self {
            budget,
            bytes,
            admitted: false,
        }
    }
    pub(crate) fn admitted(&mut self) {
        self.admitted = true;
    }
}
impl Drop for Request<'_> {
    fn drop(&mut self) {
        request(self.budget, self.bytes, self.admitted);
    }
}

struct Registration {
    generation: u64,
    ledger: usize,
    serial: usize,
    depth: usize,
    previous: Option<Family>,
    index: usize,
}
pub(crate) struct Guard {
    registered: Option<Registration>,
    success: bool,
}
pub(crate) fn enter(family: Family) -> Guard {
    let owner = observed_active().map(|b| Rc::as_ptr(&b) as usize);
    let registered = access(|s| {
        if !s.enabled || s.depth == 0 {
            return None;
        }
        let Some((ledger, index, nested)) = owner.and_then(|owner| locate(s, owner)) else {
            foreign(s);
            return None;
        };
        let Some(depth) = s.ledgers[ledger].path_depth.checked_add(1) else {
            s.report.overflow = true;
            s.invalid();
            return None;
        };
        let previous = s.ledgers[ledger].path_family.replace(family);
        s.ledgers[ledger].path_depth = depth;
        update(s, index, family, |r| {
            add(&mut r.entries, 1) & (!nested || add(&mut r.descendant_entries, 1))
        });
        Some(Registration {
            generation: s.generation,
            ledger,
            serial: s.ledgers[ledger].serial,
            depth,
            previous,
            index,
        })
    })
    .flatten();
    Guard {
        registered,
        success: false,
    }
}
impl Guard {
    pub(crate) fn success(&mut self) {
        self.success = true;
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        let Some(r) = self.registered.as_ref() else {
            return;
        };
        access(|s| {
            if !s.enabled || s.generation != r.generation {
                lost();
                return;
            }
            if r.ledger >= s.ledger_depth
                || s.ledgers[r.ledger].serial != r.serial
                || s.ledgers[r.ledger].path_depth != r.depth
            {
                s.invalid();
                return;
            }
            let family = s.ledgers[r.ledger].path_family.unwrap_or(Family::Residual);
            // Refusal is telemetry from the same live payer, without a new
            // authorization query. The compiler never consumes this outcome.
            let active = observed_active();
            if active
                .as_ref()
                .is_none_or(|b| Rc::as_ptr(b) as usize != s.ledgers[r.ledger].owner)
            {
                foreign(s);
                return;
            }
            let refused = active.as_ref().is_some_and(|b| b.overflowed.get());
            update(s, r.index, family, |row| {
                add(
                    if std::thread::panicking() {
                        &mut row.unwinds
                    } else if self.success {
                        &mut row.successes
                    } else if refused {
                        &mut row.refusals
                    } else {
                        &mut row.errors
                    },
                    1,
                )
            });
            s.ledgers[r.ledger].path_family = r.previous;
            s.ledgers[r.ledger].path_depth -= 1;
        });
    }
}
pub(crate) fn event(event: Event) {
    record(Some(event), 0, 0);
}
pub(crate) fn finish(terminal: usize, successful: usize) {
    record(None, terminal, successful);
}
fn record(event: Option<Event>, terminal: usize, successful: usize) {
    let owner = observed_active().map(|b| Rc::as_ptr(&b) as usize);
    access(|s| {
        if !s.enabled || s.depth == 0 {
            return;
        }
        let Some((ledger, index, nested)) = owner.and_then(|owner| locate(s, owner)) else {
            foreign(s);
            return;
        };
        let family = s.ledgers[ledger].path_family.unwrap_or(Family::Residual);
        update(s, index, family, |r| match event {
            Some(event) => {
                add(&mut r.events[event as usize], 1)
                    & (!nested || add(&mut r.descendant_events[event as usize], 1))
            }
            None => add(&mut r.terminal_paths, terminal) & add(&mut r.successful_paths, successful),
        });
    });
}
pub(super) fn balanced(rows: &[Row; 3], consumed: usize) -> bool {
    let mut bytes = Some(0usize);
    for r in rows {
        if r.successes
            .checked_add(r.errors)
            .and_then(|n| n.checked_add(r.unwinds))
            .and_then(|n| n.checked_add(r.refusals))
            != Some(r.entries)
            || r.admitted_requests > r.requests
            || r.admitted_bytes > r.requested_bytes
            || r.foreign_events != 0
            || r.descendant_requests > r.requests
            || r.descendant_requested_bytes > r.requested_bytes
            || r.descendant_admitted_requests > r.admitted_requests
            || r.descendant_admitted_bytes > r.admitted_bytes
            || r.descendant_entries > r.entries
            || r.events
                .iter()
                .zip(r.descendant_events)
                .any(|(all, nested)| nested > *all)
        {
            return false;
        }
        bytes = bytes.and_then(|n| n.checked_add(r.debit_bytes));
    }
    bytes == Some(consumed)
}

#[cfg(test)]
mod tests {
    use super::{enter, event, finish, Event, Family};
    use crate::bounded_output::exclusive_debits::{access, attempt};
    use crate::bounded_output::{
        reserve_active_preserving, reserve_active_required, with_limit_usage_refusal,
    };
    use crate::workflow_profile::plan_comparison::{Binding, Capture};
    fn capture() -> Capture {
        Capture::begin(Binding::caller_labelled("path family controls", [0; 32])).unwrap()
    }

    #[test]
    fn exact_requests_debits_prebody_refusal_and_source_events_remain_distinct() {
        let c = capture();
        let (_, refused, used, first) = with_limit_usage_refusal(10, || {
            let mut a = attempt(0, 0, 0);
            assert!(reserve_active_required(1));
            {
                let mut g = enter(Family::Ordinary);
                event(Event::Body);
                assert!(reserve_active_required(2));
                event(Event::OwnedBox);
                event(Event::Empty);
                finish(3, 2);
                g.success();
            }
            {
                let _g = enter(Family::Selected);
                // Actual selected call receiving can refuse before body entry.
                assert!(!reserve_active_preserving(7, 1));
                assert!(!reserve_active_required(8));
                assert!(!reserve_active_required(0));
            }
            a.complete(false);
        });
        let r = c.finish().unwrap().resolved_core;
        assert!(r.complete && !r.invalid && !r.lost && !r.overflow);
        assert!(refused && first.is_some());
        assert_eq!(used, 3);
        let p = r.attempts[0].path_census;
        assert_eq!((p[0].requests, p[0].admitted_bytes), (1, 1));
        assert_eq!(
            (
                p[1].entries,
                p[1].successes,
                p[1].terminal_paths,
                p[1].successful_paths
            ),
            (1, 1, 3, 2)
        );
        assert_eq!(
            (p[1].requests, p[1].requested_bytes, p[1].admitted_bytes),
            (1, 2, 2)
        );
        assert_eq!(
            (
                p[2].entries,
                p[2].refusals,
                p[2].events[Event::Body as usize]
            ),
            (1, 1, 0)
        );
        assert_eq!(
            (p[2].requests, p[2].requested_bytes, p[2].admitted_requests),
            (3, 15, 0)
        );
        assert_eq!(p.iter().map(|row| row.debit_bytes).sum::<usize>(), used);
    }
    #[test]
    fn known_descendant_requests_never_double_count_parent_settlement() {
        let c = capture();
        let (_, refused, used, _) = with_limit_usage_refusal(30, || {
            let mut a = attempt(0, 0, 0);
            let mut outer = enter(Family::Ordinary);
            assert!(reserve_active_required(2));
            with_limit_usage_refusal(10, || {
                let mut child = enter(Family::Selected);
                assert!(reserve_active_required(3));
                event(Event::Body);
                finish(1, 1);
                child.success();
            });
            with_limit_usage_refusal(10, || {
                let mut b = attempt(0, 0, 0);
                let mut child = enter(Family::Selected);
                assert!(reserve_active_required(4));
                child.success();
                drop(child);
                b.complete(true);
            });
            outer.success();
            drop(outer);
            a.complete(true);
        });
        let r = c.finish().unwrap().resolved_core;
        assert!(r.complete && !r.invalid && !refused);
        assert_eq!(used, 9);
        let parent = r.attempts[0].path_census;
        assert_eq!((parent[2].entries, parent[2].descendant_entries), (1, 1));
        assert_eq!(
            (
                parent[2].admitted_bytes,
                parent[2].descendant_admitted_bytes,
                parent[2].debit_bytes
            ),
            (3, 3, 0)
        );
        assert_eq!(parent.iter().map(|row| row.debit_bytes).sum::<usize>(), 9);
        assert_eq!(r.attempts[1].path_census[2].admitted_bytes, 4);
        assert_eq!(r.attempts[1].path_census[2].debit_bytes, 4);
    }
    #[test]
    fn unwind_overflow_and_forgotten_scope_fail_closed_without_affecting_payments() {
        let c = capture();
        let caught = std::panic::catch_unwind(|| {
            with_limit_usage_refusal(3, || {
                let _a = attempt(0, 0, 0);
                let _g = enter(Family::Ordinary);
                assert!(reserve_active_required(1));
                std::panic::panic_any(919usize);
            })
        });
        assert!(caught.is_err());
        drop(caught);
        let r = c.finish().unwrap().resolved_core;
        assert!(r.complete);
        assert_eq!(r.attempts[0].path_census[1].unwinds, 1);
        let c = capture();
        with_limit_usage_refusal(3, || {
            let mut a = attempt(0, 0, 0);
            let mut g = enter(Family::Ordinary);
            access(|s| {
                s.report.attempts[0].path_census[1].events[Event::Body as usize] = usize::MAX
            });
            event(Event::Body);
            assert!(reserve_active_required(1));
            g.success();
            drop(g);
            a.complete(true);
        });
        let r = c.finish().unwrap().resolved_core;
        assert!(!r.complete && r.invalid && r.overflow);
        let c = capture();
        with_limit_usage_refusal(3, || {
            let mut a = attempt(0, 0, 0);
            std::mem::forget(enter(Family::Selected));
            assert!(reserve_active_required(1));
            a.complete(true);
        });
        let r = c.finish().unwrap().resolved_core;
        assert!(!r.complete && r.invalid);
    }
}

#[cfg(test)]
mod owner_tests {
    use super::{enter, request, Family};
    use crate::bounded_output::exclusive_debits::{access, attempt};
    use crate::bounded_output::{reserve_active_required, with_limit_usage_refusal, ACTIVE};
    use crate::workflow_profile::plan_comparison::{Binding, Capture};
    #[test]
    fn stale_child_and_replaced_ledger_serial_invalidate_only_telemetry() {
        let c =
            Capture::begin(Binding::caller_labelled("census foreign controls", [0; 32])).unwrap();
        let (_, refused, paid, _) = with_limit_usage_refusal(5, || {
            let mut a = attempt(0, 0, 0);
            let (stale, _, _, _) = with_limit_usage_refusal(2, || {
                ACTIVE.with(|cell| cell.borrow().as_ref().unwrap().clone())
            });
            request(Some(&stale), 1, false);
            assert!(reserve_active_required(1));
            a.complete(true);
        });
        assert!(!refused);
        assert_eq!(paid, 1);
        let r = c.finish().unwrap().resolved_core;
        assert!(!r.complete && r.invalid);
        assert_eq!(r.attempts[0].path_census[0].foreign_events, 1);
        let c =
            Capture::begin(Binding::caller_labelled("census serial controls", [0; 32])).unwrap();
        let (_, refused, paid, _) = with_limit_usage_refusal(5, || {
            let mut a = attempt(0, 0, 0);
            let g = enter(Family::Ordinary);
            access(|s| s.ledgers[0].serial += 1);
            drop(g);
            assert!(reserve_active_required(1));
            a.complete(true);
        });
        assert!(!refused);
        assert_eq!(paid, 1);
        let r = c.finish().unwrap().resolved_core;
        assert!(!r.complete && r.invalid);
    }
}
