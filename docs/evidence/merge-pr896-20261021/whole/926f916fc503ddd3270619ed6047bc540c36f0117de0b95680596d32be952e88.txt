// Shared real System-backed observation; compiled only by test harnesses.
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::{Cell, RefCell};
#[path = "free_order.rs"]
mod free_order;
pub(crate) use free_order::watch_free_order;

// Fixed test instrumentation capacity: the original 4096-row selected gate
// holds 4096 title allocations plus its frame, row/slot tables and owned
// backing. Four entries per maximum row leave a bounded margin for those
// fixed allocations. This changes no runtime/compiler quota: exhaustion still
// marks observation invalid, and the fixed tracking backing is excluded from the
// measured requested layouts just as before.
const SLOTS: usize = 16_384;
#[derive(Clone, Copy)]
struct Slot {
    pointer: usize,
    bytes: usize,
}
const EMPTY: Slot = Slot {
    pointer: 0,
    bytes: 0,
};
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Metrics {
    pub(crate) allocations: usize,
    pub(crate) reallocations: usize,
    pub(crate) allocated: usize,
    pub(crate) freed: usize,
    pub(crate) live: usize,
    pub(crate) highwater: usize,
    pub(crate) invalid: bool,
}

// Preallocated open-addressed instrumentation. Deletion shifts its probe chain,
// so callbacks never allocate, grow or accumulate tombstones across long runs.
struct ExtendedRegistry {
    slots: Vec<Slot>,
    live: usize,
    limit: usize,
}
impl ExtendedRegistry {
    fn home(pointer: usize, mask: usize) -> usize {
        pointer.rotate_right(4).wrapping_mul(0x9e37_79b9) & mask
    }
    fn insert(&mut self, pointer: usize, bytes: usize) -> Result<(), &'static str> {
        if self.live == self.limit {
            return Err("pointer table exhausted");
        }
        let mask = self.slots.len() - 1;
        let mut index = Self::home(pointer, mask);
        while self.slots[index].pointer != 0 {
            if self.slots[index].pointer == pointer {
                return Err("duplicate observed live pointer");
            }
            index = (index + 1) & mask;
        }
        self.slots[index] = Slot { pointer, bytes };
        self.live += 1;
        Ok(())
    }
    fn remove(&mut self, pointer: usize) -> Option<usize> {
        let mask = self.slots.len() - 1;
        let mut hole = Self::home(pointer, mask);
        while self.slots[hole].pointer != pointer {
            if self.slots[hole].pointer == 0 {
                return None;
            }
            hole = (hole + 1) & mask;
        }
        let bytes = self.slots[hole].bytes;
        let mut scan = (hole + 1) & mask;
        while self.slots[scan].pointer != 0 {
            let home = Self::home(self.slots[scan].pointer, mask);
            if (scan.wrapping_sub(home) & mask) >= (scan.wrapping_sub(hole) & mask) {
                self.slots[hole] = self.slots[scan];
                hole = scan;
            }
            scan = (scan + 1) & mask;
        }
        self.slots[hole] = EMPTY;
        self.live -= 1;
        Some(bytes)
    }
}
struct State {
    extended: Option<ExtendedRegistry>,
    slots: Vec<Slot>,
    metrics: Metrics,
}
impl State {
    const fn new() -> Self {
        Self {
            extended: None,
            slots: Vec::new(),
            metrics: Metrics {
                allocations: 0,
                reallocations: 0,
                allocated: 0,
                freed: 0,
                live: 0,
                highwater: 0,
                invalid: false,
            },
        }
    }
    fn allocated(&mut self, pointer: usize, bytes: usize) {
        free_order::record_acquire(pointer);
        if let Some(registry) = &mut self.extended {
            if let Err(reason) = registry.insert(pointer, bytes) {
                INVALID.with(|invalid| invalid.set(true));
                note_invalid_reason(reason);
                return;
            }
        } else {
            let Some(slot) = self.slots.iter_mut().find(|slot| slot.pointer == 0) else {
                INVALID.with(|invalid| invalid.set(true));
                note_invalid_reason("pointer table exhausted");
                return;
            };
            *slot = Slot { pointer, bytes };
        }
        let (Some(allocated), Some(live), Some(count)) = (
            self.metrics.allocated.checked_add(bytes),
            self.metrics.live.checked_add(bytes),
            self.metrics.allocations.checked_add(1),
        ) else {
            INVALID.with(|invalid| invalid.set(true));
            note_invalid_reason("allocation counter overflow");
            return;
        };
        self.metrics.allocated = allocated;
        self.metrics.live = live;
        self.metrics.allocations = count;
        self.metrics.highwater = self.metrics.highwater.max(live);
    }
    fn freed(&mut self, pointer: usize, bytes: usize) {
        free_order::record_free(pointer);
        let actual = if let Some(registry) = &mut self.extended {
            let Some(actual) = registry.remove(pointer) else {
                return;
            };
            actual
        } else {
            let Some(slot) = self.slots.iter_mut().find(|slot| slot.pointer == pointer) else {
                return;
            };
            let actual = slot.bytes;
            *slot = EMPTY;
            actual
        };
        if actual != bytes {
            INVALID.with(|invalid| invalid.set(true));
            note_invalid_reason("deallocation layout mismatch");
        }
        let (Some(freed), Some(live)) = (
            self.metrics.freed.checked_add(actual),
            self.metrics.live.checked_sub(actual),
        ) else {
            INVALID.with(|invalid| invalid.set(true));
            note_invalid_reason("deallocation counter underflow or overflow");
            return;
        };
        self.metrics.freed = freed;
        self.metrics.live = live;
    }
}
thread_local! {
    // Only the carrier lives in TLS. Its fixed backing is acquired before
    // activation; allocator callbacks never allocate, grow or format metadata.
    static STATE: RefCell<State> = const {RefCell::new(State::new())};
    static ACTIVE: Cell<bool> = const {Cell::new(false)};
    static INVALID: Cell<bool> = const {Cell::new(false)};
    static INVALID_REASON: Cell<Option<&'static str>> = const {Cell::new(None)};
}
// Preserve the invalid flag and every measured counter. First-cause text is
// fixed instrumentation only, so a failed probe can identify its own limit.
fn note_invalid_reason(reason: &'static str) {
    let _ = INVALID_REASON.try_with(|slot| {
        if slot.get().is_none() {
            slot.set(Some(reason));
        }
    });
}
pub(crate) fn invalid_reason() -> Option<&'static str> {
    INVALID_REASON.with(Cell::get)
}
fn active() -> bool {
    ACTIVE.try_with(Cell::get).unwrap_or(false)
}
fn record(callback: impl FnOnce(&mut State)) {
    let result = STATE.try_with(|state| match state.try_borrow_mut() {
        Ok(mut state) => callback(&mut state),
        Err(_) => {
            let _ = INVALID.try_with(|invalid| invalid.set(true));
            note_invalid_reason("reentrant observer callback");
        }
    });
    if result.is_err() {
        let _ = INVALID.try_with(|invalid| invalid.set(true));
        note_invalid_reason("observer thread-local unavailable");
    }
}
struct ObservedSystem;
#[global_allocator]
static ALLOCATOR: ObservedSystem = ObservedSystem;
// SAFETY: all pointers/layouts come from GlobalAlloc's caller. System remains
// the allocator of every backing; realloc copies min(old,new) before releasing
// the old allocation and leaves it intact if the new allocation fails.
#[allow(unsafe_code)]
unsafe impl GlobalAlloc for ObservedSystem {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forward the valid caller layout unchanged.
        let pointer = unsafe { System.alloc(layout) };
        if active() && !pointer.is_null() {
            record(|state| state.allocated(pointer as usize, layout.size()));
        }
        pointer
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forward the valid caller layout unchanged.
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if active() && !pointer.is_null() {
            record(|state| state.allocated(pointer as usize, layout.size()));
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        if active() {
            record(|state| state.freed(pointer as usize, layout.size()));
        }
        // SAFETY: this allocation still belongs to System with its original layout.
        unsafe { System.dealloc(pointer, layout) };
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if !active() {
            // SAFETY: preserve ordinary System behavior outside the probe thread.
            return unsafe { System.realloc(pointer, layout, size) };
        }
        let Ok(next) = Layout::from_size_align(size, layout.align()) else {
            INVALID.with(|invalid| invalid.set(true));
            note_invalid_reason("invalid replacement layout");
            return std::ptr::null_mut();
        };
        // SAFETY: allocate disjoint replacement backing of the requested alignment.
        let replacement = unsafe { System.alloc(next) };
        if replacement.is_null() {
            return replacement;
        }
        record(|state| {
            state.metrics.reallocations = state.metrics.reallocations.saturating_add(1);
            state.allocated(replacement as usize, size);
        });
        // SAFETY: both allocations are valid/nonoverlapping for min(old,new) bytes.
        unsafe { std::ptr::copy_nonoverlapping(pointer, replacement, layout.size().min(size)) };
        record(|state| state.freed(pointer as usize, layout.size()));
        // SAFETY: copied bytes are retained in replacement; release original once.
        unsafe { System.dealloc(pointer, layout) };
        replacement
    }
}
struct Guard;
impl Drop for Guard {
    fn drop(&mut self) {
        ACTIVE.with(|active| active.set(false));
        // Instrumentation is destroyed after the observed operation's unwind
        // and local Drop frontier, with observation already disabled.
        let registry = STATE.with(|state| state.borrow_mut().extended.take());
        drop(registry);
    }
}
/// Read the live counters inside an already active, nonnested observation.
/// This copies fixed instrumentation only and never changes its pointer table.
pub(crate) fn current_metrics() -> Metrics {
    assert!(active(), "live metrics require an active observation");
    STATE.with(|state| {
        let mut metrics = state.borrow().metrics;
        metrics.invalid = INVALID.with(Cell::get);
        metrics
    })
}
/// Attribute currently live observed layouts to an explicit owner graph.
/// The predicate must borrow existing data only: callbacks run while the
/// observation registry is borrowed. No metric, slot, or debit is rewritten.
pub(crate) fn live_bytes_matching(mut owns: impl FnMut(usize) -> bool) -> usize {
    assert!(active(), "live ownership requires an active observation");
    STATE.with(|state| {
        let state = state.borrow();
        let slots = state
            .extended
            .as_ref()
            .map_or(&state.slots[..], |registry| &registry.slots[..]);
        slots
            .iter()
            .filter(|slot| slot.pointer != 0 && owns(slot.pointer))
            .try_fold(0usize, |sum, slot| sum.checked_add(slot.bytes))
            .expect("live ownership layout sum overflowed")
    })
}

// Keep instrumentation outside the per-operation generic body. These helpers
// prepare metadata before activation and retain the same observation and unwind
// boundaries. Metadata backing is excluded, as the former TLS array was.
fn begin_observation() -> Guard {
    assert!(!active(), "allocation observation cannot nest");
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.slots.resize(SLOTS, EMPTY);
        state.slots.fill(EMPTY);
        if let Some(registry) = &mut state.extended {
            registry.slots.fill(EMPTY);
            registry.live = 0;
        }
        state.metrics = Metrics::default();
    });
    INVALID.with(|invalid| invalid.set(false));
    INVALID_REASON.with(|reason| reason.set(None));
    ACTIVE.with(|active| active.set(true));
    Guard
}
fn finish_observation(guard: Guard) -> Metrics {
    drop(guard);
    STATE.with(|state| {
        let mut metrics = state.borrow().metrics;
        metrics.invalid = INVALID.with(Cell::get);
        metrics
    })
}
pub(crate) fn measure<T>(operation: impl FnOnce() -> T) -> (T, Metrics) {
    let guard = begin_observation();
    let result = operation();
    let metrics = finish_observation(guard);
    (result, metrics)
}

/// Explicit test instrumentation size, never a compiler or runtime allowance.
/// The registry is wholly constructed before activation and dropped after it;
/// every operation allocation, replacement overlap and final Drop stays visible.
/// The default `measure` keeps its original fixed SLOTS exhaustion behavior.
pub(crate) fn measure_with_capacity<T>(
    pointer_limit: usize,
    operation: impl FnOnce() -> T,
) -> (T, Metrics) {
    assert!(!active(), "allocation observation cannot nest");
    assert!(pointer_limit > 0, "pointer registry must be nonempty");
    let slots = pointer_limit
        .checked_mul(2)
        .and_then(usize::checked_next_power_of_two)
        .expect("pointer instrumentation capacity overflow");
    let slots = vec![EMPTY; slots];
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        assert!(state.extended.is_none());
        state.extended = Some(ExtendedRegistry {
            slots,
            live: 0,
            limit: pointer_limit,
        });
    });
    measure(operation)
}

