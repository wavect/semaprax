//! Private diagnostic attribution, enabled only by the existing codec witness.
//! Fixed-capacity backing is prepared before observation; it grants no compiler credit.
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
// Fixed aggregate census: reservation calls, never physical allocations. No
// per-event buffer, no panic at4096/8769 events, no observer input to admission.
const COMPARISON_PHASES: usize = 13;
#[derive(Clone, Copy)]
struct ComparisonCell {
    attempts: usize,
    requested: usize,
    acquired: usize,
    refused: usize,
}
impl ComparisonCell {
    const EMPTY: Self = Self {
        attempts: 0,
        requested: 0,
        acquired: 0,
        refused: 0,
    };
}
fn comparison_phase(phase: &str) -> Option<usize> {
    match phase {
        "ordinary plan history comparator controls"
        | "ordinary plan retained comparison owner"
        | "ordinary plan reachable cursor primitive frames" => Some(0),
        "ordinary plan history order call" => Some(1),
        "ordinary plan history sift call" => Some(2),
        "ordinary plan history comparison receivers" => Some(3),
        "ordinary plan history sort receivers" => Some(4),
        "ordinary plan history left cursor prepare" => Some(5),
        "ordinary plan history right cursor prepare" => Some(6),
        "ordinary plan history HIR cursor prepare" => Some(7),
        "ordinary plan history paired cursor next" => Some(8),
        "ordinary plan history HIR cursor next" => Some(9),
        "ordinary plan retained drain swap receivers" => Some(10),
        "ordinary plan retained sift swap receivers" => Some(11),
        "ordinary plan retained final Zip next receivers" => Some(12),
        _ => None,
    }
}
#[derive(Clone, Copy)]
pub(crate) struct ComparisonTicket {
    owner: usize,
    row: usize,
    route: usize,
    phase: usize,
    remaining: usize,
}
// Ticket coordinates only bind observation to the already-active diagnostic
// packet. They are never accepted by a compiler/ledger operation.
pub(crate) fn comparison_before(phase: &'static str, requested: usize) -> Option<ComparisonTicket> {
    let phase = comparison_phase(phase)?;
    let mut ticket = None;
    update(|state| {
        let packet = &mut state.current;
        let Some(row) = packet.coordinate.row.filter(|row| *row < packet.used) else {
            return;
        };
        if packet.owner == 0 {
            return;
        }
        let Some(remaining) = crate::bounded_output::active_remaining() else {
            packet.comparison_complete = false;
            return;
        };
        let route = packet.coordinate.route as usize;
        let cell = &mut packet.rows[row].comparison[route][phase];
        add(&mut cell.attempts, 1, &mut packet.comparison_complete);
        add(
            &mut cell.requested,
            requested,
            &mut packet.comparison_complete,
        );
        ticket = Some(ComparisonTicket {
            owner: packet.owner,
            row,
            route,
            phase,
            remaining,
        });
    });
    ticket
}
pub(crate) fn comparison_after(ticket: Option<ComparisonTicket>, success: bool) {
    let Some(ticket) = ticket else {
        return;
    };
    update(|state| {
        let packet = &mut state.current;
        if packet.owner != ticket.owner || ticket.row >= packet.used {
            packet.comparison_complete = false;
            return;
        }
        let cell = &mut packet.rows[ticket.row].comparison[ticket.route][ticket.phase];
        match crate::bounded_output::active_remaining()
            .and_then(|after| ticket.remaining.checked_sub(after))
        {
            Some(acquired) => add(
                &mut cell.acquired,
                acquired,
                &mut packet.comparison_complete,
            ),
            None => packet.comparison_complete = false,
        }
        if !success {
            add(&mut cell.refused, 1, &mut packet.comparison_complete);
        }
    });
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
    comparison: [[ComparisonCell; COMPARISON_PHASES]; ROUTES],
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
        comparison: [[ComparisonCell::EMPTY; COMPARISON_PHASES]; ROUTES],
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
    comparison_complete: bool,
    used: usize,
    coordinate: Coordinate,
    first: Option<Coordinate>,
    first_is_hir_cleanup: bool,
    rows: Vec<Row>,
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
        comparison_complete: true,
        used: 0,
        coordinate: Coordinate::EMPTY,
        first: None,
        first_is_hir_cleanup: false,
        rows: Vec::new(),
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
        self.comparison_complete = true;
        self.used = 0;
        self.coordinate = Coordinate::EMPTY;
        self.first = None;
        self.first_is_hir_cleanup = false;
    }

    // Copy directly between prepared packets, retaining only the populated rows.
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
        self.comparison_complete = source.comparison_complete;
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
    // Keep the three large diagnostic banks out of every thread's static TLS.
    // Acquire all fixed backing before activating callbacks or live measures.
    if !ENABLED.with(Cell::get) {
        STATE.with(|state| {
            let mut state = state.borrow_mut();
            if state.current.rows.is_empty() {
                state.current.rows.resize(ROWS, Row::EMPTY);
            }
            if state.last.rows.is_empty() {
                state.last.rows.resize(ROWS, Row::EMPTY);
            }
            if state.maximum.rows.is_empty() {
                state.maximum.rows.resize(ROWS, Row::EMPTY);
            }
        });
    }
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
                eprintln!("OPT880 comparison {name} row={index}: complete={}", packet.comparison_complete && !LOST.with(Cell::get));
                for (route_index, route) in [Route::Production, Route::UnoptedCrosscheck, Route::OptedOracle].into_iter().enumerate() {
                    for (comparison_index, cell) in row.comparison[route_index].iter().enumerate() {
                        if cell.attempts != 0 {
                            eprintln!("OPT880 comparison {name} row={index} route={route:?} phase={comparison_index} attempts={} requested={} acquired={} refused={}", cell.attempts, cell.requested, cell.acquired, cell.refused);
                        }
                    }
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
    use super::Operation;
    use crate::bounded_output::{
        reserve_active_required, with_limit_usage_refusal, with_reservation_stage, ReservationStage,
    };
    let _observation = observe();
    let (((), metrics), refused, used, first) = with_limit_usage_refusal(64, || {
        crate::test_allocation_observer::measure(|| {
            let _trace = super::start(0, 0, 6);
            with_reservation_stage(ReservationStage::HirValidation, || {
                let _function = function(0, false, "route-child");
                let _phase = phase(Phase::EnumeratedHIR);
                for operation in [
                    Operation::CleanupReplay,
                    Operation::ReplayStructure,
                    Operation::ReplayExpressionFacts,
                    Operation::ReplayPathStates,
                    Operation::ReplayHirSkeleton,
                    Operation::ReplayPlanSkeleton,
                    Operation::ReplayFactored,
                    Operation::ReplayFactoredConstruction,
                    Operation::ReplayFactoredCanonical,
                ] {
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
    assert_eq!(
        (metrics.allocations, metrics.allocated, metrics.live),
        (0, 0, 0)
    );
    STATE.with(|state| {
        let state = state.borrow();
        assert!(state.have_last && state.have_max);
        let packet = &state.last;
        assert!(packet.complete && packet.first_is_hir_cleanup);
        assert_eq!(
            (packet.hir, packet.expected_hir, packet.non_cleanup_hir),
            (11, 11, 2)
        );
        assert_eq!(
            (packet.unknown_cleanup, packet.overflow_cleanup, packet.used),
            (0, 0, 1)
        );
        assert_eq!(
            packet.rows[0].bytes[Route::Production as usize][Phase::EnumeratedHIR as usize],
            9
        );
        assert_eq!(
            packet.rows[0].calls[Route::Production as usize][Phase::EnumeratedHIR as usize],
            9
        );
        assert_eq!(packet.first.unwrap().row, Some(0));
    });
}

#[test]
fn comparison_aggregate_census_keeps_8769_calls_and_marks_arithmetic_overflow_without_event_buffer()
{
    let mut cell = ComparisonCell::EMPTY;
    let mut complete = true;
    for _ in 0..8_769 {
        add(&mut cell.attempts, 1, &mut complete);
        add(&mut cell.requested, 1_121, &mut complete);
        add(&mut cell.acquired, 1_121, &mut complete);
    }
    assert!(complete);
    assert_eq!(cell.attempts, 8_769);
    assert_eq!(cell.requested, 8_769 * 1_121);
    assert_eq!(cell.acquired, cell.requested);
    assert_eq!(cell.refused, 0);
    add(&mut cell.attempts, usize::MAX, &mut complete);
    assert!(!complete);
    assert_eq!(cell.attempts, usize::MAX);
}

// Full mapping/control test; these are diagnostic cells, never compiler credit.
#[test]
fn comparison_all_thirteen_categories_keep_real_routes_8769_calls_and_checked_overflow() {
    use crate::bounded_output::{reserve_active_required, with_limit_usage_refusal};
    const LABELS: [&str; 13] = [
        "ordinary plan history comparator controls",
        "ordinary plan history order call",
        "ordinary plan history sift call",
        "ordinary plan history comparison receivers",
        "ordinary plan history sort receivers",
        "ordinary plan history left cursor prepare",
        "ordinary plan history right cursor prepare",
        "ordinary plan history HIR cursor prepare",
        "ordinary plan history paired cursor next",
        "ordinary plan history HIR cursor next",
        "ordinary plan retained drain swap receivers",
        "ordinary plan retained sift swap receivers",
        "ordinary plan retained final Zip next receivers",
    ];
    assert_eq!(LABELS.len(), COMPARISON_PHASES);
    assert_eq!(
        comparison_phase("ordinary plan retained comparison owner"),
        Some(0)
    );
    assert_eq!(comparison_phase("foreign comparison phase"), None);
    let _observation = observe();
    for selected_route in [
        Route::Production,
        Route::UnoptedCrosscheck,
        Route::OptedOracle,
    ] {
        for (category, label) in LABELS
            .into_iter()
            .enumerate()
            .chain([(0, "ordinary plan reachable cursor primitive frames")])
        {
            assert_eq!(comparison_phase(label), Some(category));
            let (((), metrics), refused, used, first) = with_limit_usage_refusal(8_769, || {
                crate::test_allocation_observer::measure(|| {
                    // Only observer coordinates are supplied here. The actual
                    // allowance comes from with_limit_usage_refusal above.
                    start(1, 0, 0);
                    let _function = function(0, false, "comparison-census");
                    let _route = route(selected_route);
                    assert!(comparison_before("foreign comparison phase", 1).is_none());
                    for _ in 0..8_769 {
                        let ticket = comparison_before(label, 1);
                        assert!(ticket.is_some());
                        assert!(reserve_active_required(1));
                        comparison_after(ticket, true);
                    }
                    let ticket = comparison_before(label, 1);
                    assert!(ticket.is_some());
                    assert!(!reserve_active_required(1));
                    comparison_after(ticket, false);
                    finish(1, 8_769, 0);
                })
            });
            assert!(refused);
            assert_eq!(used, 8_769);
            let first = first.unwrap();
            assert_eq!((first.requested, first.remaining, first.floor), (1, 0, 0));
            assert!(!metrics.invalid);
            assert_eq!(
                (
                    metrics.allocations,
                    metrics.reallocations,
                    metrics.allocated,
                    metrics.freed,
                    metrics.live
                ),
                (0, 0, 0, 0, 0)
            );
            STATE.with(|state| {
                let state = state.borrow();
                assert!(state.have_last && state.have_max);
                let packet = &state.last;
                assert!(packet.comparison_complete);
                assert_eq!((packet.used, packet.consumed), (1, 8_769));
                for route_index in 0..ROUTES {
                    for phase_index in 0..COMPARISON_PHASES {
                        let cell = packet.rows[0].comparison[route_index][phase_index];
                        let expected =
                            if route_index == selected_route as usize && phase_index == category {
                                (8_770, 8_770, 8_769, 1)
                            } else {
                                (0, 0, 0, 0)
                            };
                        assert_eq!(
                            (cell.attempts, cell.requested, cell.acquired, cell.refused),
                            expected
                        );
                    }
                }
            });
            // Every category's four counters use the SAME checked primitive;
            // exercise each field independently, without saturating away the
            // incomplete flag or confusing it with a compiler refusal.
            let mut cell = ComparisonCell::EMPTY;
            for slot in [
                &mut cell.attempts,
                &mut cell.requested,
                &mut cell.acquired,
                &mut cell.refused,
            ] {
                let mut complete = true;
                add(slot, usize::MAX, &mut complete);
                assert!(complete);
                add(slot, 1, &mut complete);
                assert!(!complete);
                assert_eq!(*slot, usize::MAX);
            }
        }
    }
}

#[test]
fn fixed_route_banks_are_lazy_and_reused_without_allocating_in_callbacks() {
    std::thread::spawn(|| {
        assert!(std::mem::size_of::<State>() < 1024);
        STATE.with(|state| {
            let state = state.borrow();
            assert!(state.current.rows.is_empty());
            assert!(state.last.rows.is_empty());
            assert!(state.maximum.rows.is_empty());
        });
        start(7, 0, 0); // Inactive instrumentation acquires no backing.
        STATE.with(|state| assert!(state.borrow().current.rows.is_empty()));
        let observation = observe();
        let backing = || {
            STATE.with(|state| {
                let state = state.borrow();
                [&state.current, &state.last, &state.maximum].map(|packet| {
                    assert_eq!(packet.rows.len(), ROWS);
                    (packet.rows.as_ptr() as usize, packet.rows.capacity())
                })
            })
        };
        let before = backing();
        let ((), metrics) = crate::test_allocation_observer::measure(|| {
            start(7, 0, 0);
            for ordinal in 0..=ROWS {
                let _function = function(ordinal, false, "fixed-bank");
            }
            finish(7, 0, 0);
        });
        assert!(!metrics.invalid);
        assert_eq!(
            (
                metrics.allocations,
                metrics.reallocations,
                metrics.allocated,
                metrics.freed,
                metrics.live
            ),
            (0, 0, 0, 0, 0)
        );
        assert_eq!(backing(), before);
        STATE.with(|state| {
            let state = state.borrow();
            assert_eq!(state.last.used, ROWS);
            assert!(
                !state.last.complete,
                "the original row overflow still refuses completeness"
            );
        });
        STATE.with(|state| {
            let held = state.borrow();
            let nested = observe();
            assert!(LOST.with(Cell::get));
            drop(nested);
            drop(held);
        });
        drop(observation);
        let ((), reused) = crate::test_allocation_observer::measure(|| {
            let _observation = observe();
            start(8, 0, 0);
            finish(8, 0, 0);
        });
        assert!(!reused.invalid);
        assert_eq!(
            (
                reused.allocations,
                reused.reallocations,
                reused.allocated,
                reused.freed,
                reused.live
            ),
            (0, 0, 0, 0, 0)
        );
        assert_eq!(backing(), before);
    })
    .join()
    .unwrap();
}
