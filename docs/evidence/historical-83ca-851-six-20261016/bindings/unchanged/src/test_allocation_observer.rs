//! Test-only, thread-bound observation of actual requested Rust Layout backing.
//! Other threads use System unchanged. The observed realloc route deliberately
//! allocates/copies/frees, retaining both buffers until copy completion; this is
//! an actual conforming allocator, not an estimated replacement-overlap charge.
//! These metrics are requested backing bytes, not malloc usable bytes or RSS.
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::{Cell, RefCell};
mod free_order;
pub(crate) use free_order::watch_free_order;

// Fixed test instrumentation capacity: the original 4096-row selected gate
// holds 4096 title allocations plus its frame, row/slot tables and owned
// backing. Four entries per maximum row leave a bounded margin for those
// fixed allocations. This changes no runtime/compiler quota: exhaustion still
// marks observation invalid, and the fixed tracking array is excluded from the
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
    slots: [Slot; SLOTS],
    metrics: Metrics,
}
impl State {
    const fn new() -> Self {
        Self {
            extended: None,
            slots: [EMPTY; SLOTS],
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
    // Const fixed storage is initialized before the phase; the observer never
    // allocates a registry, grows a container or formats text in a callback.
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
// allocate nothing and retain the same observation and unwind boundaries.
fn begin_observation() -> Guard {
    assert!(!active(), "allocation observation cannot nest");
    STATE.with(|state| {
        let mut state = state.borrow_mut();
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spare_capacity_and_replacement_overlap_are_observed_from_allocator_layouts() {
        let (capacities, metrics) = measure(|| {
            let mut bytes = Vec::<u8>::with_capacity(17);
            bytes.resize(17, 42);
            let first = bytes.capacity();
            bytes.reserve_exact(19);
            let second = bytes.capacity();
            assert!(bytes.iter().all(|value| *value == 42));
            std::hint::black_box(&bytes);
            (first, second)
        });
        assert_eq!(capacities, (17, 36));
        assert!(!metrics.invalid);
        assert_eq!(metrics.reallocations, 1);
        assert_eq!(metrics.allocated, 53);
        assert_eq!(metrics.freed, 53);
        assert_eq!(metrics.highwater, 53);
        assert_eq!(metrics.live, 0);
    }
    #[test]
    fn observation_boundaries_preserve_local_result_and_unwind_drop_timing() {
        struct Witness<'a>(&'a Cell<Option<bool>>);
        impl Drop for Witness<'_> {
            fn drop(&mut self) {
                self.0.set(Some(active()));
            }
        }
        let local = Cell::new(None);
        let returned = Cell::new(None);
        let (result, _) = measure(|| {
            let _local = Witness(&local);
            Witness(&returned)
        });
        assert_eq!(local.get(), Some(true));
        assert_eq!(returned.get(), None);
        assert!(!active());
        drop(result);
        assert_eq!(returned.get(), Some(false));

        let unwound = Cell::new(None);
        let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            measure(|| {
                let _local = Witness(&unwound);
                panic!("observe unwind boundary");
            });
        }));
        assert!(failure.is_err());
        assert_eq!(unwound.get(), Some(true));
        assert!(!active());
        let (value, _) = measure(|| {
            assert!(active());
            7
        });
        assert_eq!(value, 7);
        assert!(!active());
    }
    #[test]
    fn registry_overflow_is_invalid_evidence_instead_of_a_partial_success_count() {
        let mut owners = Vec::with_capacity(SLOTS + 1);
        let (_, metrics) = measure(|| {
            for _ in 0..=SLOTS {
                owners.push(Box::new(7u8));
            }
            std::hint::black_box(&owners);
        });
        assert!(metrics.invalid);
        assert!(metrics.live > 0);
        drop(owners);
    }
    #[test]
    fn explicit_registry_preserves_complete_layout_counters_and_default_capacity() {
        let operation = || {
            let mut bytes = Vec::<u8>::with_capacity(17);
            bytes.resize(17, 42);
            bytes.reserve_exact(19);
            assert!(bytes.iter().all(|value| *value == 42));
            std::hint::black_box(&bytes);
        };
        let (_, original) = measure(operation);
        let (_, extended) = measure_with_capacity(SLOTS * 8, operation);
        assert_eq!(original, extended);
        assert!(!extended.invalid && extended.live == 0);
        assert_eq!(extended.allocated, 53);
        assert_eq!(extended.freed, 53);
        assert_eq!(extended.highwater, 53);
        assert!(STATE.with(|state| state.borrow().extended.is_none()));
    }
    #[test]
    fn explicit_registry_tracks_every_live_owner_and_retains_overflow_refusal() {
        let mut owners = Vec::with_capacity(SLOTS + 1);
        let (_, metrics) = measure_with_capacity(SLOTS + 1, || {
            for _ in 0..=SLOTS {
                owners.push(Box::new(7u8));
            }
            assert_eq!(current_metrics().live, SLOTS + 1);
            owners.clear();
        });
        assert!(!metrics.invalid);
        assert_eq!(metrics.allocations, SLOTS + 1);
        assert_eq!(metrics.allocated, SLOTS + 1);
        assert_eq!(metrics.freed, SLOTS + 1);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.highwater, SLOTS + 1);
        let (_, overflow) = measure_with_capacity(1, || {
            let first = Box::new(7u8);
            let second = Box::new(8u8);
            std::hint::black_box((&first, &second));
            assert!(current_metrics().invalid);
            drop((first, second));
        });
        assert!(overflow.invalid);
        assert_eq!(invalid_reason(), Some("pointer table exhausted"));
        assert!(STATE.with(|state| state.borrow().extended.is_none()));
        // A later default probe cannot inherit the extended capacity.
        let mut owners = Vec::with_capacity(SLOTS + 1);
        let (_, default) = measure(|| {
            for _ in 0..=SLOTS {
                owners.push(Box::new(9u8));
            }
        });
        assert!(default.invalid);
        drop(owners);
    }
    #[test]
    fn explicit_registry_is_removed_after_real_unwind_and_next_default_probe() {
        let local = Cell::new(None);
        struct Witness<'a>(&'a Cell<Option<bool>>);
        impl Drop for Witness<'_> {
            fn drop(&mut self) {
                self.0.set(Some(active()));
            }
        }
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            measure_with_capacity(SLOTS * 8, || {
                let _witness = Witness(&local);
                let _bytes = Box::new([7u8; 32]);
                std::panic::resume_unwind(Box::new("explicit observer unwind"));
            });
        }));
        assert!(caught.is_err());
        drop(caught);
        assert_eq!(local.get(), Some(true));
        assert!(!active());
        assert!(STATE.with(|state| state.borrow().extended.is_none()));
        let (_, next) = measure(|| {
            drop(Box::new(7u8));
        });
        assert!(!next.invalid && next.allocated == 1 && next.freed == 1 && next.live == 0);
    }

    #[test]
    fn explicit_registry_preserves_wraparound_collisions_reuse_and_unknown_frees() {
        let mut registry = ExtendedRegistry {
            slots: vec![EMPTY; 64],
            live: 0,
            limit: 32,
        };
        let keys: Vec<_> = (1usize..=4096)
            .map(|n| n * 16)
            .filter(|pointer| ExtendedRegistry::home(*pointer, 63) == 63)
            .take(17)
            .collect();
        assert_eq!(keys.len(), 17);
        let backing = registry.slots.as_ptr();
        for (ordinal, pointer) in keys.iter().enumerate() {
            registry.insert(*pointer, ordinal + 1).unwrap();
        }
        assert_eq!(
            registry.insert(keys[0], 99),
            Err("duplicate observed live pointer")
        );
        assert_eq!(registry.remove(usize::MAX), None);
        for index in (0..keys.len()).step_by(2) {
            assert_eq!(registry.remove(keys[index]), Some(index + 1));
            assert_eq!(registry.remove(keys[index]), None);
        }
        for index in (1..keys.len()).step_by(2).rev() {
            assert_eq!(registry.remove(keys[index]), Some(index + 1));
        }
        assert_eq!(registry.live, 0);
        assert!(registry.slots.iter().all(|slot| slot.pointer == 0));
        for bytes in 1..=20_000 {
            registry.insert(keys[0], bytes).unwrap();
            assert_eq!(registry.remove(keys[0]), Some(bytes));
            assert_eq!(registry.live, 0);
            assert_eq!(registry.slots.as_ptr(), backing);
        }
    }
}

#[test]
fn live_pointer_owner_receipt_preserves_full_metrics_and_drop() {
    let (_, metrics) = measure(|| {
        let owner = vec![0u8; 32];
        let pointer = owner.as_ptr() as usize;
        let before = current_metrics();
        assert_eq!(live_bytes_matching(|candidate| candidate == pointer), 32);
        assert_eq!(live_bytes_matching(|_| false), 0);
        assert_eq!(before, current_metrics());
        drop(owner);
        assert_eq!(live_bytes_matching(|candidate| candidate == pointer), 0);
    });
    assert!(!metrics.invalid && metrics.live == 0);
    assert_eq!(metrics.allocated, metrics.freed);
}
