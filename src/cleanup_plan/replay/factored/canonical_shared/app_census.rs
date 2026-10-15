//! Test-only cumulative events in the unchanged original application owner.
//! No unique-node frequency, runtime permission, scan, allocation or debit.
use std::cell::Cell;

#[derive(Clone, Copy)]
struct Stats {
    eligible: usize,
    ready_entries: usize,
    acquired: usize,
    published: usize,
    overflowed: bool,
}
#[derive(Clone, Copy)]
struct State {
    active: bool,
    stats: Stats,
}
impl State {
    const EMPTY: Self = Self {
        active: false,
        stats: Stats {
            eligible: 0,
            ready_entries: 0,
            acquired: 0,
            published: 0,
            overflowed: false,
        },
    };
}
thread_local! {
    static CURRENT: Cell<State> = const { Cell::new(State::EMPTY) };
}
fn add(slot: &mut usize, overflowed: &mut bool, amount: usize) {
    match slot.checked_add(amount) {
        Some(value) => *slot = value,
        None => {
            *slot = usize::MAX;
            *overflowed = true;
        }
    }
}
fn update(operation: impl FnOnce(&mut Stats)) {
    CURRENT.with(|current| {
        let mut state = current.get();
        if state.active {
            operation(&mut state.stats);
            current.set(state);
        }
    });
}
pub(super) fn eligible() {
    update(|s| add(&mut s.eligible, &mut s.overflowed, 1));
}
pub(super) fn child_ready(entries: usize) {
    update(|s| add(&mut s.ready_entries, &mut s.overflowed, entries));
}
pub(super) fn acquired() {
    update(|s| add(&mut s.acquired, &mut s.overflowed, 1));
}
pub(super) fn published() {
    update(|s| add(&mut s.published, &mut s.overflowed, 1));
}

// Constructed before the actual owner prepares any inputs or starts an observer.
// Restored/printed only after its complete body (or unwinding teardown) finishes.
pub(crate) struct Scope(State);
impl Scope {
    pub(crate) fn normalizer() -> Self {
        let previous = CURRENT.with(|current| {
            let mut next = State::EMPTY;
            next.active = true;
            current.replace(next)
        });
        Self(previous)
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        let stats = CURRENT.with(|current| current.replace(self.0).stats);
        let partial_prefix = std::thread::panicking();
        eprintln!(
            "epsilon-app-census owner=catalog-normalizer-compact-original partial_prefix={} eligible_events={} ready_child_entry_events={} shared_acquired_events={} memo_publication_events={} counters_overflowed={} cumulative_events_not_unique_frequency=true",
            partial_prefix,
            stats.eligible,
            stats.ready_entries,
            stats.acquired,
            stats.published,
            stats.overflowed
        );
    }
}
