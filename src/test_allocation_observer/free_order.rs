//! Optional fixed test instrumentation; allocator counters remain unchanged.
use super::*;
const WATCHES: usize = 32;
#[derive(Clone, Copy)]
struct Watch {
    active: bool,
    pointers: [usize; WATCHES],
    expected: usize,
    order: [usize; WATCHES],
    count: usize,
    reused: [bool; WATCHES],
    invalid: bool,
}
impl Watch {
    const fn empty() -> Self {
        Self {
            active: false,
            pointers: [0; WATCHES],
            expected: 0,
            order: [usize::MAX; WATCHES],
            count: 0,
            reused: [false; WATCHES],
            invalid: false,
        }
    }
}
thread_local! {
    static WATCH: Cell<Watch> = const {Cell::new(Watch::empty())};
    static WATCH_ACTIVE: Cell<bool> = const {Cell::new(false)};
}
#[derive(Debug)]
pub(crate) struct FreeOrder {
    pub(crate) order: [usize; WATCHES],
    pub(crate) count: usize,
    pub(crate) invalid: bool,
}
// Follow the initial allocation's lifetime, rather than every later allocation
// that happens to receive the same numeric address from System.
pub(super) fn record_acquire(pointer: usize) {
    if !WATCH_ACTIVE.try_with(Cell::get).unwrap_or(false) {
        return;
    }
    let _ = WATCH.try_with(|slot| {
        let mut watch = slot.get();
        if !watch.active {
            return;
        }
        if let Some(ordinal) = watch.pointers[..watch.expected]
            .iter()
            .position(|p| *p == pointer)
        {
            if watch.order[..watch.count].contains(&ordinal) {
                watch.reused[ordinal] = true;
            } else {
                // A still-live watched allocation cannot be acquired again.
                watch.invalid = true;
            }
            slot.set(watch);
        }
    });
}
pub(super) fn record_free(pointer: usize) {
    if !WATCH_ACTIVE.try_with(Cell::get).unwrap_or(false) {
        return;
    }
    let _ = WATCH.try_with(|slot| {
        let mut watch = slot.get();
        if !watch.active {
            return;
        }
        if let Some(ordinal) = watch.pointers[..watch.expected]
            .iter()
            .position(|p| *p == pointer)
        {
            if watch.reused[ordinal] {
                return;
            }
            if watch.count == WATCHES || watch.order[..watch.count].contains(&ordinal) {
                watch.invalid = true;
            } else {
                watch.order[watch.count] = ordinal;
                watch.count += 1;
            }
            slot.set(watch);
        }
    });
}
/// Watch exact live backing pointers inside an existing observation. Unknown,
/// duplicate, zero or excessive watches fail explicitly; missing frees invalidate
/// the returned report. No pointer is dereferenced and no callback allocates.
/// A later allocation reusing a released address is outside the initial watch.
pub(crate) fn watch_free_order<T>(
    pointers: &[usize],
    operation: impl FnOnce() -> T,
) -> (T, FreeOrder) {
    assert!(
        active(),
        "free-order recording requires allocation observation"
    );
    assert!(
        !pointers.is_empty() && pointers.len() <= WATCHES,
        "free-order watch capacity"
    );
    assert!(
        !WATCH.with(|slot| slot.get().active),
        "free-order recording cannot nest"
    );
    for (i, pointer) in pointers.iter().enumerate() {
        assert!(
            *pointer != 0 && !pointers[..i].contains(pointer),
            "free-order watches must be distinct live pointers"
        );
        assert!(
            live_bytes_matching(|p| p == *pointer) != 0,
            "unknown free-order watch pointer"
        );
    }
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            WATCH_ACTIVE.with(|slot| slot.set(false));
            WATCH.with(|slot| slot.set(Watch::empty()));
        }
    }
    let mut watch = Watch::empty();
    watch.active = true;
    watch.expected = pointers.len();
    watch.pointers[..pointers.len()].copy_from_slice(pointers);
    WATCH.with(|slot| slot.set(watch));
    WATCH_ACTIVE.with(|slot| slot.set(true));
    let reset = Reset;
    let value = operation();
    let watch = WATCH.with(Cell::get);
    let result = FreeOrder {
        order: watch.order,
        count: watch.count,
        invalid: watch.invalid || watch.count != watch.expected,
    };
    drop(reset);
    (value, result)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Deterministic event-controller controls. The real owner remains live
    // until the watch ends; these callbacks model no actual deallocation.
    fn events(operation: impl FnOnce(usize)) -> FreeOrder {
        let (report, metrics) = measure(|| {
            let owner = vec![7u8; 64];
            let pointer = owner.as_ptr() as usize;
            let (_, report) = watch_free_order(&[pointer], || operation(pointer));
            drop(owner);
            report
        });
        assert!(!metrics.invalid && metrics.live == 0);
        assert_eq!(metrics.allocated, 64);
        assert_eq!(metrics.freed, 64);
        report
    }

    #[test]
    fn a_reacquired_address_does_not_repeat_the_initial_release_event() {
        let report = events(|pointer| {
            record_free(pointer);
            record_acquire(pointer);
            record_free(pointer);
            record_acquire(pointer);
            record_free(pointer);
        });
        assert!(!report.invalid);
        assert_eq!(report.count, 1);
        assert_eq!(report.order[0], 0);
    }

    #[test]
    fn duplicate_free_without_reacquisition_and_live_reacquisition_still_refuse() {
        let duplicate = events(|pointer| {
            record_free(pointer);
            record_free(pointer);
        });
        assert!(duplicate.invalid);
        assert_eq!(duplicate.count, 1);
        let live = events(|pointer| {
            record_acquire(pointer);
            record_free(pointer);
        });
        assert!(live.invalid);
        assert_eq!(live.count, 1);
    }

    #[test]
    fn missing_release_still_refuses_and_unwatched_events_do_not_hide_it() {
        let missing = events(|pointer| {
            record_acquire(pointer + 1);
            record_free(pointer + 1);
        });
        assert!(missing.invalid);
        assert_eq!(missing.count, 0);
        let complete = events(|pointer| {
            record_acquire(pointer + 1);
            record_free(pointer + 1);
            record_free(pointer);
        });
        assert!(!complete.invalid);
        assert_eq!(complete.count, 1);
    }

    #[test]
    fn allocator_replacement_keeps_the_initial_watch_and_all_layout_counters() {
        let (report, metrics) = measure(|| {
            let owner = Vec::<u8>::with_capacity(64);
            let pointer = owner.as_ptr() as usize;
            let (_, report) = watch_free_order(&[pointer], || {
                drop(owner);
                let mut replacement = vec![0u8; 64];
                replacement.reserve_exact(64);
                std::hint::black_box(&replacement);
                drop(replacement);
            });
            report
        });
        assert!(
            !report.invalid && !metrics.invalid,
            "{report:?} {metrics:?}"
        );
        assert_eq!(report.count, 1);
        assert_eq!(report.order[0], 0);
        assert_eq!(metrics.allocations, 3);
        assert_eq!(metrics.reallocations, 1);
        assert_eq!(metrics.allocated, 256);
        assert_eq!(metrics.freed, 256);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.highwater, 192);
    }
}
