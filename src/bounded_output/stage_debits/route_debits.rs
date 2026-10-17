//! Private diagnostic attribution, enabled only by the existing codec witness.
//! Fixed TLS never becomes a payer, allowance, source key or compiler limit.
use std::cell::{Cell, RefCell};
const ROWS: usize = 1024;
const ID_BYTES: usize = 256;
const ROUTES: usize = 3;
const PHASES: usize = 11;
#[derive(Clone, Copy, Debug)]
pub(crate) enum Route {
    Production,
    UnoptedCrosscheck,
    OptedOracle,
}
#[derive(Clone, Copy, Debug)]
pub(crate) enum Phase {
    OtherReplay,
    StructuralAndState,
    EnumeratedHIR,
    FlatPLAN,
    HistoryPLAN,
    PathComparison,
    FactoredHIR,
    HIRAutomaton,
    PLANAutomaton,
    CanonicalHIR,
    CanonicalPLAN,
}
#[derive(Clone, Copy, Debug)]
struct Coordinate {
    row: Option<usize>,
    ordinal: usize,
    instance: bool,
    route: Route,
    phase: Phase,
}
impl Coordinate {
    const EMPTY: Self = Self {
        row: None,
        ordinal: 0,
        instance: false,
        route: Route::Production,
        phase: Phase::OtherReplay,
    };
}
#[derive(Clone, Copy)]
struct Row {
    ordinal: usize,
    instance: bool,
    id: [u8; ID_BYTES],
    len: usize,
    truncated: bool,
    bytes: [[usize; PHASES]; ROUTES],
    calls: [[usize; PHASES]; ROUTES],
}
impl Row {
    const EMPTY: Self = Self {
        ordinal: 0,
        instance: false,
        id: [0; ID_BYTES],
        len: 0,
        truncated: false,
        bytes: [[0; PHASES]; ROUTES],
        calls: [[0; PHASES]; ROUTES],
    };
}
struct Packet {
    owner: usize,
    prebound: usize,
    mode: u8,
    consumed: usize,
    expected_hir: usize,
    hir: usize,
    non_cleanup_hir: usize,
    unknown_cleanup: usize,
    overflow_cleanup: usize,
    complete: bool,
    used: usize,
    coordinate: Coordinate,
    first: Option<Coordinate>,
    first_is_hir_cleanup: bool,
    rows: [Row; ROWS],
}
impl Packet {
    const EMPTY: Self = Self {
        owner: 0,
        prebound: 0,
        mode: 0,
        consumed: 0,
        expected_hir: 0,
        hir: 0,
        non_cleanup_hir: 0,
        unknown_cleanup: 0,
        overflow_cleanup: 0,
        complete: true,
        used: 0,
        coordinate: Coordinate::EMPTY,
        first: None,
        first_is_hir_cleanup: false,
        rows: [Row::EMPTY; ROWS],
    };

    // Rows are overwritten when registered; clearing the header avoids a
    // whole-packet temporary on the instrumented thread's stack.
    fn clear_header(&mut self) {
        self.owner = 0;
        self.prebound = 0;
        self.mode = 0;
        self.consumed = 0;
        self.expected_hir = 0;
        self.hir = 0;
        self.non_cleanup_hir = 0;
        self.unknown_cleanup = 0;
        self.overflow_cleanup = 0;
        self.complete = true;
        self.used = 0;
        self.coordinate = Coordinate::EMPTY;
        self.first = None;
        self.first_is_hir_cleanup = false;
    }

    // Copy directly between TLS packets, retaining only the populated rows.
    fn copy_from(&mut self, source: &Self) {
        self.owner = source.owner;
        self.prebound = source.prebound;
        self.mode = source.mode;
        self.consumed = source.consumed;
        self.expected_hir = source.expected_hir;
        self.hir = source.hir;
        self.non_cleanup_hir = source.non_cleanup_hir;
        self.unknown_cleanup = source.unknown_cleanup;
        self.overflow_cleanup = source.overflow_cleanup;
        self.complete = source.complete;
        self.used = source.used;
        self.coordinate = source.coordinate;
        self.first = source.first;
        self.first_is_hir_cleanup = source.first_is_hir_cleanup;
        self.rows[..source.used].copy_from_slice(&source.rows[..source.used]);
    }
}
struct State {
    current: Packet,
    last: Packet,
    maximum: Packet,
    have_last: bool,
    have_max: bool,
}
thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(false) };
    static LOST: Cell<bool> = const { Cell::new(false) };
    static STATE: RefCell<State> = const { RefCell::new(State { current: Packet::EMPTY,
        last: Packet::EMPTY, maximum: Packet::EMPTY, have_last: false, have_max: false }) };
}
// No panic on an instrumentation borrow conflict, including during unwind.
fn update(body: impl FnOnce(&mut State)) {
    if !ENABLED.with(Cell::get) {
        return;
    }
    STATE.with(|state| match state.try_borrow_mut() {
        Ok(mut state) => body(&mut state),
        Err(_) => LOST.with(|lost| lost.set(true)),
    });
}
fn add(slot: &mut usize, bytes: usize, complete: &mut bool) {
    match slot.checked_add(bytes) {
        Some(sum) => *slot = sum,
        None => {
            *slot = usize::MAX;
            *complete = false;
        }
    }
}
pub(crate) struct Observation {
    previous: bool,
}
pub(crate) fn observe() -> Observation {
    let previous = ENABLED.with(|enabled| enabled.replace(true));
    if !previous {
        LOST.with(|lost| lost.set(false));
        update(|state| {
            state.current.clear_header();
            state.have_last = false;
            state.have_max = false;
        });
    } else {
        LOST.with(|lost| lost.set(true));
    }
    Observation { previous }
}
impl Drop for Observation {
    fn drop(&mut self) {
        ENABLED.with(|enabled| enabled.set(self.previous));
    }
}
pub(super) fn start(owner: usize, prebound: usize, mode: u8) {
    update(|state| {
        if state.current.owner != 0 {
            // A nested core remains a separate exact ledger. Do not overwrite
            // the outer packet or attribute its child's debit to the parent.
            state.current.complete = false;
            return;
        }
        state.current.clear_header();
        state.current.owner = owner;
        state.current.prebound = prebound;
        state.current.mode = mode;
    });
}
pub(super) fn debit(owner: usize, bytes: usize, hir: bool, cleanup: bool) {
    update(|state| {
        let packet = &mut state.current;
        if packet.owner != owner || owner == 0 || !hir {
            return;
        }
        add(&mut packet.hir, bytes, &mut packet.complete);
        if !cleanup {
            add(&mut packet.non_cleanup_hir, bytes, &mut packet.complete);
            return;
        }
        match packet.coordinate.row {
            Some(index) if index < packet.used => {
                let row = &mut packet.rows[index];
                let route = packet.coordinate.route as usize;
                let phase = packet.coordinate.phase as usize;
                add(&mut row.bytes[route][phase], bytes, &mut packet.complete);
                add(&mut row.calls[route][phase], 1, &mut packet.complete);
            }
            Some(_) => add(&mut packet.overflow_cleanup, bytes, &mut packet.complete),
            None => add(&mut packet.unknown_cleanup, bytes, &mut packet.complete),
        }
    });
}
pub(super) fn refusal(owner: usize, hir_cleanup: bool) {
    update(|state| {
        if state.current.owner == owner && owner != 0 && state.current.first.is_none() {
            state.current.first = Some(state.current.coordinate);
            state.current.first_is_hir_cleanup = hir_cleanup;
        }
    });
}
pub(super) fn finish(owner: usize, consumed: usize, expected_hir: usize) {
    update(|state| {
        if state.current.owner != owner || owner == 0 {
            return;
        }
        state.current.consumed = consumed;
        state.current.expected_hir = expected_hir;
        state.current.complete &= !LOST.with(Cell::get) && state.current.hir == expected_hir;
        state.last.copy_from(&state.current);
        state.have_last = true;
        if !state.have_max || consumed > state.maximum.consumed {
            state.maximum.copy_from(&state.current);
            state.have_max = true;
        }
        state.current.owner = 0;
    });
}
/// Stores only copied coordinates. No program, function or ledger reference
/// survives the live call; a raw address is only an opaque exact-owner number.
pub(crate) struct Scope {
    owner: usize,
    previous: Option<Coordinate>,
}
fn scope(body: impl FnOnce(&mut Packet)) -> Scope {
    let mut result = Scope {
        owner: 0,
        previous: None,
    };
    update(|state| {
        let packet = &mut state.current;
        if packet.owner != 0 {
            result.owner = packet.owner;
            result.previous = Some(packet.coordinate);
            body(packet);
        }
    });
    result
}
pub(crate) fn function(ordinal: usize, instance: bool, identity: &str) -> Scope {
    scope(|packet| {
        let index = packet.used;
        if index < ROWS {
            let row = &mut packet.rows[index];
            *row = Row::EMPTY;
            row.ordinal = ordinal;
            row.instance = instance;
            row.len = identity.len().min(ID_BYTES);
            row.truncated = identity.len() > ID_BYTES;
            row.id[..row.len].copy_from_slice(&identity.as_bytes()[..row.len]);
            packet.complete &= !row.truncated;
            packet.used += 1;
        } else {
            packet.complete = false;
        }
        packet.coordinate = Coordinate {
            row: Some(index),
            ordinal,
            instance,
            route: Route::Production,
            phase: Phase::StructuralAndState,
        };
    })
}
pub(crate) fn route(route: Route) -> Scope {
    scope(|packet| packet.coordinate.route = route)
}
pub(crate) fn phase(phase: Phase) -> Scope {
    scope(|packet| packet.coordinate.phase = phase)
}
impl Drop for Scope {
    fn drop(&mut self) {
        if let Some(previous) = self.previous {
            update(|state| {
                if state.current.owner == self.owner {
                    state.current.coordinate = previous;
                }
            });
        }
    }
}
/// Called only after catch_unwind returned and the ordinary core trace settled.
/// This never runs from debit, a compiler phase, a scope Drop or live measure.
pub(crate) fn print_settled() {
    if !ENABLED.with(Cell::get) {
        return;
    }
    STATE.with(|state| {
        let Ok(state) = state.try_borrow() else { eprintln!("OPT808 routes incomplete: observer borrow conflict"); return; };
        if state.current.owner != 0 { eprintln!("OPT808 routes incomplete: core has not settled; no packet printed"); return; }
        for (name, available, packet) in [("maximum", state.have_max, &state.maximum), ("final", state.have_last, &state.last)] {
            if !available { eprintln!("OPT808 routes {name}: no settled core"); continue; }
            let mut classified = 0usize;
            for row in &packet.rows[..packet.used] {
                for route in row.bytes { for bytes in route { classified = classified.saturating_add(bytes); } }
            }
            let reconciled = classified.checked_add(packet.unknown_cleanup)
                .and_then(|value| value.checked_add(packet.overflow_cleanup))
                .and_then(|value| value.checked_add(packet.non_cleanup_hir));
            eprintln!("OPT808 routes {name}: complete={} owner={} mode={} prebound={} consumed={} HIR={} expected_HIR={} classified_cleanup={} unknown_cleanup={} overflow_cleanup={} non_cleanup_HIR={} reconciled={reconciled:?} first={:?} first_is_HIR_cleanup={}",
                packet.complete && !LOST.with(Cell::get) && reconciled == Some(packet.expected_hir),
                packet.owner, packet.mode, packet.prebound, packet.consumed, packet.hir, packet.expected_hir,
                classified, packet.unknown_cleanup, packet.overflow_cleanup, packet.non_cleanup_hir,
                packet.first.map(|at| (at.ordinal, at.instance, at.row, at.route, at.phase)), packet.first_is_hir_cleanup);
            for (index, row) in packet.rows[..packet.used].iter().enumerate() {
                eprintln!("OPT808 routes {name} function row={index} ordinal={} instance={} identity={:?} truncated={}",
                    row.ordinal, row.instance, std::str::from_utf8(&row.id[..row.len]), row.truncated);
                if std::str::from_utf8(&row.id[..row.len]).is_err() {
                    eprintln!("OPT808 routes {name} row={index} bounded identity bytes={:?}", &row.id[..row.len]);
                }
                for (route_index, route) in [Route::Production, Route::UnoptedCrosscheck, Route::OptedOracle].into_iter().enumerate() {
                    for (phase_index, phase) in [Phase::OtherReplay, Phase::StructuralAndState, Phase::EnumeratedHIR, Phase::FlatPLAN,
                        Phase::HistoryPLAN, Phase::PathComparison, Phase::FactoredHIR, Phase::HIRAutomaton, Phase::PLANAutomaton,
                        Phase::CanonicalHIR, Phase::CanonicalPLAN].into_iter().enumerate() {
                        if row.calls[route_index][phase_index] != 0 {
                            eprintln!("OPT808 routes {name} row={index} route={route:?} phase={phase:?} committed={} calls={}",
                                row.bytes[route_index][phase_index], row.calls[route_index][phase_index]);
                        }
                    }
                }
            }
        }
    });
}

#[test]
fn child_replay_operations_keep_route_cells_and_refusal_attribution_reconciled() {
    use crate::bounded_output::{reserve_active_required, with_limit_usage_refusal, with_reservation_stage, ReservationStage};
    use super::Operation;
    let _observation = observe();
    let (((), metrics), refused, used, first) = with_limit_usage_refusal(64, || {
        crate::test_allocation_observer::measure(|| {
            let _trace = super::start(0, 0, 6);
            with_reservation_stage(ReservationStage::HirValidation, || {
                let _function = function(0, false, "route-child");
                let _phase = phase(Phase::EnumeratedHIR);
                for operation in [Operation::CleanupReplay, Operation::ReplayStructure,
                    Operation::ReplayExpressionFacts, Operation::ReplayPathStates,
                    Operation::ReplayHirSkeleton, Operation::ReplayPlanSkeleton,
                    Operation::ReplayFactored, Operation::ReplayFactoredConstruction,
                    Operation::ReplayFactoredCanonical] {
                    let _operation = super::operation_guard(operation);
                    assert!(reserve_active_required(1));
                }
                {
                    let _operation = super::operation_guard(Operation::CoreValidation);
                    assert!(reserve_active_required(2));
                }
                let _operation = super::operation_guard(Operation::ReplayPlanSkeleton);
                assert!(!reserve_active_required(64));
            });
        })
    });
    assert!(refused && used == 11);
    let first = first.unwrap();
    assert_eq!((first.requested, first.remaining, first.floor), (64, 53, 0));
    assert!(!metrics.invalid);
    assert_eq!((metrics.allocations, metrics.allocated, metrics.live), (0, 0, 0));
    STATE.with(|state| {
        let state = state.borrow();
        assert!(state.have_last && state.have_max);
        let packet = &state.last;
        assert!(packet.complete && packet.first_is_hir_cleanup);
        assert_eq!((packet.hir, packet.expected_hir, packet.non_cleanup_hir), (11, 11, 2));
        assert_eq!((packet.unknown_cleanup, packet.overflow_cleanup, packet.used), (0, 0, 1));
        assert_eq!(packet.rows[0].bytes[Route::Production as usize][Phase::EnumeratedHIR as usize], 9);
        assert_eq!(packet.rows[0].calls[Route::Production as usize][Phase::EnumeratedHIR as usize], 9);
        assert_eq!(packet.first.unwrap().row, Some(0));
    });
}
