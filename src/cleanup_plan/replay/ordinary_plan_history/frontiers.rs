//! Test-only fixed observation buffer. It cannot inject or authorize a debit.
use std::cell::RefCell;
#[derive(Clone, Copy, Debug)]
pub(super) struct Event {
    pub phase: &'static str,
    pub before: usize,
    pub after: Option<usize>,
    pub swaps: usize,
}
const EMPTY: Event = Event {
    phase: "",
    before: 0,
    after: None,
    swaps: 0,
};
struct Trace {
    enabled: bool,
    count: usize,
    pending: usize,
    swaps: usize,
    events: [Event; 4096],
}
thread_local! {
    static TRACE: RefCell<Trace> = const { RefCell::new(Trace { enabled: false, count: 0, pending: 0, swaps: 0, events: [EMPTY; 4096] }) };
}
fn consumed() -> usize {
    crate::bounded_output::active_limit().unwrap()
        - crate::bounded_output::active_remaining().unwrap()
}
pub(super) fn reset() {
    TRACE.with(|trace| {
        let mut t = trace.borrow_mut();
        t.enabled = true;
        t.count = 0;
        t.pending = 0;
        t.swaps = 0;
    });
}
pub(super) fn before(phase: &'static str) {
    TRACE.with(|trace| {
        let mut t = trace.borrow_mut();
        if !t.enabled {
            return;
        }
        assert!(
            t.count < t.events.len(),
            "finite frontier observation capacity exhausted"
        );
        let index = t.count;
        t.events[index] = Event {
            phase,
            before: consumed(),
            after: None,
            swaps: t.swaps,
        };
        t.pending = index;
        t.count += 1;
    });
}
pub(super) fn after() {
    TRACE.with(|trace| {
        let mut t = trace.borrow_mut();
        if t.enabled {
            let index = t.pending;
            t.events[index].after = Some(consumed());
        }
    });
}
pub(super) fn swapped() {
    TRACE.with(|trace| {
        let mut t = trace.borrow_mut();
        if t.enabled {
            t.swaps += 1;
        }
    });
}
pub(super) fn finish() -> Vec<Event> {
    TRACE.with(|trace| {
        let mut t = trace.borrow_mut();
        t.enabled = false;
        t.events[..t.count].to_vec()
    })
}
