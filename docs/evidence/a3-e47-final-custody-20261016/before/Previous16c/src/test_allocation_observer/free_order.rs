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
