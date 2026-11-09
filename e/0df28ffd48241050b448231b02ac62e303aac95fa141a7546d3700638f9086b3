//! Fixed active-interval maxima of the real allocator's simultaneous live
//! requested layouts. Earlier cumulative peaks cannot mask a later interval.
//! No backing allocation, ledger, refund, or estimate is introduced here.
use std::cell::{Cell, RefCell};

const CAPACITY: usize = 64;
thread_local! {
    static COUNT: Cell<usize> = const { Cell::new(0) };
    static PEAKS: RefCell<[Option<usize>; CAPACITY]> = const {
        RefCell::new([None; CAPACITY])
    };
}

pub(super) fn begin(live: usize) -> Option<usize> {
    PEAKS.with(|peaks| {
        let mut peaks = peaks.borrow_mut();
        let slot = peaks.iter().position(Option::is_none)?;
        peaks[slot] = Some(live);
        COUNT.with(|count| count.set(count.get() + 1));
        Some(slot)
    })
}

pub(super) fn finish(slot: usize) -> usize {
    let peak = PEAKS.with(|peaks| peaks.borrow_mut()[slot].take().unwrap());
    COUNT.with(|count| count.set(count.get() - 1));
    peak
}

pub(super) fn allocated(live: usize) {
    if !COUNT.try_with(|count| count.get() != 0).unwrap_or(false) {
        return;
    }
    // The allocator's ordinary callback guard owns State, not this separate
    // fixed TLS carrier. begin/finish allocate no memory and cannot reenter it.
    let _ = PEAKS.try_with(|peaks| {
        for peak in peaks.borrow_mut().iter_mut().flatten() {
            *peak = (*peak).max(live);
        }
    });
}
