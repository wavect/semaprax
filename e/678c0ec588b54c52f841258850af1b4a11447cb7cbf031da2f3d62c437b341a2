//! Fixed-storage, test-only observation of completed module forecasts and actual uncached peak passes.
//! It changes no debit, forecast, source validation, or admission result.
use std::cell::{Cell, RefCell};
use std::io::Write;
const SLOTS: usize = 256;
const PATH: usize = 256;
const VALUES: usize = 19;
#[derive(Clone, Copy)]
enum Kind {
    Component,
    Legacy,
    Authority,
    Global,
    Step,
}
#[derive(Clone, Copy)]
struct Record {
    path: [u8; PATH],
    len: usize,
    mode: u8,
    pass: Option<(u8, usize)>,
    module: Option<usize>,
    global: bool,
    step: bool,
    semantic: bool,
    include_imports: bool,
    legacy_components: bool,
    authority_request: bool,
    values: [usize; VALUES],
}
struct State {
    entries: Vec<Option<Record>>,
    count: usize,
    oversized_paths: usize,
    forecasts: [usize; 8],
    current_pass: Option<(u8, usize)>,
    current_module: Option<usize>,
    next_pass: usize,
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
            current_module: None,
            next_pass: 0,
            authority_requests: 0,
        }
    }
    fn lost_records(&self) -> usize {
        self.count
            .saturating_sub(SLOTS)
            .saturating_add(self.oversized_paths)
    }
}
thread_local! {
    static ACTIVE: Cell<bool> = const {Cell::new(false)};
    static STATE: RefCell<State> = const {RefCell::new(State::new())};
}
// Complete independently compiled historical producers keep their original API.
// They did not supply imports/prelude/HIR fields, so no complete current peak
// attribution may be inferred from a mixed legacy observation.
pub(crate) fn record(path: &str, mode: u8, semantic: bool, old: [usize; 17]) {
    if !matches!(mode, 4..=7) || !ACTIVE.with(Cell::get) {
        return;
    }
    let mut values = [0; VALUES];
    values[..17].copy_from_slice(&old);
    capture(path, mode, semantic, false, Kind::Legacy, values);
}
pub(crate) fn record_components(
    path: &str,
    mode: u8,
    semantic: bool,
    include_imports: bool,
    values: [usize; VALUES],
) {
    if !matches!(mode, 4..=7) || !ACTIVE.with(Cell::get) {
        return;
    }
    capture(
        path,
        mode,
        semantic,
        include_imports,
        Kind::Component,
        values,
    );
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
    let mut values = [0; VALUES];
    values[..5].copy_from_slice(&[
        indexed,
        node,
        requested,
        remaining.unwrap_or(usize::MAX),
        usize::from(remaining.is_some()),
    ]);
    capture(path, 0, false, false, Kind::Authority, values);
}
fn capture(
    path: &str,
    mode: u8,
    semantic: bool,
    include_imports: bool,
    kind: Kind,
    values: [usize; VALUES],
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
            module: state.current_module,
            global: matches!(kind, Kind::Global),
            step: matches!(kind, Kind::Step),
            semantic,
            include_imports,
            legacy_components: matches!(kind, Kind::Legacy),
            authority_request: matches!(kind, Kind::Authority),
            values,
        };
        stored.path[..path.len()].copy_from_slice(path.as_bytes());
        let index = state.count % SLOTS;
        state.entries[index] = Some(stored);
        state.count += 1;
        if matches!(kind, Kind::Authority) {
            state.authority_requests += 1;
        } else if matches!(kind, Kind::Component | Kind::Legacy) {
            state.forecasts[usize::from(mode)] += 1;
        }
    });
}
/// An actual formula invocation. Incomplete passes never publish totals.
pub(crate) struct Pass {
    mode: u8,
    active: bool,
    completed: bool,
    peak: usize,
    winner: Option<usize>,
    last_module: Option<usize>,
}
pub(crate) fn begin_pass(mode: u8) -> Pass {
    let active = ACTIVE.with(Cell::get);
    if active {
        STATE.with(|state| {
            let mut state = state.borrow_mut();
            assert!(state.current_pass.is_none());
            let id = state.next_pass;
            state.next_pass += 1;
            state.current_pass = Some((mode, id));
            state.current_module = None;
        });
    }
    Pass {
        mode,
        active,
        completed: false,
        peak: 0,
        winner: None,
        last_module: None,
    }
}
impl Pass {
    pub(crate) fn module(&self, index: usize) {
        if self.active {
            STATE.with(|state| state.borrow_mut().current_module = Some(index));
        }
    }
    /// Copies only actual, successful mode6 step results; no formula is rerun.
    pub(crate) fn step(
        &mut self,
        path: &str,
        index: usize,
        ordinal: usize,
        before_and_costs: (usize, usize, usize),
        outcome: (usize, usize),
    ) {
        let (retained_before, full, retained_cost) = before_and_costs;
        let (peak, retained) = outcome;
        if !self.active {
            return;
        }
        if peak > self.peak {
            self.winner = Some(index);
        }
        self.peak = peak;
        self.last_module = Some(index);
        self.module(index);
        let mut values = [0; VALUES];
        values[..8].copy_from_slice(&[
            index,
            ordinal,
            retained_before,
            full,
            retained_cost,
            peak,
            retained,
            self.winner.unwrap_or(usize::MAX),
        ]);
        capture(path, self.mode, false, false, Kind::Step, values);
    }
    pub(crate) fn complete(&mut self, peak: usize, retained: usize, transient: usize) {
        if self.active {
            let mut values = [0; VALUES];
            let retained_wins = peak > self.peak;
            let winner = if retained_wins {
                self.last_module
            } else {
                self.winner
            };
            values[..6].copy_from_slice(&[
                1,
                peak,
                retained,
                transient,
                winner.unwrap_or(usize::MAX),
                usize::from(retained_wins),
            ]);
            capture("", self.mode, false, false, Kind::Global, values);
        }
        self.completed = true;
    }
}
impl Drop for Pass {
    fn drop(&mut self) {
        if self.active {
            if !self.completed {
                capture("", self.mode, false, false, Kind::Global, [0; VALUES]);
            }
            STATE.with(|state| {
                let mut state = state.borrow_mut();
                state.current_pass = None;
                state.current_module = None;
            });
        }
    }
}
fn prepare() {
    assert!(!ACTIVE.with(Cell::get), "forecast observers cannot nest");
    STATE.with(|s| {
        let mut state = s.borrow_mut();
        state.entries.resize(SLOTS, None);
        state.entries.fill(None);
        state.count = 0;
        state.oversized_paths = 0;
        state.forecasts = [0; 8];
        state.current_pass = None;
        state.current_module = None;
        state.next_pass = 0;
        state.authority_requests = 0;
    });
}
/// Storage is acquired before fixtures/budgets. Declare before all owned fixtures
/// so reporting runs after their Drop on both return and unwinding.
pub(crate) struct Report {
    application: &'static str,
}
pub(crate) struct Capture;
impl Report {
    pub(crate) fn prepare(application: &'static str) -> Self {
        prepare();
        Self { application }
    }
    /// Only the original builder call belongs inside this lexical scope.
    pub(crate) fn capture(&self) -> Capture {
        assert!(
            !ACTIVE.with(|active| active.replace(true)),
            "forecast observers cannot nest"
        );
        Capture
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        let _ = ACTIVE.try_with(|active| active.set(false));
    }
}
impl Drop for Report {
    fn drop(&mut self) {
        // No assert/unwrap, infallible print macro, or owner panic interception.
        // Diagnostic IO failures are ignored; the original Result/panic survives.
        let mut stderr = std::io::stderr().lock();
        write_observation(&mut stderr, self.application, false);
    }
}
fn hir_sum(v: &[usize; VALUES]) -> Option<usize> {
    v[10]
        .checked_add(v[17])
        .and_then(|n| n.checked_add(v[12]))
        .and_then(|n| n.checked_add(v[11]))
}
fn reconciles(v: &[usize; VALUES]) -> bool {
    hir_sum(v) == Some(v[18]) && v[0].checked_add(v[18]) == Some(v[13])
}
fn legacy_hir_sum(v: &[usize; VALUES]) -> Option<usize> {
    v[10].checked_add(v[11]).and_then(|n| n.checked_add(v[12]))
}
fn record_reconciles(item: &Record) -> bool {
    if item.legacy_components {
        legacy_hir_sum(&item.values).and_then(|n| n.checked_add(item.values[0]))
            == Some(item.values[13])
    } else {
        reconciles(&item.values)
    }
}
fn write_row(output: &mut impl Write, value: serde_json::Value) {
    let _ = serde_json::to_writer(&mut *output, &value);
    let _ = output.write_all(b"\n");
}
fn write_observation(output: &mut impl Write, application: &str, strict: bool) {
    let _ = STATE.try_with(|state| {
        let Ok(state) = state.try_borrow() else {
            write_row(output, serde_json::json!({"schema":"semaprax.forecast-observation.v1","application":application,"reconciliation_error":true,"complete_records":false}));
            return;
        };
        let errors = state.entries.iter().flatten().filter(|item|
            (!item.global && !item.step && !item.authority_request && !record_reconciles(item))
                || (item.authority_request && item.values[0].checked_mul(item.values[1]) != Some(item.values[2]))
        ).count();
        let legacy = state.entries.iter().flatten().filter(|item| item.legacy_components).count();
        let complete = state.lost_records() == 0 && errors == 0 && legacy == 0;
        write_row(output, serde_json::json!({"schema":"semaprax.forecast-observation.v1",
            "application":application,"completed_mode7_forecasts":state.forecasts[7],
            "completed_module_forecasts_by_layout_mode":state.forecasts,"authority_index_requests":state.authority_requests,
            "retained_records":state.count.min(SLOTS),"discarded_prefix_records":state.count.saturating_sub(SLOTS),
            "oversized_path_records":state.oversized_paths,"lost_records":state.lost_records(),
            "reconciliation_errors":errors,"legacy_component_records":legacy,"complete_records":complete,
            "scope":"actual static forecasts and successful mode6 steps; sums are not global peaks, allocations or application fit",
            "observer_metadata":"fixed 256-record ring/path buffers allocated before fixtures; no allocation during collection"}));
        for ordinal in state.count.saturating_sub(SLOTS)..state.count {
            let Some(item) = state.entries.get(ordinal % SLOTS).copied().flatten() else { continue; };
            let v = item.values;
            let path = item.path.get(..item.len).and_then(|bytes| std::str::from_utf8(bytes).ok()).unwrap_or("<invalid-path>");
            if item.global {
                write_row(output, serde_json::json!({"schema":"semaprax.forecast-pass.v1",
                    "application":application,"ordinal":ordinal,"execution_mode":item.mode,"pass_id":item.pass.map(|p|p.1),
                    "completed":v[0]!=0,"complete_attribution":v[0]!=0 && complete,
                    "global_peak":(v[0]!=0 && complete).then_some(v[1]),"retained_sum":(v[0]!=0 && complete).then_some(v[2]),
                    "transient_ast_peak":(v[0]!=0 && complete && item.mode==5).then_some(v[3]),
                    "winning_module_index":(v[0]!=0 && complete && item.mode==6 && v[4]!=usize::MAX).then_some(v[4]),
                    "winning_term":if item.mode==6 && v[0]!=0 && complete {Some(if v[5]!=0 {"retained_sum"} else {"resolve_step"})} else {None},
                    "winner_selection":"first strict resolve maximum; final retained sum only when larger",
                    "residual":null,"scope":"static forecast only; incomplete/lost/invalid pass totals unavailable"}));
            } else if item.step {
                write_row(output, serde_json::json!({"schema":"semaprax.forecast-mode6-step.v1",
                    "application":application,"ordinal":ordinal,"pass_id":item.pass.map(|p|p.1),"path":path,
                    "module_index":v[0],"sorted_step":v[1],"retained_before":v[2],"full_cost":v[3],"retained_cost":v[4],
                    "peak_after":v[5],"retained_after":v[6],"complete_record_set":complete,
                    "prefix_winning_module_index":(complete && v[7]!=usize::MAX).then_some(v[7]),
                    "scope":"successful sorted prefix step; not a completed pass peak"}));
            } else if item.authority_request {
                if strict { assert_eq!(v[0].checked_mul(v[1]), Some(v[2])); }
                write_row(output, serde_json::json!({"schema":"semaprax.authority-index-request.v1",
                    "application":application,"ordinal":ordinal,"caller_path":path,"indexed_declarations":v[0],"btree_node_upper":v[1],"requested_bytes":v[2],
                    "enclosing_remaining":(v[4]!=0).then_some(v[3]),"reconciliation_error":v[0].checked_mul(v[1])!=Some(v[2])}));
            } else {
                if strict {
                    if item.legacy_components {
                        assert_eq!(legacy_hir_sum(&v).and_then(|n| n.checked_add(v[0])), Some(v[13]), "complete original legacy formula");
                    } else {
                        assert_eq!(hir_sum(&v), Some(v[18]), "complete HIR bound includes prelude");
                        assert_eq!(v[0].checked_add(v[18]), Some(v[13]), "complete raw clone plus HIR bound");
                    }
                }
                write_row(output, serde_json::json!({"schema":"semaprax.forecast-component.v1",
                    "application":application,"ordinal":ordinal,"layout_mode":item.mode,"execution_peak_pass":item.pass.map(|p|p.0),"pass_id":item.pass.map(|p|p.1),
                    "module_index":item.module,"path":path,"include_imports":(!item.legacy_components).then_some(item.include_imports),"legacy_component_fields":item.legacy_components,
                    "mode6_role":if !item.legacy_components && item.pass.is_some_and(|p|p.0==6) {Some(if item.include_imports {"full"} else {"retained"})} else {None},
                    "retention":if item.semantic {"Semantic"} else {"Parsed"},"reconciliation_error":!record_reconciles(&item),"complete_record_set":complete,
                    "reconciliation_status":if !record_reconciles(&item) {"invalid"} else if item.legacy_components {"legacy_formula_only_new_fields_unknown"} else {"complete_two_equations"},
                    "raw_total":v[0],"raw_strings":v[1],"raw_exact_expressions":v[2],"raw_literal_absence_bytes":v[3],
                    "runtime_total":v[4],"runtime_strings":v[5],"runtime_exact_expressions":v[6],"structure_input":v[7],"string_input":v[8],
                    "fixed_upper_before_layout_adjustment":v[9],"fixed_upper_after_layout_adjustment":v[10],"identity_occurrence_upper":v[11],"identity_carrier_upper":v[12],
                    "prelude_hir_upper":(!item.legacy_components).then_some(v[17]),"hir_upper":(!item.legacy_components).then_some(v[18]),"retained_clone_and_hir":v[13],"generic_instances":v[14],"transient_import_clone":v[15],
                    "original_mode7_fixed_upper":(item.mode==7 && v[16]!=usize::MAX).then_some(v[16]),"original_formula_overflow":(item.mode==7).then_some(v[16]==usize::MAX)}));
            }
        }
    });
}
/// Legacy normal-return observer keeps its strict reconciliation assertions.
/// Catalog uses the separate lifetime guard so fixture Drop precedes reporting.
pub(crate) fn observe<T>(application: &str, operation: impl FnOnce() -> T) -> T {
    prepare();
    ACTIVE.with(|active| active.set(true));
    let guard = Capture;
    let result = operation();
    drop(guard);
    write_observation(&mut std::io::stderr().lock(), application, true);
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
            assert_eq!((component.mode, component.pass), (4, Some((5, 0))));
            let peak = state.entries[1].unwrap();
            assert!(peak.global);
            assert_eq!(peak.mode, 5);
            assert_eq!(&peak.values[..4], &[1, 11, 7, 4]);
            let incomplete = state.entries[3].unwrap();
            assert!(incomplete.global);
            assert_eq!(incomplete.mode, 6);
            assert_eq!(incomplete.pass, Some((6, 1)));
            assert_eq!(incomplete.values, [0; VALUES]);
            assert!(state.current_pass.is_none());
        });
        observe("no-forecast", || ());
        STATE.with(|state| {
            let state = state.borrow();
            assert_eq!(state.count, 0);
            assert_eq!(state.forecasts, [0; 8]);
        });
    }
    fn components() -> [usize; VALUES] {
        let mut values = [0; VALUES];
        values[0] = 7;
        values[10] = 11;
        values[11] = 13;
        values[12] = 17;
        values[17] = 19;
        values[18] = 60;
        values[13] = 67;
        values
    }
    #[test]
    fn actual_mode6_steps_include_prelude_full_retained_and_winner_without_collection_allocations()
    {
        let report = Report::prepare("mode6-components");
        let ((), metrics) = crate::test_allocation_observer::measure(|| {
            let _capture = report.capture();
            let mut pass = begin_pass(6);
            pass.module(0);
            record_components("first", 5, false, true, components());
            record_components("first", 5, false, false, components());
            pass.step("first", 0, 0, (0, 100, 30), (100, 30));
            pass.module(1);
            record_components("second", 5, false, true, components());
            record_components("second", 5, false, false, components());
            pass.step("second", 1, 1, (30, 120, 20), (150, 50));
            pass.complete(150, 50, 0);
        });
        assert!(!metrics.invalid);
        assert_eq!((metrics.allocated, metrics.freed, metrics.live), (0, 0, 0));
        assert_eq!(hir_sum(&components()), Some(60));
        assert!(reconciles(&components()));
        let mut missing_prelude = components();
        missing_prelude[17] = 0;
        assert!(
            !reconciles(&missing_prelude),
            "the old omitted summand cannot pass"
        );
        let mut bad_clone = components();
        bad_clone[13] -= 1;
        assert!(!reconciles(&bad_clone));
        let mut bytes = Vec::new();
        write_observation(&mut bytes, "mode6-components", true);
        let rows = bytes
            .split(|b| *b == b'\n')
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_slice::<serde_json::Value>(line).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(rows[0]["complete_records"], true);
        assert_eq!(rows[1]["include_imports"], true);
        assert_eq!(rows[1]["mode6_role"], "full");
        assert_eq!(rows[2]["include_imports"], false);
        assert_eq!(rows[2]["mode6_role"], "retained");
        assert_eq!(rows[1]["prelude_hir_upper"], 19);
        assert_eq!(rows[1]["hir_upper"], 60);
        assert_eq!(rows[1]["pass_id"], rows[5]["pass_id"]);
        assert_eq!(rows[6]["sorted_step"], 1);
        assert_eq!(rows[6]["retained_before"], 30);
        assert_eq!(rows[6]["peak_after"], 150);
        assert_eq!(rows[6]["prefix_winning_module_index"], 1);
        assert_eq!(rows[7]["global_peak"], 150);
        assert_eq!(rows[7]["winning_module_index"], 1);
        drop(report);
        // Historical producers retain their exact17-field API and original
        // reconciliation. Missing current fields are unknown, never zero.
        let legacy_report = Report::prepare("legacy-components");
        {
            let _capture = legacy_report.capture();
            let mut values = [0; 17];
            values[0] = 7;
            values[10] = 11;
            values[11] = 13;
            values[12] = 17;
            values[13] = 48;
            record("historical", 5, false, values);
        }
        let mut bytes = Vec::new();
        write_observation(&mut bytes, "legacy-components", true);
        let rows = bytes
            .split(|b| *b == b'\n')
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_slice::<serde_json::Value>(line).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(rows[0]["legacy_component_records"], 1);
        assert_eq!(rows[0]["complete_records"], false);
        assert!(rows[1]["include_imports"].is_null());
        assert!(rows[1]["prelude_hir_upper"].is_null() && rows[1]["hir_upper"].is_null());
        assert_eq!(
            rows[1]["reconciliation_status"],
            "legacy_formula_only_new_fields_unknown"
        );
        assert_eq!(rows[1]["retained_clone_and_hir"], 48);
        drop(legacy_report);
    }
    #[test]
    fn lost_or_invalid_component_records_make_complete_peak_attribution_unavailable() {
        let report = Report::prepare("lost-components");
        let oversized = "x".repeat(PATH + 1);
        {
            let _capture = report.capture();
            let mut pass = begin_pass(6);
            for _ in 0..SLOTS {
                record_components("module", 5, false, true, components());
            }
            record_components(&oversized, 5, false, false, components());
            let mut invalid = components();
            invalid[18] -= 1;
            record_components("invalid", 5, false, false, invalid);
            pass.complete(150, 50, 0);
        }
        STATE.with(|state| {
            let state = state.borrow();
            assert_eq!(state.lost_records(), 3);
        });
        let mut bytes = Vec::new();
        write_observation(&mut bytes, "lost-components", false);
        let rows = bytes
            .split(|b| *b == b'\n')
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_slice::<serde_json::Value>(line).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(rows[0]["lost_records"], 3);
        assert_eq!(rows[0]["reconciliation_errors"], 1);
        assert_eq!(rows[0]["complete_records"], false);
        let last = rows.last().unwrap();
        assert!(last["global_peak"].is_null() && last["retained_sum"].is_null());
        assert!(last["winning_module_index"].is_null());
        struct Closed;
        impl Write for Closed {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Err(std::io::ErrorKind::BrokenPipe.into())
            }
        }
        write_observation(&mut Closed, "closed-report-stream", false);
        drop(report);
    }
    #[test]
    fn catalog_style_capture_excludes_rebuild_and_preserves_original_unwind_after_fixture_drop() {
        struct Fixture<'a>(&'a Cell<bool>);
        impl Drop for Fixture<'_> {
            fn drop(&mut self) {
                self.0.set(!ACTIVE.with(Cell::get));
            }
        }
        for inside_capture in [false, true] {
            let fixture_dropped_inactive = Cell::new(false);
            let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> () {
                let report = Report::prepare("original-owner-unwind");
                let _fixture = Fixture(&fixture_dropped_inactive);
                {
                    let _capture = report.capture();
                    let _pass = begin_pass(6);
                    record_components("original", 5, false, true, components());
                    if inside_capture {
                        panic!("original owner failure");
                    }
                }
                record_components("failure-only-rebuild", 5, false, true, components());
                panic!("original owner failure");
            }))
            .unwrap_err();
            assert_eq!(
                failure.downcast_ref::<&str>(),
                Some(&"original owner failure")
            );
            assert!(fixture_dropped_inactive.get());
            assert!(!ACTIVE.with(Cell::get));
            STATE.with(|state| {
                let state = state.borrow();
                assert_eq!(
                    state.count, 2,
                    "only original component and incomplete pass"
                );
                assert!(state.current_pass.is_none());
            });
        }
    }
}
