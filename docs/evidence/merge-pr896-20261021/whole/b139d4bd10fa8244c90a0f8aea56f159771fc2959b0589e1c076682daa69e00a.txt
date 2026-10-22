//! Fixed diagnostic aggregate only: no allocation, authority, payer or gate input.
use super::*;
use std::cell::RefCell;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::cleanup_plan::replay::factored::canonical_shared) struct Cell {
    pub attempts: usize,
    pub requested: usize,
    pub acquired: usize,
    pub primitive_banks: usize,
    pub refused: usize,
    pub selection_requested: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::cleanup_plan::replay::factored::canonical_shared) struct Report {
    // [old/current][other/HIR/PLAN][general/unit]
    pub cells: [[[Cell; 2]; 3]; 2],
    pub complete: bool,
    pub function: usize,
    pub budget: usize,
    pub invocation: usize,
    pub foreign_observation: bool,
}
#[derive(Clone, Copy)]
struct Pending {
    old: usize,
    phase: usize,
    unit: usize,
    bytes: usize,
    banks: usize,
}
struct State {
    enabled: bool,
    phase: usize,
    serial: usize,
    report: Report,
    pending: Option<Pending>,
}
impl State {
    const fn new() -> Self {
        Self {
            enabled: false,
            phase: 0,
            serial: 0,
            report: Report {
                cells: [[[Cell {
                    attempts: 0,
                    requested: 0,
                    acquired: 0,
                    primitive_banks: 0,
                    refused: 0,
                    selection_requested: 0,
                }; 2]; 3]; 2],
                complete: true,
                function: 0,
                budget: 0,
                invocation: 0,
                foreign_observation: false,
            },
            pending: None,
        }
    }
}
thread_local! { static STATE: RefCell<State> = const { RefCell::new(State::new()) }; }
fn add(slot: &mut usize, value: usize, complete: &mut bool) {
    match slot.checked_add(value) {
        Some(n) => *slot = n,
        None => {
            *slot = usize::MAX;
            *complete = false;
        }
    }
}
pub(in crate::cleanup_plan::replay::factored::canonical_shared) struct Observation {
    previous: bool,
}
pub(in crate::cleanup_plan::replay::factored::canonical_shared) fn observe(function: &ResolvedFunction, budget: &ReplayBudget) -> Observation {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let previous = s.enabled;
        assert!(!previous, "diagnostic observations are not nested");
        let mut complete = true;
        let mut serial = s.serial;
        add(&mut serial, 1, &mut complete);
        s.serial = serial;
        s.report = State::new().report;
        s.report.complete = complete;
        s.report.function = std::ptr::from_ref(function) as usize;
        s.report.budget = std::ptr::from_ref(budget) as usize;
        s.report.invocation = serial;
        s.enabled = true;
        s.pending = None;
        Observation { previous }
    })
}
pub(in crate::cleanup_plan::replay::factored::canonical_shared) fn take() -> Report {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if let Some(p) = s.pending.take() {
            let mut complete = s.report.complete;
            add(
                &mut s.report.cells[p.old][p.phase][p.unit].refused,
                1,
                &mut complete,
            );
            s.report.complete = complete;
        }
        s.report
    })
}
impl Drop for Observation {
    fn drop(&mut self) {
        STATE.with(|s| s.borrow_mut().enabled = self.previous);
    }
}
pub(in crate::cleanup_plan::replay::factored) struct Phase {
    previous: usize,
    application_previous: usize,
}
pub(in crate::cleanup_plan::replay::factored) fn phase(hir: bool) -> Phase {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let previous = s.phase;
        s.phase = if hir { 1 } else { 2 };
        let application_previous = APPLICATION.with(|slot| {
            let mut state = slot.get();
            let prior = state.phase;
            state.phase = if hir { 1 } else { 2 };
            slot.set(state);
            prior
        });
        Phase {
            previous,
            application_previous,
        }
    })
}
impl Drop for Phase {
    fn drop(&mut self) {
        STATE.with(|s| s.borrow_mut().phase = self.previous);
        let _ = APPLICATION.try_with(|slot| {
            let mut state = slot.get();
            state.phase = self.application_previous;
            slot.set(state);
        });
    }
}
pub(in crate::cleanup_plan::replay::factored::canonical_shared) fn request(
    unit: bool,
    old: bool,
    warm: bool,
    function: &ResolvedFunction,
    budget: &ReplayBudget,
) {
    application_request(unit, old, warm, function, budget);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if !s.enabled {
            return;
        }
        if s.report.function != std::ptr::from_ref(function) as usize
            || s.report.budget != std::ptr::from_ref(budget) as usize
        {
            s.report.foreign_observation = true;
            s.report.complete = false;
            return;
        }
        if s.pending.is_some() {
            s.report.complete = false;
            return;
        }
        let old = usize::from(!old);
        let phase = s.phase;
        let unit = usize::from(unit);
        let bytes = if old == 0 {
            ORIGINAL_RECEIVING
        } else if warm {
            WARM_RECEIVING
        } else {
            COLD_RECEIVING
        };
        let banks = usize::from(old == 0 || !warm);
        let mut complete = s.report.complete;
        let c = &mut s.report.cells[old][phase][unit];
        add(&mut c.attempts, 1, &mut complete);
        add(&mut c.requested, bytes, &mut complete);
        if old == 1 {
            add(&mut c.selection_requested, PER_USE_SELECTION, &mut complete);
        }
        s.report.complete = complete;
        s.pending = Some(Pending {
            old,
            phase,
            unit,
            bytes,
            banks,
        });
    });
}
pub(in crate::cleanup_plan::replay::factored::canonical_shared) fn acquired() {
    application_acquired();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if !s.enabled {
            return;
        }
        let Some(p) = s.pending.take() else {
            s.report.complete = false;
            return;
        };
        let mut complete = s.report.complete;
        let c = &mut s.report.cells[p.old][p.phase][p.unit];
        add(&mut c.acquired, p.bytes, &mut complete);
        add(&mut c.primitive_banks, p.banks, &mut complete);
        s.report.complete = complete;
    });
}
#[test]
fn fixed_successor_census_8769_rows_and_checked_overflow_are_diagnostic_only() {
    let mut count = 0;
    let mut complete = true;
    for _ in 0..8769 {
        add(&mut count, 1, &mut complete);
    }
    assert_eq!(count, 8769);
    assert!(complete);
    count = usize::MAX;
    add(&mut count, 1, &mut complete);
    assert_eq!(count, usize::MAX);
    assert!(!complete);
}

#[test]
fn all_twelve_census_cells_route_without_event_buffer_or_authority() {
    let program = crate::check("module c884; @id(\"main\") fn main()->i64{1}", "c884.spx").unwrap();
    let f = crate::hir::resolve(&program).unwrap().functions.remove(0);
    let b = ReplayBudget::unobserved();
    let guard = observe(&f, &b);
    // Diagnostic routing only: these calls do not reserve any real bytes and
    // their synthetic aggregate must never serve as an earned bill/fit claim.
    for phase_index in 0..3 {
        STATE.with(|s| s.borrow_mut().phase = phase_index);
        for old in [true, false] {
            for unit in [false, true] {
                for n in 0..8769 {
                    request(unit, old, n != 0, &f, &b);
                    acquired();
                }
            }
        }
    }
    let r = take();
    assert!(r.complete && !r.foreign_observation);
    assert_eq!(r.function, std::ptr::from_ref(&f) as usize);
    assert_eq!(r.budget, std::ptr::from_ref(&b) as usize);
    for old in 0..2 {
        for phase in 0..3 {
            for unit in 0..2 {
                let c = r.cells[old][phase][unit];
                let expected = if old == 0 {
                    8769 * ORIGINAL_RECEIVING
                } else {
                    COLD_RECEIVING + 8768 * WARM_RECEIVING
                };
                assert_eq!(
                    (c.attempts, c.requested, c.acquired, c.refused),
                    (8769, expected, expected, 0)
                );
                assert_eq!(c.primitive_banks, if old == 0 { 8769 } else { 1 });
            }
        }
    }
    drop(guard);
}

// Outer application diagnostic scope. The single-pair State above remains an
// independent hostile control. These copy-only counters never acquire a payer,
// borrow an account, allocate, choose a route, or enter production compilation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ApplicationCell {
    attempted_entries: usize,
    acquired_entries: usize,
    requested: usize,
    acquired: usize,
    attempted_banks: usize,
    acquired_banks: usize,
    refused_entries: usize,
}
#[derive(Clone, Copy, Debug)]
struct ApplicationReport {
    cells: [[[ApplicationCell; 2]; 3]; 2],
    started: [[usize; 3]; 2],
    finished: [[usize; 3]; 2],
    complete: bool,
    overflowed: bool,
    lost: bool,
    last_function: usize,
    last_budget: usize,
    last_serial: usize,
}
#[derive(Clone, Copy)]
struct InvocationIdentity {
    function: usize,
    budget: usize,
    serial: usize,
    old: usize,
    phase: usize,
}
#[derive(Clone, Copy)]
struct ApplicationState {
    enabled: bool,
    phase: usize,
    serial: usize,
    suppressed: usize,
    current: Option<InvocationIdentity>,
    pending: Option<Pending>,
    report: ApplicationReport,
}
impl ApplicationState {
    const EMPTY: Self = Self {
        enabled: false,
        phase: 0,
        serial: 0,
        suppressed: 0,
        current: None,
        pending: None,
        report: ApplicationReport {
            cells: [[[ApplicationCell {
                attempted_entries: 0,
                acquired_entries: 0,
                requested: 0,
                acquired: 0,
                attempted_banks: 0,
                acquired_banks: 0,
                refused_entries: 0,
            }; 2]; 3]; 2],
            started: [[0; 3]; 2],
            finished: [[0; 3]; 2],
            complete: true,
            overflowed: false,
            lost: false,
            last_function: 0,
            last_budget: 0,
            last_serial: 0,
        },
    };
    fn lost(&mut self) {
        self.report.lost = true;
        self.report.complete = false;
    }
    fn refuse_pending(&mut self) {
        if let Some(p) = self.pending.take() {
            application_add(
                &mut self.report.cells[p.old][p.phase][p.unit].refused_entries,
                1,
                &mut self.report.complete,
                &mut self.report.overflowed,
            );
        }
    }
}
thread_local! {
    static APPLICATION: std::cell::Cell<ApplicationState> = const {
        std::cell::Cell::new(ApplicationState::EMPTY)
    };
}
fn application_add(slot: &mut usize, n: usize, complete: &mut bool, overflowed: &mut bool) {
    match slot.checked_add(n) {
        Some(next) => *slot = next,
        None => {
            *slot = usize::MAX;
            *complete = false;
            *overflowed = true;
        }
    }
}
// First local in each genuine application owner: Drop/printing occurs after all
// later fixtures, allocation observers, replay ledgers and unwind cleanup.
pub(crate) struct Scope {
    previous: Option<ApplicationState>,
    owner: &'static str,
}
impl Scope {
    fn new(owner: &'static str) -> Self {
        let previous = APPLICATION.with(|slot| {
            let mut state = slot.get();
            if state.enabled {
                state.lost();
                slot.set(state);
                return None;
            }
            let previous = state;
            state = ApplicationState::EMPTY;
            state.enabled = true;
            state.serial = previous.serial;
            slot.set(state);
            Some(previous)
        });
        Self { previous, owner }
    }
    pub(crate) fn codec808() -> Self {
        Self::new("codec808-actual-preparation")
    }
    pub(crate) fn normalizer() -> Self {
        Self::new("catalog-normalizer-compact-original")
    }
}
impl Scope {
    // Report only after restoring the fixed counters. TLS teardown and closed
    // diagnostic streams must not replace the original owner Result or unwind.
    fn write_settled(&mut self, output: &mut impl std::io::Write) {
        let Some(mut previous) = self.previous.take() else {
            return;
        };
        let Ok(report) = APPLICATION.try_with(|slot| {
            let mut state = slot.get();
            if state.current.is_some() || state.pending.is_some() || state.suppressed != 0 {
                state.lost();
            }
            // A scope serial is diagnostic only; never reuse it as an authority.
            previous.serial = state.serial;
            slot.set(previous);
            state.report
        }) else {
            return;
        };
        let _ = writeln!(output, "OPT884 application-successor-census owner={} partial_prefix={} complete={} overflowed={} lost={} axes=old_current/other_HIR_PLAN/general_unit actual_attempted_and_acquired_not_unique_frequency=true report={report:?}",
            self.owner, std::thread::panicking(), report.complete, report.overflowed, report.lost);
        let _ = writeln!(output, "OPT884 application invocation binding diagnostic_only=true function={} budget={} serial={} started={:?} finished={:?}", report.last_function, report.last_budget, report.last_serial, report.started, report.finished);
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        let mut stderr = std::io::stderr().lock();
        self.write_settled(&mut stderr);
    }
}
pub(in crate::cleanup_plan::replay::factored::canonical_shared) struct Invocation {
    serial: Option<usize>,
    suppressed: bool,
}
pub(in crate::cleanup_plan::replay::factored::canonical_shared) fn invocation(
    old: bool,
    function: &ResolvedFunction,
    budget: &ReplayBudget,
) -> Invocation {
    APPLICATION.with(|slot| {
        let mut state = slot.get();
        if !state.enabled {
            return Invocation {
                serial: None,
                suppressed: false,
            };
        }
        if state.current.is_some() || state.suppressed != 0 {
            state.lost();
            match state.suppressed.checked_add(1) {
                Some(n) => state.suppressed = n,
                None => {
                    state.report.overflowed = true;
                }
            }
            slot.set(state);
            return Invocation {
                serial: None,
                suppressed: true,
            };
        }
        if state.pending.is_some() {
            state.lost();
            state.refuse_pending();
        }
        let Some(serial) = state.serial.checked_add(1) else {
            state.lost();
            state.report.overflowed = true;
            slot.set(state);
            return Invocation {
                serial: None,
                suppressed: false,
            };
        };
        state.serial = serial;
        let id = InvocationIdentity {
            function: std::ptr::from_ref(function) as usize,
            budget: std::ptr::from_ref(budget) as usize,
            serial,
            old: usize::from(!old),
            phase: state.phase,
        };
        state.current = Some(id);
        state.report.last_function = id.function;
        state.report.last_budget = id.budget;
        state.report.last_serial = serial;
        application_add(
            &mut state.report.started[id.old][id.phase],
            1,
            &mut state.report.complete,
            &mut state.report.overflowed,
        );
        slot.set(state);
        Invocation {
            serial: Some(serial),
            suppressed: false,
        }
    })
}
impl Drop for Invocation {
    fn drop(&mut self) {
        let _ = APPLICATION.try_with(|slot| {
            let mut state = slot.get();
            if self.suppressed {
                if state.suppressed == 0 {
                    state.lost();
                } else {
                    state.suppressed -= 1;
                }
            } else if let Some(serial) = self.serial {
                match state.current {
                    Some(id) if id.serial == serial => {
                        state.refuse_pending();
                        application_add(
                            &mut state.report.finished[id.old][id.phase],
                            1,
                            &mut state.report.complete,
                            &mut state.report.overflowed,
                        );
                        state.current = None;
                    }
                    _ => state.lost(),
                }
            }
            slot.set(state);
        });
    }
}
fn application_request(
    unit: bool,
    old: bool,
    warm: bool,
    function: &ResolvedFunction,
    budget: &ReplayBudget,
) {
    APPLICATION.with(|slot| {
        let mut state = slot.get();
        if !state.enabled {
            return;
        }
        let old = usize::from(!old);
        let valid = state.current.is_some_and(|id| {
            id.old == old
                && id.serial == state.serial
                && id.phase == state.phase
                && id.function == std::ptr::from_ref(function) as usize
                && id.budget == std::ptr::from_ref(budget) as usize
        });
        if !valid || state.suppressed != 0 || state.pending.is_some() {
            state.lost();
            slot.set(state);
            return;
        }
        let phase = state.phase;
        let unit = usize::from(unit);
        let bytes = if old == 0 {
            ORIGINAL_RECEIVING
        } else if warm {
            WARM_RECEIVING
        } else {
            COLD_RECEIVING
        };
        let banks = usize::from(old == 0 || !warm);
        let c = &mut state.report.cells[old][phase][unit];
        application_add(
            &mut c.attempted_entries,
            1,
            &mut state.report.complete,
            &mut state.report.overflowed,
        );
        application_add(
            &mut c.requested,
            bytes,
            &mut state.report.complete,
            &mut state.report.overflowed,
        );
        application_add(
            &mut c.attempted_banks,
            banks,
            &mut state.report.complete,
            &mut state.report.overflowed,
        );
        state.pending = Some(Pending {
            old,
            phase,
            unit,
            bytes,
            banks,
        });
        slot.set(state);
    });
}
fn application_acquired() {
    APPLICATION.with(|slot| {
        let mut state = slot.get();
        if !state.enabled {
            return;
        }
        if state.suppressed != 0 {
            state.lost();
            slot.set(state);
            return;
        }
        let Some(p) = state.pending.take() else {
            state.lost();
            slot.set(state);
            return;
        };
        if !state.current.is_some_and(|id| {
            id.serial == state.serial
                && id.old == p.old
                && id.phase == p.phase
                && state.phase == p.phase
        }) {
            state.lost();
            slot.set(state);
            return;
        }
        let c = &mut state.report.cells[p.old][p.phase][p.unit];
        application_add(
            &mut c.acquired_entries,
            1,
            &mut state.report.complete,
            &mut state.report.overflowed,
        );
        application_add(
            &mut c.acquired,
            p.bytes,
            &mut state.report.complete,
            &mut state.report.overflowed,
        );
        application_add(
            &mut c.acquired_banks,
            p.banks,
            &mut state.report.complete,
            &mut state.report.overflowed,
        );
        slot.set(state);
    });
}

#[test]
fn application_census_keeps_attempted_acquired_and_refused_pairs_separate_in_all_cells() {
    let program = crate::check("module c884; @id(\"main\") fn main()->i64{1}", "c884.spx").unwrap();
    let f = crate::hir::resolve(&program).unwrap().functions.remove(0);
    let b = ReplayBudget::unobserved();
    let scope = Scope::new("synthetic-routing-not-actual-reservations");
    for p in 0..3 {
        APPLICATION.with(|slot| {
            let mut s = slot.get();
            s.phase = p;
            slot.set(s);
        });
        for old in [true, false] {
            for unit in [false, true] {
                // An invocation ending before any bank must not create N/M.
                drop(invocation(old, &f, &b));
                let owner = invocation(old, &f, &b);
                application_request(unit, old, false, &f, &b);
                application_acquired();
                application_request(unit, old, true, &f, &b);
                drop(owner); // Real hook has no acquired event on refusal.
            }
        }
    }
    let r = APPLICATION.with(|slot| slot.get().report);
    assert!(r.complete && !r.lost && !r.overflowed);
    assert_eq!(r.started, r.finished);
    for old in 0..2 {
        for p in 0..3 {
            for unit in 0..2 {
                let c = r.cells[old][p][unit];
                assert_eq!(
                    (c.attempted_entries, c.acquired_entries, c.refused_entries),
                    (2, 1, 1)
                );
                assert_eq!(c.attempted_banks, if old == 0 { 2 } else { 1 });
                assert_eq!(c.acquired_banks, 1);
                assert_eq!(
                    c.acquired,
                    if old == 0 {
                        ORIGINAL_RECEIVING
                    } else {
                        COLD_RECEIVING
                    }
                );
            }
        }
    }
    drop(scope);
}
#[test]
fn application_census_nested_foreign_and_serial_overflow_are_incomplete_not_authority() {
    let program = crate::check("module c884; @id(\"main\") fn main()->i64{1}", "c884.spx").unwrap();
    let f = crate::hir::resolve(&program).unwrap().functions.remove(0);
    let foreign = f.clone();
    let b = ReplayBudget::unobserved();
    for attack in 0..4 {
        let scope = Scope::new("synthetic-hostile-not-actual-reservations");
        if attack == 0 {
            let nested = Scope::new("nested");
            drop(nested);
        } else if attack == 1 {
            let owner = invocation(false, &f, &b);
            application_request(true, false, false, &foreign, &b);
            drop(owner);
        } else if attack == 2 {
            let owner = invocation(false, &f, &b);
            let nested = invocation(false, &f, &b);
            application_request(true, false, false, &f, &b);
            drop(nested);
            drop(owner);
        } else {
            APPLICATION.with(|slot| {
                let mut s = slot.get();
                s.serial = usize::MAX;
                slot.set(s);
            });
            drop(invocation(false, &f, &b));
        }
        let r = APPLICATION.with(|slot| slot.get().report);
        assert!(!r.complete && r.lost);
        assert_eq!(r.cells, [[[ApplicationCell::default(); 2]; 3]; 2]);
        if attack == 3 {
            APPLICATION.with(|slot| {
                let mut s = slot.get();
                s.serial = scope.previous.as_ref().unwrap().serial;
                slot.set(s);
            });
        }
        drop(scope);
    }
}
#[test]
fn application_census_counter_overflow_invalidates_fixed_report() {
    let program = crate::check("module c884; @id(\"main\") fn main()->i64{1}", "c884.spx").unwrap();
    let f = crate::hir::resolve(&program).unwrap().functions.remove(0);
    let b = ReplayBudget::unobserved();
    let scope = Scope::new("synthetic-overflow-not-actual-reservations");
    let owner = invocation(false, &f, &b);
    application_request(true, false, false, &f, &b);
    APPLICATION.with(|slot| {
        let mut s = slot.get();
        s.report.cells[1][0][1].acquired_entries = usize::MAX;
        slot.set(s);
    });
    application_acquired();
    drop(owner);
    let r = APPLICATION.with(|slot| slot.get().report);
    assert!(!r.complete && r.overflowed);
    assert_eq!(r.cells[1][0][1].acquired_entries, usize::MAX);
    drop(scope);
}

#[test]
fn application_closed_report_stream_preserves_original_unwind_after_fixture_drop() {
    use std::cell::Cell;
    struct Closed<'a>(&'a Cell<usize>);
    impl std::io::Write for Closed<'_> {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            self.0.set(self.0.get() + 1);
            Err(std::io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }
    }
    struct Fixture<'a>(&'a Cell<bool>);
    impl Drop for Fixture<'_> {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    struct ClosedReport<'a> {
        scope: Scope,
        fixture_dropped: &'a Cell<bool>,
        after_fixture: &'a Cell<bool>,
        writes: &'a Cell<usize>,
    }
    impl Drop for ClosedReport<'_> {
        fn drop(&mut self) {
            self.after_fixture.set(self.fixture_dropped.get());
            self.scope.write_settled(&mut Closed(self.writes));
        }
    }
    for unwind in [false, true] {
        let fixture_dropped = Cell::new(false);
        let after_fixture = Cell::new(false);
        let writes = Cell::new(0);
        // Synthetic ownership only: no wrapper intercepts an actual producer.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _report = ClosedReport {
                scope: Scope::new("synthetic-closed-report-stream"),
                fixture_dropped: &fixture_dropped,
                after_fixture: &after_fixture,
                writes: &writes,
            };
            let _fixture = Fixture(&fixture_dropped);
            if unwind {
                panic!("original application owner failure");
            }
        }));
        if unwind {
            assert_eq!(
                result.unwrap_err().downcast_ref::<&str>(),
                Some(&"original application owner failure")
            );
        } else {
            assert!(result.is_ok());
        }
        assert!(fixture_dropped.get() && after_fixture.get());
        assert_eq!(writes.get(), 2);
        assert!(!APPLICATION.with(|slot| slot.get().enabled));
    }
}

// OPT889 diagnostic adapter; original observers and all old bodies stay exact.
pub(in crate::cleanup_plan::replay::factored::canonical_shared) fn request_inline(
    unit: bool,
    warm: bool,
    function: &ResolvedFunction,
    budget: &ReplayBudget,
    bytes: usize,
) {
    let old = false;
    application_request_inline(unit, old, warm, function, budget, bytes);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if !s.enabled {
            return;
        }
        if s.report.function != std::ptr::from_ref(function) as usize
            || s.report.budget != std::ptr::from_ref(budget) as usize
        {
            s.report.foreign_observation = true;
            s.report.complete = false;
            return;
        }
        if s.pending.is_some() {
            s.report.complete = false;
            return;
        }
        let old = usize::from(!old);
        let phase = s.phase;
        let unit = usize::from(unit);
        let banks = usize::from(old == 0 || !warm);
        let mut complete = s.report.complete;
        let c = &mut s.report.cells[old][phase][unit];
        add(&mut c.attempts, 1, &mut complete);
        add(&mut c.requested, bytes, &mut complete);
        if old == 1 {
            add(&mut c.selection_requested, PER_USE_SELECTION, &mut complete);
        }
        s.report.complete = complete;
        s.pending = Some(Pending {
            old,
            phase,
            unit,
            bytes,
            banks,
        });
    });
}
fn application_request_inline(
    unit: bool,
    old: bool,
    warm: bool,
    function: &ResolvedFunction,
    budget: &ReplayBudget,
    bytes: usize,
) {
    APPLICATION.with(|slot| {
        let mut state = slot.get();
        if !state.enabled {
            return;
        }
        let old = usize::from(!old);
        let valid = state.current.is_some_and(|id| {
            id.old == old
                && id.serial == state.serial
                && id.phase == state.phase
                && id.function == std::ptr::from_ref(function) as usize
                && id.budget == std::ptr::from_ref(budget) as usize
        });
        if !valid || state.suppressed != 0 || state.pending.is_some() {
            state.lost();
            slot.set(state);
            return;
        }
        let phase = state.phase;
        let unit = usize::from(unit);
        let banks = usize::from(old == 0 || !warm);
        let c = &mut state.report.cells[old][phase][unit];
        application_add(
            &mut c.attempted_entries,
            1,
            &mut state.report.complete,
            &mut state.report.overflowed,
        );
        application_add(
            &mut c.requested,
            bytes,
            &mut state.report.complete,
            &mut state.report.overflowed,
        );
        application_add(
            &mut c.attempted_banks,
            banks,
            &mut state.report.complete,
            &mut state.report.overflowed,
        );
        state.pending = Some(Pending {
            old,
            phase,
            unit,
            bytes,
            banks,
        });
        slot.set(state);
    });
}
