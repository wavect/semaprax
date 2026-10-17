//! Test-only fixed request census. This observer never supplies admission input.
use super::*;
use std::cell::{Cell as Counter, RefCell};
const CATEGORIES: usize = 13;
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Cell {
    pub attempts: usize, pub requested: usize, pub acquired: usize, pub refused: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Event {
    pub requested: usize, pub charged_before: usize, pub phase: &'static str,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Report {
    pub cells: [Cell; CATEGORIES], pub complete: bool, pub foreign: usize,
    pub first: [Option<Event>; CATEGORIES], pub last: [Option<Event>; CATEGORIES],
}
impl Report {
    const EMPTY: Self = Self { cells: [Cell { attempts: 0, requested: 0, acquired: 0, refused: 0 }; CATEGORIES], complete: true, foreign: 0, first: [None; CATEGORIES], last: [None; CATEGORIES] };
}
struct State {
    id: usize, function: usize, budget: usize, binding: crate::bounded_output::classifier_string::ClassifierLedger,
    report: Report, start: usize,
}
thread_local! {
    static ACTIVE: RefCell<Option<State>> = const { RefCell::new(None) };
    static NEXT: Counter<usize> = const { Counter::new(0) };
}
// Correlation ordinal only; no compiler/payer accepts it as authority.
pub(super) struct Scope { previous: Option<State>, id: usize }
impl Scope {
    pub fn enter(function: &ResolvedFunction, budget: &ReplayBudget) -> Self {
        let id = NEXT.with(|next| { match next.get().checked_add(1) { Some(id) => { next.set(id); id }, None => 0 } });
        let mut current = State { id, function: function as *const _ as usize, budget: budget as *const _ as usize,
            binding: crate::bounded_output::classifier_string::ClassifierLedger::capture(),
            report: Report::EMPTY, start: charged(budget).unwrap_or(0) };
        if id == 0 || charged(budget).is_none() { current.report.complete = false; }
        Self { previous: ACTIVE.with(|slot| slot.replace(Some(current))), id }
    }
    pub fn report(&self) -> Report { ACTIVE.with(|slot| {
        match slot.borrow().as_ref() {
            Some(state) if self.id != 0 && state.id == self.id => state.report,
            _ => { let mut report = Report::EMPTY; report.complete = false; report }
        }
    }) }
}
impl Drop for Scope {
    fn drop(&mut self) { ACTIVE.with(|slot| { *slot.borrow_mut() = self.previous.take(); }); }
}
fn charged(budget: &ReplayBudget) -> Option<usize> {
    match budget.payer {
        AllocationPayer::SelectedReservation => budget.physical.as_ref().map(|p| p.charged()),
        AllocationPayer::ActiveBuilder => crate::bounded_output::active_limit()?.checked_sub(crate::bounded_output::active_remaining()?),
        // No unrelated ACTIVE account is substituted for legacy/oracle storage.
        _ => None,
    }
}
fn add(value: &mut usize, by: usize, complete: &mut bool) {
    if let Some(next) = value.checked_add(by) { *value = next; } else { *complete = false; }
}
fn category(phase: &str) -> (usize, &'static str) {
    match phase {
        "factored borrowed merge directory" => (0, "factored borrowed merge directory"),
        "factored merge fee computation controls" => (1, "factored merge fee computation controls"),
        "factored borrowed merge lookup" => (2, "factored borrowed merge lookup"),
        "factored retained merge query" => (2, "factored retained merge query"),
        "factored retained merge comparison" => (3, "factored retained merge comparison"),
        "factored borrowed merge indices" => (4, "factored borrowed merge indices"),
        "factored merge groups" | "factored call merge groups" | "isolated lexical path" | "isolated lexical call state" => (5, "factored merge groups"),
        "factored merge members" | "factored call merge members" | "factored first merge member" | "factored first call member" => (6, "factored merge members"),
        "factored merged paths" | "factored merged call states" | "owned suffix merged rows" => (7, "factored merged paths"),
        "owned suffix dispatch query controls" => (8, "owned suffix dispatch query controls"),
        "owned suffix retention receiving controls" => (9, "owned suffix retention receiving controls"),
        "factored suffix path copies" | "factored suffix path backing" => (10, "factored suffix path backing"),
        "owned suffix merge headers" | "owned suffix borrowed views" | "owned suffix isolated observation" => (11, "owned suffix merge headers"),
        _ => (12, "other reserve_bytes requests"),
    }
}
pub(super) struct Ticket { id: usize, budget: usize, function: usize, category: usize, before: usize }
pub(super) fn before(budget: &ReplayBudget, function: &ResolvedFunction, requested: usize, phase: &str) -> Option<Ticket> {
    ACTIVE.with(|slot| {
        let mut slot = slot.borrow_mut(); let state = slot.as_mut()?;
        if state.budget != budget as *const _ as usize || state.function != function as *const _ as usize || !state.binding.current_identity() {
            add(&mut state.report.foreign, 1, &mut state.report.complete); state.report.complete = false; return None;
        }
        let Some(before) = charged(budget) else { state.report.complete = false; return None; };
        let (category, literal) = category(phase);
        let Some(charged_before) = before.checked_sub(state.start) else { state.report.complete = false; return None; };
        let event = Event { requested, charged_before, phase: literal };
        state.report.first[category].get_or_insert(event); state.report.last[category] = Some(event);
        let cell = &mut state.report.cells[category];
        add(&mut cell.attempts, 1, &mut state.report.complete);
        add(&mut cell.requested, requested, &mut state.report.complete);
        Some(Ticket { id: state.id, budget: state.budget, function: state.function, category, before })
    })
}
pub(super) fn after(ticket: Option<Ticket>, budget: &ReplayBudget, function: &ResolvedFunction, success: bool) {
    let Some(ticket) = ticket else { return; };
    ACTIVE.with(|slot| {
        let mut slot = slot.borrow_mut(); let Some(state) = slot.as_mut() else { return; };
        if state.id != ticket.id || state.budget != ticket.budget || state.function != ticket.function || state.budget != budget as *const _ as usize || state.function != function as *const _ as usize || !state.binding.current_identity() {
            state.report.complete = false; return;
        }
        let cell = &mut state.report.cells[ticket.category];
        match charged(budget).and_then(|after| after.checked_sub(ticket.before)) {
            Some(delta) => add(&mut cell.acquired, delta, &mut state.report.complete),
            None => state.report.complete = false,
        }
        if !success { add(&mut cell.refused, 1, &mut state.report.complete); }
    });
}
#[test]
fn fixed_merge_census_keeps_thirteen_cells_real_account_identity_and_checked_overflow() {
    let program = crate::hir::resolve(&crate::check("module census; @id(\"main\") fn main()->i64 {0}", "census.spx").unwrap()).unwrap();
    let function = &program.functions[0];
    crate::bounded_output::with_limit_usage_refusal(1 << 24, || {
        let mut budget = ReplayBudget::with_physical(function, 1 << 20).unwrap();
        budget.reserve_bytes(function, 1, "census bootstrap").unwrap();
        {
            let scope = Scope::enter(function, &budget);
            for phase in ["factored borrowed merge directory", "factored merge fee computation controls", "factored retained merge query", "factored retained merge comparison", "factored borrowed merge indices", "factored merge groups", "factored merge members", "factored merged paths", "owned suffix dispatch query controls", "owned suffix retention receiving controls", "factored suffix path backing", "owned suffix merge headers", "closed other category fixture"] {
                budget.reserve_bytes(function, 1, phase).unwrap();
            }
            let report = scope.report(); assert!(report.complete);
            assert_eq!(report.cells, [Cell { attempts: 1, requested: 1, acquired: 1, refused: 0 }; CATEGORIES]);
        }
        let scope = Scope::enter(function, &budget);
        for _ in 0..8769 { budget.reserve_bytes(function, 1, "factored borrowed merge lookup").unwrap(); }
        let report = scope.report(); assert!(report.complete);
        assert_eq!(report.cells[2], Cell { attempts: 8769, requested: 8769, acquired: 8769, refused: 0 });
        let denied = (1 << 20) - budget.physical.as_ref().unwrap().charged() + 1;
        let error = budget.reserve_bytes(function, denied, "factored borrowed merge lookup").unwrap_err();
        assert_eq!(scope.report().cells[2], Cell { attempts: 8770, requested: 8769 + denied, acquired: 8769, refused: 1 });
        drop(error);
        let prior = scope.report().cells;
        let mut other = ReplayBudget::with_physical(function, 1 << 20).unwrap();
        other.reserve_bytes(function, 1, "wrong budget cannot enter this census").unwrap(); drop(other);
        assert_eq!(scope.report().cells, prior); assert!(!scope.report().complete);
        crate::bounded_output::with_limit_usage_refusal(1 << 20, || { budget.reserve_bytes(function, 0, "foreign invocation").ok(); });
        let report = scope.report(); assert!(!report.complete); assert_eq!(report.cells, prior); assert!(report.foreign > 0);
        drop(scope); drop(budget);
    });
    for category in 0..CATEGORIES { for field in 0..4 { let mut report = Report::EMPTY; let cell = &mut report.cells[category];
        let value = match field { 0 => &mut cell.attempts, 1 => &mut cell.requested, 2 => &mut cell.acquired, _ => &mut cell.refused };
        *value = usize::MAX; add(value, 1, &mut report.complete); assert!(!report.complete); assert_eq!(*value, usize::MAX);
    } }
}

// Stack-only lossless first Diagnostic snapshot; captures no owning error
// beyond the allocator window and cannot affect selected failure provenance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Failure {
    code: &'static str, severity: crate::diagnostic::Severity,
    message: ([u8; 2048], usize), path: Option<([u8; 2048], usize)>,
    span: Option<crate::ast::Span>, help: Option<([u8; 2048], usize)>,
}
fn text(value: &str) -> ([u8; 2048], usize) {
    let mut out = [0; 2048]; assert!(value.len() <= out.len());
    out[..value.len()].copy_from_slice(value.as_bytes()); (out, value.len())
}
pub(super) fn failure(value: &Diagnostic) -> Failure {
    Failure { code: value.code, severity: value.severity, message: text(&value.message),
        path: value.path.as_deref().map(text), span: value.span, help: value.help.as_deref().map(text) }
}
