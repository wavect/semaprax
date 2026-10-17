//! Fixed-storage, test-only observation of completed module forecasts and actual uncached peak passes.
//! It changes no debit, forecast, source validation, or admission result.
use std::cell::{Cell, RefCell};
const SLOTS: usize = 256;
const PATH: usize = 256;
#[derive(Clone, Copy)]
struct Record {
    path: [u8; PATH],
    len: usize,
    mode: u8,
    pass: Option<u8>,
    global: bool,
    semantic: bool,
    authority_request: bool,
    values: [usize; 17],
}
struct State {
    entries: Vec<Option<Record>>,
    count: usize,
    oversized_paths: usize,
    forecasts: [usize; 8],
    current_pass: Option<u8>,
    authority_requests: usize,
}
impl State {
    const fn new() -> Self {
        Self {
            entries: Vec::new(),
            count: 0,
            oversized_paths: 0,
            forecasts: [0; 8],
            current_pass: None,
            authority_requests: 0,
        }
    }
}
thread_local! {
    static ACTIVE: Cell<bool> = const {Cell::new(false)};
    static STATE: RefCell<State> = const {RefCell::new(State::new())};
}
pub(crate) fn record(path: &str, mode: u8, semantic: bool, values: [usize; 17]) {
    if !matches!(mode, 4..=7) || !ACTIVE.with(Cell::get) {
        return;
    }
    capture(path, mode, semantic, false, false, values);
}
pub(crate) fn authority_index(
    path: &str,
    indexed: usize,
    node: usize,
    requested: usize,
    remaining: Option<usize>,
) {
    if !ACTIVE.with(Cell::get) {
        return;
    }
    let mut values = [0; 17];
    values[..5].copy_from_slice(&[
        indexed,
        node,
        requested,
        remaining.unwrap_or(usize::MAX),
        usize::from(remaining.is_some()),
    ]);
    capture(path, 0, false, true, false, values);
}
fn capture(
    path: &str,
    mode: u8,
    semantic: bool,
    authority_request: bool,
    global: bool,
    values: [usize; 17],
) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        if path.len() > PATH {
            state.oversized_paths += 1;
            return;
        }
        let mut stored = Record {
            path: [0; PATH],
            len: path.len(),
            mode,
            pass: state.current_pass,
            global,
            semantic,
            authority_request,
            values,
        };
        stored.path[..path.len()].copy_from_slice(path.as_bytes());
        let index = state.count % SLOTS;
        state.entries[index] = Some(stored);
        state.count += 1;
        if authority_request {
            state.authority_requests += 1;
        } else if !global {
            state.forecasts[usize::from(mode)] += 1;
        }
    });
}
/// An actual execution-peak formula invocation, not a guessed layout mode.
/// A failed/early-returned pass publishes null totals, never the last partial sum.
pub(crate) struct Pass {
    mode: u8,
    active: bool,
    completed: bool,
}
pub(crate) fn begin_pass(mode: u8) -> Pass {
    let active = ACTIVE.with(Cell::get);
    if active {
        STATE.with(|state| {
            let mut state = state.borrow_mut();
            assert!(state.current_pass.is_none());
            state.current_pass = Some(mode);
        });
    }
    Pass {
        mode,
        active,
        completed: false,
    }
}
impl Pass {
    pub(crate) fn complete(&mut self, peak: usize, retained: usize, transient: usize) {
        if self.active {
            let mut values = [0; 17];
            values[..4].copy_from_slice(&[1, peak, retained, transient]);
            capture("", self.mode, false, false, true, values);
        }
        self.completed = true;
    }
}
impl Drop for Pass {
    fn drop(&mut self) {
        if self.active {
            if !self.completed {
                capture("", self.mode, false, false, true, [0; 17]);
            }
            STATE.with(|state| state.borrow_mut().current_pass = None);
        }
    }
}
/// Fixed metadata backing is prepared before the callback, which runs under
/// unchanged production budgets. Reporting serializes only after it returns.
/// Ring truncation is explicitly reported.
pub(crate) fn observe<T>(application: &str, operation: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            ACTIVE.with(|a| a.set(self.0));
        }
    }
    assert!(!ACTIVE.with(Cell::get), "forecast observers cannot nest");
    STATE.with(|s| {
        let mut state = s.borrow_mut();
        state.entries.resize(SLOTS, None);
        state.entries.fill(None);
        state.count = 0;
        state.oversized_paths = 0;
        state.forecasts = [0; 8];
        state.current_pass = None;
        state.authority_requests = 0;
    });
    let guard = Restore(ACTIVE.with(|a| a.replace(true)));
    let result = operation();
    drop(guard);
    STATE.with(|state| {
        let state=state.borrow();
        eprintln!("{}",serde_json::json!({"schema":"semaprax.forecast-observation.v1",
            "application":application,"completed_mode7_forecasts":state.forecasts[7],
            "completed_module_forecasts_by_layout_mode":state.forecasts,"authority_index_requests":state.authority_requests,
            "retained_records":state.count.min(SLOTS),"discarded_prefix_records":state.count.saturating_sub(SLOTS),
            "oversized_path_records":state.oversized_paths,
            "scope":"actual per-module static forecasts and uncached formula passes; sums are not global peaks, allocations or application fit",
            "observer_metadata":"fixed 256-record ring and fixed path buffers excluded from builder/account observations"}));
        for ordinal in state.count.saturating_sub(SLOTS)..state.count {
            let item=state.entries[ordinal%SLOTS].unwrap();
            let v=item.values;
            if item.global {
                eprintln!("{}",serde_json::json!({"schema":"semaprax.forecast-pass.v1",
                    "application":application,"ordinal":ordinal,"execution_mode":item.mode,
                    "completed":v[0]!=0,"global_peak":(v[0]!=0).then_some(v[1]),
                    "retained_sum":(v[0]!=0).then_some(v[2]),
                    "transient_ast_peak":(v[0]!=0 && item.mode==5).then_some(v[3]),
                    "residual":null,"scope":"static forecast only; incomplete pass totals unavailable"}));
                continue;
            }
            if item.authority_request {
                assert_eq!(v[0].checked_mul(v[1]),Some(v[2]));
                eprintln!("{}",serde_json::json!({"schema":"semaprax.authority-index-request.v1",
                    "application":application,"ordinal":ordinal,
                    "caller_path":std::str::from_utf8(&item.path[..item.len]).unwrap(),
                    "indexed_declarations":v[0],"btree_node_upper":v[1],"requested_bytes":v[2],
                    "enclosing_remaining":(v[4]!=0).then_some(v[3]),
                    "scope":"request before actual authority-index reservation; no grant or allocation observation"}));
                continue;
            }
            assert_eq!(v[13],v[0]+v[10]+v[11]+v[12]);
            eprintln!("{}",serde_json::json!({"schema":"semaprax.forecast-component.v1",
                "application":application,"ordinal":ordinal,
                "layout_mode":item.mode,"execution_peak_pass":item.pass,
                "path":std::str::from_utf8(&item.path[..item.len]).unwrap(),
                "retention":if item.semantic {"Semantic"} else {"Parsed"},
                "raw_total":v[0],"raw_strings":v[1],"raw_exact_expressions":v[2],
                "raw_literal_absence_bytes":v[3],"runtime_total":v[4],"runtime_strings":v[5],
                "runtime_exact_expressions":v[6],"structure_input":v[7],"string_input":v[8],
                "fixed_upper_before_layout_adjustment":v[9],"fixed_upper_after_layout_adjustment":v[10],
                "identity_occurrence_upper":v[11],"identity_carrier_upper":v[12],
                "retained_clone_and_hir":v[13],"generic_instances":v[14],"transient_import_clone":v[15],
                "original_mode7_fixed_upper":(item.mode==7 && v[16]!=usize::MAX).then_some(v[16]),
                "original_formula_overflow":(item.mode==7).then_some(v[16]==usize::MAX)}));
        }
    });
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ring_backing_is_lazy_and_prepared_before_application_allocation_observation() {
        assert_eq!((SLOTS, PATH), (256, 256));
        assert!(std::mem::size_of::<State>() < std::mem::size_of::<[Option<Record>; SLOTS]>());
        std::thread::Builder::new()
            .stack_size(128 * 1024)
            .spawn(|| {
                assert!(STATE.with(|state| state.borrow().entries.is_empty()));
                let (marker, metrics) = observe("lazy-ring", || {
                    crate::test_allocation_observer::measure(|| {
                        let bytes = vec![7u8; 17];
                        std::hint::black_box(&bytes);
                        record("source", 4, false, [0; 17]);
                        31
                    })
                });
                assert_eq!(marker, 31);
                assert!(!metrics.invalid);
                assert_eq!(
                    (metrics.allocated, metrics.freed, metrics.live),
                    (17, 17, 0)
                );
                let pointer = STATE.with(|state| {
                    let state = state.borrow();
                    assert_eq!(state.entries.len(), SLOTS);
                    assert_eq!(state.count, 1);
                    state.entries.as_ptr()
                });
                observe("empty-ring", || ());
                STATE.with(|state| {
                    let state = state.borrow();
                    assert_eq!(state.entries.as_ptr(), pointer);
                    assert_eq!(state.count, 0);
                    assert!(state.entries.iter().all(Option::is_none));
                });
            })
            .unwrap()
            .join()
            .unwrap();
    }
    #[test]
    fn actual_pass_modes_keep_components_peaks_and_incomplete_totals_distinct() {
        let marker = observe("observer-contract", || {
            let mut five = begin_pass(5);
            record("same-source", 4, false, [0; 17]);
            five.complete(11, 7, 4);
            drop(five);
            let six = begin_pass(6);
            record("same-source", 5, false, [0; 17]);
            drop(six); // refusal before a completed global formula
            record("same-source", 7, true, [0; 17]);
            31
        });
        assert_eq!(marker, 31);
        STATE.with(|state| {
            let state = state.borrow();
            assert_eq!(state.forecasts, [0, 0, 0, 0, 1, 1, 0, 1]);
            assert_eq!(state.count, 5);
            let component = state.entries[0].unwrap();
            assert_eq!((component.mode, component.pass), (4, Some(5)));
            let peak = state.entries[1].unwrap();
            assert!(peak.global);
            assert_eq!(peak.mode, 5);
            assert_eq!(&peak.values[..4], &[1, 11, 7, 4]);
            let incomplete = state.entries[3].unwrap();
            assert!(incomplete.global);
            assert_eq!(incomplete.mode, 6);
            assert_eq!(incomplete.values, [0; 17]);
            assert!(state.current_pass.is_none());
        });
        observe("no-forecast", || ());
        STATE.with(|state| {
            let state = state.borrow();
            assert_eq!(state.count, 0);
            assert_eq!(state.forecasts, [0; 8]);
        });
    }
}
