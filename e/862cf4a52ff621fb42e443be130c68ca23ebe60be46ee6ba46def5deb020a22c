//! Test-only fixed observation buffer. It cannot inject or authorize a debit.
use std::cell::RefCell;
use std::sync::{Mutex, MutexGuard};
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
    pending: Option<usize>,
    swaps_only: bool,
    swaps: usize,
    events: [Event; 4096],
}
// The fixed test buffer has no heap backing and occupies no thread stack/TLS
// payload. Explicit observations serialize; inactive compiler callbacks never
// acquire this mutex. A thread retains its lease so repeated finish calls keep
// their original complete snapshot. TLS destruction releases the lease.
static STORAGE: Mutex<Trace> = Mutex::new(Trace {
    enabled: false,
    count: 0,
    pending: None,
    swaps_only: false,
    swaps: 0,
    events: [EMPTY; 4096],
});
thread_local! {
    static TRACE: RefCell<Option<MutexGuard<'static, Trace>>> = const { RefCell::new(None) };
}
// Warm only the allocation-free thread-bound lease carrier. The entire fixed
// backing already exists; neither preparation nor first reset allocates.
pub(super) fn prepare() {
    TRACE.with(|_| {});
}
fn consumed() -> usize {
    crate::bounded_output::active_limit().unwrap()
        - crate::bounded_output::active_remaining().unwrap()
}
pub(super) fn reset() {
    reset_mode(false);
}
// The cold/hot invoice corpus needs complete swap order, not every debit.
// Keep the same fixed buffer and fail closed if the selected stream fills it.
pub(super) fn reset_swaps() {
    reset_mode(true);
}
fn reset_mode(swaps_only: bool) {
    TRACE.with(|trace| {
        let mut lease = trace.borrow_mut();
        if lease.is_none() {
            // A prior observer panic cannot authorize stale events: reset below
            // overwrites every live control before enabling a new observation.
            *lease = Some(
                STORAGE
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
            );
        }
        let t = lease.as_deref_mut().unwrap();
        assert_eq!(
            t.events.len(),
            4096,
            "frontier instrumentation must be prepared"
        );
        t.enabled = true;
        t.count = 0;
        t.pending = None;
        t.swaps_only = swaps_only;
        t.swaps = 0;
    });
}
pub(super) fn before(phase: &'static str) {
    TRACE.with(|trace| {
        let mut lease = trace.borrow_mut();
        let Some(t) = lease.as_deref_mut() else {
            return;
        };
        if !t.enabled {
            return;
        }
        t.pending = None;
        if t.swaps_only
            && phase != "ordinary plan retained drain swap receivers"
            && phase != "ordinary plan retained sift swap receivers"
        {
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
        t.pending = Some(index);
        t.count += 1;
    });
}
pub(super) fn after() {
    TRACE.with(|trace| {
        let mut lease = trace.borrow_mut();
        let Some(t) = lease.as_deref_mut() else {
            return;
        };
        if t.enabled {
            if let Some(index) = t.pending {
                t.events[index].after = Some(consumed());
            }
        }
    });
}
pub(super) fn swapped() {
    TRACE.with(|trace| {
        let mut lease = trace.borrow_mut();
        let Some(t) = lease.as_deref_mut() else {
            return;
        };
        if t.enabled {
            t.swaps += 1;
        }
    });
}
pub(super) fn finish() -> Vec<Event> {
    TRACE.with(|trace| {
        let mut lease = trace.borrow_mut();
        let Some(t) = lease.as_deref_mut() else {
            return Vec::new();
        };
        t.enabled = false;
        t.events[..t.count].to_vec()
    })
}

#[cfg(test)]
mod initialization_tests;
