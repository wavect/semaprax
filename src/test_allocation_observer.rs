//! Test-only, thread-bound observation of actual requested Rust Layout backing.
//! Other threads use System unchanged. The observed realloc route deliberately
//! allocates/copies/frees, retaining both buffers until copy completion; this is
//! an actual conforming allocator, not an estimated replacement-overlap charge.
//! These metrics are requested backing bytes, not malloc usable bytes or RSS.
include!("test_allocation_observer/backing.rs");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PhaseLabel {
    Reservation(crate::bounded_output::ReservationStage),
    Function(&'static str),
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PhaseObservation {
    stage: PhaseLabel,
    entry: Metrics,
    exit: Metrics,
    // Actual maximum simultaneous live bytes during THIS interval, including
    // its entry baseline and every nested allocator callback. Not a delta.
    local_peak: usize,
}

/// Fixed instrumentation, acquired outside the measured window. Intervals are
/// inclusive of nested stages; their highwaters are cumulative, not additive
/// or exclusive phase costs. Keep the occurrence with the largest highwater
/// for each stage, including its entry snapshot to expose an earlier peak.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PhaseTrace {
    pub(crate) intervals: [Option<PhaseObservation>; 64],
    local_intervals: [Option<PhaseObservation>; 64],
    pub(crate) overflow: bool,
}
impl PhaseTrace {
    /// Largest real interval peak for this owner, with that occurrence's
    /// entry/exit snapshots. Existing cumulative intervals remain independent.
    pub(crate) fn function_peak(&self, label: &'static str) -> Option<(Metrics, usize, Metrics)> {
        self.local_intervals
            .iter()
            .flatten()
            .find(|interval| interval.stage == PhaseLabel::Function(label))
            .map(|interval| (interval.entry, interval.local_peak, interval.exit))
    }
}
thread_local! {
    static PHASE_TRACE: RefCell<Option<PhaseTrace>> = const { RefCell::new(None) };
}

pub(crate) struct PhaseGuard(Option<(PhaseLabel, Metrics, usize)>);
pub(crate) fn phase_guard(stage: crate::bounded_output::ReservationStage) -> PhaseGuard {
    labeled_phase_guard(PhaseLabel::Reservation(stage))
}

/// Fixed diagnostic label; no reservation, allocation, or semantic authority.
pub(crate) fn function_phase_guard(function: &'static str) -> PhaseGuard {
    labeled_phase_guard(PhaseLabel::Function(function))
}

fn labeled_phase_guard(stage: PhaseLabel) -> PhaseGuard {
    let enabled = PHASE_TRACE.with(|trace| trace.borrow().is_some());
    PhaseGuard(if enabled && active() {
        let entry = current_metrics();
        if let Some(slot) = phase_peaks::begin(entry.live) {
            Some((stage, entry, slot))
        } else {
            PHASE_TRACE.with(|trace| trace.borrow_mut().as_mut().unwrap().overflow = true);
            None
        }
    } else {
        None
    })
}
impl Drop for PhaseGuard {
    fn drop(&mut self) {
        let Some((stage, entry, active_slot)) = self.0 else {
            return;
        };
        let exit = current_metrics();
        let local_peak = phase_peaks::finish(active_slot);
        PHASE_TRACE.with(|trace| {
            let mut trace = trace.borrow_mut();
            let trace = trace.as_mut().expect("phase trace outlives its guards");
            let existing = trace
                .intervals
                .iter()
                .position(|item| item.is_some_and(|item| item.stage == stage));
            let slot = existing.or_else(|| trace.intervals.iter().position(Option::is_none));
            if let Some(slot) = slot {
                if trace.intervals[slot].is_none_or(|old| {
                    exit.highwater > old.exit.highwater
                        || (exit.highwater == old.exit.highwater
                            && entry.highwater < old.entry.highwater)
                }) {
                    trace.intervals[slot] = Some(PhaseObservation {
                        stage,
                        entry,
                        exit,
                        local_peak,
                    });
                }
                if trace.local_intervals[slot].is_none_or(|old| local_peak > old.local_peak) {
                    trace.local_intervals[slot] = Some(PhaseObservation {
                        stage,
                        entry,
                        exit,
                        local_peak,
                    });
                }
            } else {
                trace.overflow = true;
            }
        });
    }
}

pub(crate) fn with_phase_trace<T>(operation: impl FnOnce() -> T) -> (T, PhaseTrace) {
    assert!(
        !active(),
        "prepare phase instrumentation before observation"
    );
    PHASE_TRACE.with(|trace| {
        let mut trace = trace.borrow_mut();
        assert!(trace.is_none(), "phase traces cannot nest");
        *trace = Some(PhaseTrace {
            intervals: [None; 64],
            local_intervals: [None; 64],
            overflow: false,
        });
    });
    struct Restore;
    impl Drop for Restore {
        fn drop(&mut self) {
            PHASE_TRACE.with(|trace| {
                trace.borrow_mut().take();
            });
        }
    }
    let restore = Restore;
    let value = operation();
    let trace = PHASE_TRACE.with(|trace| trace.borrow_mut().take().unwrap());
    drop(restore);
    (value, trace)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interval_peak_is_not_masked_by_an_earlier_cumulative_peak() {
        let operation = || {
            let early = vec![1_u8; 4096];
            std::hint::black_box(&early);
            drop(early);
            let retained = vec![2_u8; 37];
            {
                let _guard = function_phase_guard("test::later_overlap");
                let scratch = vec![3_u8; 113];
                std::hint::black_box((&retained, &scratch));
            }
            drop(retained);
        };
        let (_, plain) = measure(operation);
        let (((), observed), trace) = with_phase_trace(|| measure(operation));
        assert_eq!(plain, observed);
        assert_eq!(observed.highwater, 4096);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
        assert!(!observed.invalid);
        assert!(!trace.overflow);
        let (entry, peak, exit) = trace.function_peak("test::later_overlap").unwrap();
        assert_eq!((entry.live, peak, exit.live), (37, 150, 37));
        assert_eq!((entry.highwater, exit.highwater), (4096, 4096));
    }

    #[test]
    fn interval_peak_tracks_nested_replacement_overlap_and_each_occurrence() {
        let operation = || {
            let _outer = function_phase_guard("test::outer_peak");
            let retained = vec![0_u8; 31];
            for size in [127, 43] {
                let _inner = function_phase_guard("test::repeated_peak");
                let mut scratch = Vec::<u8>::with_capacity(size);
                scratch.resize(size, 9);
                scratch.reserve_exact(size);
                std::hint::black_box((&retained, &scratch));
            }
        };
        let (_, plain) = measure(operation);
        let (((), observed), trace) = with_phase_trace(|| measure(operation));
        assert_eq!(plain, observed);
        assert!(!observed.invalid);
        assert!(!trace.overflow);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
        let (entry, peak, exit) = trace.function_peak("test::repeated_peak").unwrap();
        assert_eq!((entry.live, peak, exit.live), (31, 31 + 127 + 254, 31));
        assert_eq!(trace.function_peak("test::outer_peak").unwrap().1, peak);
        assert_eq!(observed.highwater, peak);
    }
    #[test]
    fn phase_trace_keeps_nested_intervals_without_allocating_observed_backing() {
        use crate::bounded_output::{with_reservation_stage, ReservationStage};
        let operation = || {
            with_reservation_stage(ReservationStage::HirLowering, || {
                let bytes = vec![7_u8; 257];
                with_reservation_stage(ReservationStage::HirValidation, || {
                    std::hint::black_box(&bytes);
                });
                drop(bytes);
            });
        };
        let (_, plain) = measure(operation);
        let (((), observed), trace) = with_phase_trace(|| measure(operation));
        assert_eq!(plain, observed, "tracing changes no observed layouts");
        assert_eq!(observed.live, 0);
        assert!(!observed.invalid);
        assert!(!trace.overflow);
        assert_eq!(trace.intervals.iter().flatten().count(), 2);
        for phase in trace.intervals.iter().flatten() {
            assert_eq!(phase.exit.highwater, observed.highwater);
            match phase.stage {
                PhaseLabel::Reservation(ReservationStage::HirLowering) => {
                    assert_eq!(phase.entry.live, 0);
                    assert_eq!(phase.exit.live, 0);
                }
                PhaseLabel::Reservation(ReservationStage::HirValidation) => {
                    assert_eq!(phase.entry.live, 257);
                    assert_eq!(phase.exit.live, 257);
                }
                _ => panic!("unexpected allocation interval"),
            }
        }
    }
    #[test]
    fn named_validation_intervals_are_inclusive_and_do_not_perturb_outcomes_or_layouts() {
        let operation = || {
            let _outer = function_phase_guard("test::validation");
            let bytes = vec![9_u8; 67];
            {
                let _inner = function_phase_guard("test::validation::callee");
                std::hint::black_box(&bytes);
            }
            drop(bytes);
            23
        };
        let (plain_value, plain) = measure(operation);
        let ((observed_value, observed), trace) = with_phase_trace(|| measure(operation));
        assert_eq!((plain_value, plain), (observed_value, observed));
        assert!(!trace.overflow);
        assert_eq!(trace.intervals.iter().flatten().count(), 2);
        for interval in trace.intervals.iter().flatten() {
            assert!(interval.entry.highwater <= interval.exit.highwater);
            assert_eq!(interval.exit.highwater, observed.highwater);
            match interval.stage {
                PhaseLabel::Function("test::validation") => {
                    assert_eq!((interval.entry.live, interval.exit.live), (0, 0));
                }
                PhaseLabel::Function("test::validation::callee") => {
                    assert_eq!((interval.entry.live, interval.exit.live), (67, 67));
                }
                _ => panic!("unexpected fixed interval label"),
            }
        }
    }

    #[test]
    fn named_validation_interval_overflow_is_explicit_and_allocation_free() {
        const LABELS: [&str; 65] = [
            "fixed::0",
            "fixed::1",
            "fixed::2",
            "fixed::3",
            "fixed::4",
            "fixed::5",
            "fixed::6",
            "fixed::7",
            "fixed::8",
            "fixed::9",
            "fixed::10",
            "fixed::11",
            "fixed::12",
            "fixed::13",
            "fixed::14",
            "fixed::15",
            "fixed::16",
            "fixed::17",
            "fixed::18",
            "fixed::19",
            "fixed::20",
            "fixed::21",
            "fixed::22",
            "fixed::23",
            "fixed::24",
            "fixed::25",
            "fixed::26",
            "fixed::27",
            "fixed::28",
            "fixed::29",
            "fixed::30",
            "fixed::31",
            "fixed::32",
            "fixed::33",
            "fixed::34",
            "fixed::35",
            "fixed::36",
            "fixed::37",
            "fixed::38",
            "fixed::39",
            "fixed::40",
            "fixed::41",
            "fixed::42",
            "fixed::43",
            "fixed::44",
            "fixed::45",
            "fixed::46",
            "fixed::47",
            "fixed::48",
            "fixed::49",
            "fixed::50",
            "fixed::51",
            "fixed::52",
            "fixed::53",
            "fixed::54",
            "fixed::55",
            "fixed::56",
            "fixed::57",
            "fixed::58",
            "fixed::59",
            "fixed::60",
            "fixed::61",
            "fixed::62",
            "fixed::63",
            "fixed::64",
        ];
        let operation = || {
            for label in LABELS {
                let _guard = function_phase_guard(label);
                std::hint::black_box(label);
            }
        };
        let (_, plain) = measure(operation);
        let (((), observed), trace) = with_phase_trace(|| measure(operation));
        assert_eq!(plain, observed);
        assert_eq!(observed.allocated, 0);
        assert_eq!(trace.intervals.iter().flatten().count(), 64);
        assert!(trace.overflow);
    }

    #[test]
    fn fixed_registry_backing_is_lazy_reused_and_outside_the_observed_window() {
        assert_eq!(SLOTS, 16_384);
        assert!(std::mem::size_of::<State>() < std::mem::size_of::<[Slot; SLOTS]>());
        std::thread::Builder::new()
            .stack_size(128 * 1024)
            .spawn(|| {
                assert!(STATE.with(|state| state.borrow().slots.is_empty()));
                let (_, metrics) = measure(|| {
                    let mut bytes = Vec::<u8>::with_capacity(17);
                    bytes.resize(17, 7);
                    std::hint::black_box(&bytes);
                });
                assert_eq!(
                    metrics,
                    Metrics {
                        allocations: 1,
                        reallocations: 0,
                        allocated: 17,
                        freed: 17,
                        live: 0,
                        highwater: 17,
                        invalid: false,
                    }
                );
                let pointer = STATE.with(|state| {
                    let state = state.borrow();
                    assert_eq!(state.slots.len(), SLOTS);
                    state.slots.as_ptr()
                });
                let (_, empty) = measure(|| ());
                assert_eq!(empty, Metrics::default());
                assert_eq!(STATE.with(|state| state.borrow().slots.as_ptr()), pointer);
            })
            .unwrap()
            .join()
            .unwrap();
    }
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
