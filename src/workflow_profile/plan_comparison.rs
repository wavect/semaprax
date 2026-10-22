//! Diagnostic-only, current-thread observation of the ordinary PLAN comparator.
//! Fixed TLS storage is outside the compiler's allocation ledger. No timings,
//! allocations, admission decisions, or default-shipping performance are measured.
pub use crate::bounded_output::exclusive_debits::{
    Algorithm as CoreDebitAlgorithm, Attempt as CoreDebitAttempt, Operation as CoreDebitOperation,
    Outcome as CoreDebitOutcome, Refusal as CoreDebitRefusal, Report as ResolvedCoreDebits,
    ATTEMPT_CAPACITY as CORE_ATTEMPT_CAPACITY,
};
use sha2::{Digest, Sha256};
use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::rc::Rc;

pub const ROW_CAPACITY: usize = 128;
const DEPTH: usize = 32;
/// A prefix is never presented as the full identifier when truncated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextIdentity {
    pub prefix: [u8; 96],
    pub prefix_len: usize,
    pub full_len: usize,
    pub sha256: [u8; 32],
}
impl TextIdentity {
    const EMPTY: Self = Self {
        prefix: [0; 96],
        prefix_len: 0,
        full_len: 0,
        sha256: [0; 32],
    };
    fn new(text: &str) -> Self {
        let mut value = Self::EMPTY;
        value.full_len = text.len();
        value.prefix_len = text.len().min(value.prefix.len());
        value.prefix[..value.prefix_len].copy_from_slice(&text.as_bytes()[..value.prefix_len]);
        value.sha256 = Sha256::digest(text.as_bytes()).into();
        value
    }
}
/// Caller-supplied fixture metadata, never source or validation authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Binding {
    pub fixture: TextIdentity,
    pub source_sha256: [u8; 32],
    pub caller_revision: Option<[u8; 20]>,
}
impl Binding {
    pub fn caller_labelled(fixture: &str, source_sha256: [u8; 32]) -> Self {
        Self {
            fixture: TextIdentity::new(fixture),
            source_sha256,
            caller_revision: None,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Payer {
    Unobserved,
    ActiveBuilder,
    SelectedReservation,
    Oracle,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Build {
    InstrumentedProductionAlgorithm,
    InstrumentedLibraryTest,
}
/// Closed sites. Counts describe reached prefixes, not successful admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(usize)]
pub enum Site {
    OwnerAdmitted,
    PairPrepared,
    Round,
    PairScopeAdmitted,
    MatchAdmitted,
    StepAdmitted,
    LoadAdmitted,
    EmptySegment,
    TailAdmitted,
    LeftHead,
    RightHead,
    LeftTailSome,
    RightTailSome,
    LeftExhausted,
    RightExhausted,
    TerminalCompared,
    LeftPrepareAdmitted,
    RightPrepareAdmitted,
    LeftResetCompleted,
    RightResetCompleted,
    IdentityDetectorAdmitted,
    IdentityReplayAdmitted,
    IdentityHit,
    IdentityObservation,
    IdentityWork,
    IdentityCapture,
    IdentityProbeAdmitted,
    IdentityEmptyProbe,
    CertifiedBaseAdmitted,
    CertifiedPopAdmitted,
    CertifiedRhsAdmitted,
    CertifiedViewAdmitted,
    CertifiedHeadAdmitted,
    CertifiedFirstAdmitted,
    BoundaryChecksAdmitted,
    BoundaryLeftEmptyAdmitted,
    BoundaryRightEmptyAdmitted,
    BoundaryLinksAdmitted,
    BoundaryViewAdmitted,
    BoundaryBodyAdmitted,
    NumericCaptureAdmitted,
    NumericTurnAdmitted,
    NumericPublishAdmitted,
    BoundaryCompleted,
    TailMaskAdmitted,
    TailMarkAdmitted,
    TailTurnAdmitted,
    TailWork,
    BorrowedRightStep,
    BorrowedRightTail,
    EmptySelectorAdmitted,
    EmptySelectorSkipped,
    FinalFlatAdmitted,
    FinalSharedAdmitted,
    FinalStepAdmitted,
    FinalTailAdmitted,
}
const SITES: usize = 56;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Counts {
    pub events: [u64; SITES],
    /// Sum of actually admitted bank bytes at the corresponding closed site.
    pub admitted_bytes: [u64; SITES],
    pub started: u64,
    pub ended: u64,
    pub succeeded: u64,
    pub unwound: u64,
}
impl Counts {
    const EMPTY: Self = Self {
        events: [0; SITES],
        admitted_bytes: [0; SITES],
        started: 0,
        ended: 0,
        succeeded: 0,
        unwound: 0,
    };
    pub fn at(&self, site: Site) -> u64 {
        self.events[site as usize]
    }
}
/// Actual compiled tariffs; not a forecast of reached events or a fee credit.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Invoice {
    pub payment_header: usize,
    pub owner: usize,
    pub pair_primitives: usize,
    pub paired_primitives: usize,
    pub path_index: usize,
    pub pair_scope: usize,
    pub pair_match: usize,
    pub new_pair_references: usize,
    pub step: usize,
    pub load: usize,
    pub empty: usize,
    pub tail: usize,
}
impl Invoice {
    const EMPTY: Self = Self {
        payment_header: 0,
        owner: 0,
        pair_primitives: 0,
        paired_primitives: 0,
        path_index: 0,
        pair_scope: 0,
        pair_match: 0,
        new_pair_references: 0,
        step: 0,
        load: 0,
        empty: 0,
        tail: 0,
    };
}
// Fixed TLS only; the opaque pointer is never dereferenced or accepted by
// compiler operations. Every install AND restore invalidates earlier snapshots.
#[derive(Clone, Copy, Debug)]
pub(crate) struct LedgerSnapshot {
    identity: usize,
    epoch: u64,
    remaining: usize,
}
thread_local! {
    static OBSERVED_LEDGER_EPOCH: Cell<Option<u64>> = const { Cell::new(Some(0)) };
}
pub(crate) fn observe_ledger_transition() {
    crate::bounded_output::exclusive_debits::transition();
    OBSERVED_LEDGER_EPOCH.with(|epoch| epoch.set(epoch.get().and_then(|v| v.checked_add(1))));
}
impl LedgerSnapshot {
    pub(crate) fn observe(identity: usize, remaining: usize) -> Option<Self> {
        OBSERVED_LEDGER_EPOCH.with(|epoch| {
            Some(Self {
                identity,
                epoch: epoch.get()?,
                remaining,
            })
        })
    }
    fn paid_until(self, after: Self) -> Option<usize> {
        if self.identity != after.identity || self.epoch != after.epoch {
            return None;
        }
        self.remaining.checked_sub(after.remaining)
    }
}

/// Inclusive ordinary-ledger scopes, distinct from comparator rows and event counts.
/// A completed scope is not an algorithm-success or application-admission claim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(usize)]
pub enum PaidPhase {
    CleanupValidation,
    CanonicalRebuild,
    ReplayStructure,
    CrosscheckEnumerated,
    CrosscheckFactored,
    FactoredComparison,
    FactoredConstruction,
    HirPaths,
    PlanPaths,
    Comparison,
}
const PAID_PHASES: usize = 10;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(usize)]
pub enum PhasePayer {
    ActiveBuilder,
    Unobserved,
    Oracle,
    Selected,
    Unknown,
}
const PHASE_PAYERS: usize = 5;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(usize)]
pub enum PhaseOrigin {
    Shipping,
    TestCrosscheck,
}
const PHASE_ORIGINS: usize = 2;
#[derive(Clone, Copy, Debug)]
pub struct PaidPhaseRow {
    pub entered: u64,
    pub ended: u64,
    pub normal_exits: u64,
    pub refused_prefixes: u64,
    pub unwound: u64,
    pub unknown: u64,
    /// Sum of known inclusive same-ledger debits only; never sum nested rows.
    pub known_paid: u64,
}
impl PaidPhaseRow {
    const EMPTY: Self = Self {
        entered: 0,
        ended: 0,
        normal_exits: 0,
        refused_prefixes: 0,
        unwound: 0,
        unknown: 0,
        known_paid: 0,
    };
}
#[derive(Clone, Copy, Debug)]
pub struct PaidPhases {
    /// Index with `row(phase, payer, origin)`; includes all zero rows explicitly.
    pub rows: [PaidPhaseRow; PAID_PHASES * PHASE_PAYERS * PHASE_ORIGINS],
    pub balanced: bool,
    pub overflow: bool,
}
impl PaidPhases {
    const EMPTY: Self = Self {
        rows: [PaidPhaseRow::EMPTY; PAID_PHASES * PHASE_PAYERS * PHASE_ORIGINS],
        balanced: true,
        overflow: false,
    };
    pub fn row(&self, phase: PaidPhase, payer: PhasePayer, origin: PhaseOrigin) -> &PaidPhaseRow {
        &self.rows
            [(phase as usize * PHASE_PAYERS + payer as usize) * PHASE_ORIGINS + origin as usize]
    }
}
/// Feature-only lexical guard. Its opaque snapshot grants no payer authority.
pub(crate) struct PaidPhaseGuard {
    generation: Option<u64>,
    depth: usize,
    index: usize,
    serial: u64,
    payer: PhasePayer,
    crosscheck: bool,
    before: Option<LedgerSnapshot>,
}
fn phase_access<T>(f: impl FnOnce(&mut State) -> T) -> Option<T> {
    STATE.with(|state| match state.try_borrow_mut() {
        Ok(mut state) => Some(f(&mut state)),
        Err(_) => {
            PHASE_LOST.with(|lost| lost.set(true));
            None
        }
    })
}
pub(crate) fn paid_phase(phase: PaidPhase, payer: PhasePayer) -> PaidPhaseGuard {
    let crosscheck = matches!(
        phase,
        PaidPhase::CrosscheckEnumerated | PaidPhase::CrosscheckFactored
    );
    let before = if payer == PhasePayer::ActiveBuilder {
        crate::bounded_output::active_ledger_snapshot()
    } else {
        None
    };
    let registered = phase_access(|s| {
        if !s.enabled {
            return None;
        }
        let origin = if crosscheck || s.phase_crosschecks != 0 {
            PhaseOrigin::TestCrosscheck
        } else {
            PhaseOrigin::Shipping
        };
        let index =
            (phase as usize * PHASE_PAYERS + payer as usize) * PHASE_ORIGINS + origin as usize;
        let bank = &mut s.paid_phases;
        State::add(&mut bank.rows[index].entered, 1, &mut bank.overflow);
        let Some(serial) = s.phase_serial.checked_add(1) else {
            bank.overflow = true;
            bank.balanced = false;
            return None;
        };
        s.phase_serial = serial;
        let depth = s.phase_depth;
        if depth < DEPTH {
            s.phase_stack[depth] = serial;
        } else {
            bank.balanced = false;
        }
        let Some(next) = depth.checked_add(1) else {
            bank.overflow = true;
            bank.balanced = false;
            return None;
        };
        s.phase_depth = next;
        if crosscheck {
            s.phase_crosschecks += 1;
        }
        Some((s.generation, next, serial, index))
    })
    .flatten();
    let (generation, depth, serial, index) = registered
        .map(|(g, d, s, i)| (Some(g), d, s, i))
        .unwrap_or((None, 0, 0, 0));
    PaidPhaseGuard {
        generation,
        depth,
        index,
        serial,
        payer,
        crosscheck,
        before,
    }
}
pub(crate) fn active_paid_phase(phase: PaidPhase) -> PaidPhaseGuard {
    paid_phase(
        phase,
        if crate::bounded_output::active_limit().is_some() {
            PhasePayer::ActiveBuilder
        } else {
            PhasePayer::Unobserved
        },
    )
}
impl Drop for PaidPhaseGuard {
    fn drop(&mut self) {
        let after = crate::bounded_output::active_ledger_snapshot();
        let refused = self.payer == PhasePayer::ActiveBuilder
            && self
                .before
                .zip(after)
                .and_then(|(a, b)| a.paid_until(b))
                .is_some()
            && crate::bounded_output::active_refused();
        let unwound = std::thread::panicking();
        phase_access(|s| {
            if !s.enabled || self.generation != Some(s.generation) {
                return;
            }
            let ordered = s.phase_depth == self.depth
                && self.depth <= DEPTH
                && s.phase_stack[self.depth - 1] == self.serial;
            let bank = &mut s.paid_phases;
            if !ordered {
                bank.balanced = false;
            }
            let row = &mut bank.rows[self.index];
            State::add(&mut row.ended, 1, &mut bank.overflow);
            if unwound {
                State::add(&mut row.unwound, 1, &mut bank.overflow);
            } else {
                State::add(&mut row.normal_exits, 1, &mut bank.overflow);
            }
            if refused {
                State::add(&mut row.refused_prefixes, 1, &mut bank.overflow);
            }
            let paid = if ordered {
                self.before.zip(after).and_then(|(a, b)| a.paid_until(b))
            } else {
                None
            };
            match paid.and_then(|v| u64::try_from(v).ok()) {
                Some(bytes) => State::add(&mut row.known_paid, bytes, &mut bank.overflow),
                None => State::add(&mut row.unknown, 1, &mut bank.overflow),
            }
            if s.phase_depth == self.depth {
                s.phase_depth -= 1;
                if self.crosscheck {
                    s.phase_crosschecks -= 1;
                }
            }
        });
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Row {
    pub serial: u64,
    pub function: TextIdentity,
    pub span: crate::ast::Span,
    pub payer: Payer,
    pub selected: bool,
    pub invoice: Invoice,
    pub counts: Counts,
    /// Active ordinary ledger at guard entry/exit; not allocator/RSS bytes.
    pub ledger_entry_remaining: Option<usize>,
    pub ledger_exit_remaining: Option<usize>,
    /// Inclusive successful debit within this guard, only for an unchanged
    /// ActiveBuilder ledger. Missing/changed scopes and underflow are unknown.
    pub ledger_paid: Option<usize>,
    ledger_before: Option<LedgerSnapshot>,
}
impl Row {
    const EMPTY: Self = Self {
        serial: 0,
        function: TextIdentity::EMPTY,
        span: crate::ast::Span {
            start: 0,
            end: 0,
            line: 0,
            column: 0,
        },
        payer: Payer::Unobserved,
        selected: false,
        invoice: Invoice::EMPTY,
        counts: Counts::EMPTY,
        ledger_entry_remaining: None,
        ledger_exit_remaining: None,
        ledger_paid: None,
        ledger_before: None,
    };
}
#[derive(Debug)]
pub struct Report {
    pub paid_phases: PaidPhases,
    pub build: Build,
    pub route: &'static str,
    pub generation: u64,
    pub binding: Binding,
    pub counts: Counts,
    pub rows: [Row; ROW_CAPACITY],
    pub row_count: usize,
    pub detail_complete: bool,
    pub counts_complete: bool,
    pub overflow: bool,
    pub resolved_core: ResolvedCoreDebits,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureError {
    Busy,
    GenerationExhausted,
    LiveInvocation,
    LostCapture,
}
struct State {
    paid_phases: PaidPhases,
    phase_stack: [u64; DEPTH],
    phase_depth: usize,
    phase_serial: u64,
    phase_crosschecks: usize,
    enabled: bool,
    generation: u64,
    binding: Binding,
    rows: [Row; ROW_CAPACITY],
    row_count: usize,
    counts: Counts,
    stack: [Option<usize>; DEPTH],
    depth: usize,
    detail_complete: bool,
    complete: bool,
    overflow: bool,
}
impl State {
    const fn new() -> Self {
        Self {
            paid_phases: PaidPhases::EMPTY,
            phase_stack: [0; DEPTH],
            phase_depth: 0,
            phase_serial: 0,
            phase_crosschecks: 0,
            enabled: false,
            generation: 0,
            binding: Binding {
                fixture: TextIdentity::EMPTY,
                source_sha256: [0; 32],
                caller_revision: None,
            },
            rows: [Row::EMPTY; ROW_CAPACITY],
            row_count: 0,
            counts: Counts::EMPTY,
            stack: [None; DEPTH],
            depth: 0,
            detail_complete: true,
            complete: true,
            overflow: false,
        }
    }
    fn add(target: &mut u64, amount: u64, overflow: &mut bool) {
        if let Some(value) = target.checked_add(amount) {
            *target = value;
        } else {
            *overflow = true;
        }
    }
    fn each(&mut self, mut f: impl FnMut(&mut Counts, &mut bool)) {
        f(&mut self.counts, &mut self.overflow);
        if self.depth > 0 && self.depth <= DEPTH {
            if let Some(index) = self.stack[self.depth - 1] {
                f(&mut self.rows[index].counts, &mut self.overflow);
            }
        }
    }
}
thread_local! {
    static STATE: RefCell<State> = const { RefCell::new(State::new()) };
    static LOST: Cell<bool> = const { Cell::new(false) };
    static PHASE_LOST: Cell<bool> = const { Cell::new(false) };
}
fn access<T>(f: impl FnOnce(&mut State) -> T) -> Option<T> {
    STATE.with(|state| match state.try_borrow_mut() {
        Ok(mut state) => Some(f(&mut state)),
        Err(_) => {
            LOST.with(|lost| lost.set(true));
            None
        }
    })
}
/// Begin/finish never execute compiler work or change its result. Thread-bound.
pub struct Capture {
    generation: u64,
    _thread: PhantomData<Rc<()>>,
}
impl Capture {
    pub fn begin(binding: Binding) -> Result<Self, CaptureError> {
        access(|s| {
            if s.enabled {
                return Err(CaptureError::Busy);
            }
            let generation = s
                .generation
                .checked_add(1)
                .ok_or(CaptureError::GenerationExhausted)?;
            *s = State::new();
            s.generation = generation;
            s.binding = binding;
            s.enabled = true;
            LOST.with(|lost| lost.set(false));
            PHASE_LOST.with(|lost| lost.set(false));
            crate::bounded_output::exclusive_debits::begin(generation);
            Ok(Self {
                generation,
                _thread: PhantomData,
            })
        })
        .unwrap_or(Err(CaptureError::Busy))
    }
    /// Call after compiler owners have dropped. A live invocation refuses finish.
    pub fn finish(self) -> Result<Report, CaptureError> {
        access(|s| {
            if !s.enabled || s.generation != self.generation {
                return Err(CaptureError::LostCapture);
            }
            if s.depth != 0 || s.phase_depth != 0 || s.phase_crosschecks != 0 {
                return Err(CaptureError::LiveInvocation);
            }
            s.enabled = false;
            s.paid_phases.balanced &= !PHASE_LOST.with(Cell::get);
            Ok(Report {
                paid_phases: s.paid_phases,
                resolved_core: crate::bounded_output::exclusive_debits::finish(s.generation),
                build: if cfg!(test) {
                    Build::InstrumentedLibraryTest
                } else {
                    Build::InstrumentedProductionAlgorithm
                },
                route: "ordinary-plan-comparison/paired-value.v1",
                generation: s.generation,
                binding: s.binding,
                counts: s.counts,
                rows: s.rows,
                row_count: s.row_count,
                detail_complete: s.detail_complete,
                counts_complete: s.complete && !s.overflow && !LOST.with(Cell::get),
                overflow: s.overflow,
            })
        })
        .unwrap_or(Err(CaptureError::LostCapture))
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        access(|s| {
            if s.generation == self.generation {
                crate::bounded_output::exclusive_debits::cancel(self.generation);
                s.enabled = false;
            }
        });
    }
}
/// Private compiler guard: stores no references into a function, arena or payer.
pub(crate) struct Invocation {
    generation: Option<u64>,
    depth: usize,
    succeeded: bool,
}
pub(crate) fn begin(
    function: &str,
    span: crate::ast::Span,
    payer: Payer,
    selected: bool,
    invoice: Invoice,
) -> Invocation {
    let identity = access(|s| {
        if !s.enabled {
            return None;
        }
        let serial = match s.counts.started.checked_add(1) {
            Some(serial) => serial,
            None => {
                s.overflow = true;
                0
            }
        };
        let row = if s.row_count < ROW_CAPACITY {
            let index = s.row_count;
            s.row_count += 1;
            let ledger_before = if payer == Payer::ActiveBuilder {
                crate::bounded_output::active_ledger_snapshot()
            } else {
                None
            };
            s.rows[index] = Row {
                serial,
                function: TextIdentity::new(function),
                span,
                payer,
                selected,
                invoice,
                counts: Counts::EMPTY,
                ledger_entry_remaining: ledger_before.map(|v| v.remaining),
                ledger_exit_remaining: None,
                ledger_paid: None,
                ledger_before,
            };
            Some(index)
        } else {
            s.detail_complete = false;
            None
        };
        if s.depth < DEPTH {
            s.stack[s.depth] = row;
        } else {
            s.detail_complete = false;
        }
        match s.depth.checked_add(1) {
            Some(depth) => s.depth = depth,
            None => {
                s.complete = false;
                return None;
            }
        }
        s.each(|c, overflow| State::add(&mut c.started, 1, overflow));
        Some((s.generation, s.depth))
    })
    .flatten();
    Invocation {
        generation: identity.map(|x| x.0),
        depth: identity.map_or(0, |x| x.1),
        succeeded: false,
    }
}
pub(crate) fn event(site: Site, bytes: usize) {
    access(|s| {
        if s.enabled && s.depth != 0 {
            s.each(|c, overflow| {
                State::add(&mut c.events[site as usize], 1, overflow);
                match u64::try_from(bytes) {
                    Ok(bytes) => State::add(&mut c.admitted_bytes[site as usize], bytes, overflow),
                    Err(_) => *overflow = true,
                }
            });
        }
    });
}
impl Invocation {
    pub(crate) fn success(&mut self) {
        self.succeeded = true;
    }
}
impl Drop for Invocation {
    fn drop(&mut self) {
        access(|s| {
            if self.generation != Some(s.generation) || !s.enabled {
                return;
            }
            if s.depth != self.depth {
                s.complete = false;
                return;
            }
            if let Some(index) = s
                .depth
                .checked_sub(1)
                .and_then(|i| s.stack.get(i))
                .copied()
                .flatten()
            {
                let row = &mut s.rows[index];
                let after = if row.payer == Payer::ActiveBuilder {
                    crate::bounded_output::active_ledger_snapshot()
                } else {
                    None
                };
                row.ledger_exit_remaining = after.map(|v| v.remaining);
                row.ledger_paid = row
                    .ledger_before
                    .zip(after)
                    .and_then(|(before, after)| before.paid_until(after));
            }
            s.each(|c, overflow| {
                State::add(&mut c.ended, 1, overflow);
                if self.succeeded {
                    State::add(&mut c.succeeded, 1, overflow);
                }
                if std::thread::panicking() {
                    State::add(&mut c.unwound, 1, overflow);
                }
            });
            s.depth -= 1;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn binding() -> Binding {
        Binding::caller_labelled("synthetic compiler observation", [17; 32])
    }
    fn invocation(id: &str) -> Invocation {
        begin(
            id,
            crate::ast::Span {
                start: 1,
                end: 9,
                line: 2,
                column: 3,
            },
            Payer::ActiveBuilder,
            true,
            Invoice::EMPTY,
        )
    }
    #[test]
    fn paid_phase_scopes_keep_nested_prefixes_unknown_ledgers_and_overflow_separate() {
        paid_phase_nested_ledger_prefix_cases();
        paid_phase_counter_overflow_case();
        paid_phase_live_scope_refusal_case();
        paid_phase_reentrant_access_case();
    }

    // Each independent fixed-size report stays in its own ordinary-stack frame.
    #[inline(never)]
    fn paid_phase_nested_ledger_prefix_cases() {
        let capture = Capture::begin(binding()).unwrap();
        let (child_paid, refused, paid) = crate::bounded_output::with_limit_usage(100, || {
            let _outer = active_paid_phase(PaidPhase::CleanupValidation);
            assert!(crate::bounded_output::reserve_active_required(3));
            {
                let _inner = active_paid_phase(PaidPhase::CanonicalRebuild);
                assert!(crate::bounded_output::reserve_active_required(5));
            }
            assert_eq!(crate::bounded_output::active_remaining(), Some(92));
            let child_paid = {
                let _restored = active_paid_phase(PaidPhase::ReplayStructure);
                let (_, refused, paid) = crate::bounded_output::with_limit_usage(20, || {
                    assert!(crate::bounded_output::reserve_active_required(7));
                });
                assert!(!refused && paid == 7);
                paid
            };
            // Child output is charged back to the restored parent ledger.
            assert_eq!(
                crate::bounded_output::active_remaining(),
                Some(92 - child_paid)
            );
            {
                let _prefix = active_paid_phase(PaidPhase::Comparison);
                assert!(crate::bounded_output::reserve_active_required(2));
                assert!(!crate::bounded_output::reserve_active_required(101));
            }
            assert_eq!(
                crate::bounded_output::active_remaining(),
                Some(90 - child_paid)
            );
            child_paid
        });
        assert!(refused && paid == 3 + 5 + child_paid + 2);
        for payer in [
            PhasePayer::Unobserved,
            PhasePayer::Selected,
            PhasePayer::Oracle,
            PhasePayer::Unknown,
        ] {
            let _other = paid_phase(PaidPhase::HirPaths, payer);
        }
        crate::bounded_output::with_limit_usage(10, || {
            let _crosscheck = active_paid_phase(PaidPhase::CrosscheckEnumerated);
            let _child = active_paid_phase(PaidPhase::HirPaths);
            assert!(crate::bounded_output::reserve_active_required(4));
        });
        let unwind = std::panic::catch_unwind(|| {
            let _unwind = active_paid_phase(PaidPhase::PlanPaths);
            std::panic::resume_unwind(Box::new("phase-control"));
        });
        assert!(unwind.is_err());
        let report = capture.finish().unwrap();
        assert!(report.paid_phases.balanced && !report.paid_phases.overflow);
        assert_eq!(report.row_count, 0);
        assert_eq!(report.counts.started, 0);
        let row = |phase| {
            report
                .paid_phases
                .row(phase, PhasePayer::ActiveBuilder, PhaseOrigin::Shipping)
        };
        assert_eq!(row(PaidPhase::CanonicalRebuild).known_paid, 5);
        assert_eq!(row(PaidPhase::CanonicalRebuild).unknown, 0);
        assert_eq!(row(PaidPhase::Comparison).known_paid, 2);
        assert_eq!(row(PaidPhase::Comparison).refused_prefixes, 1);
        assert_eq!(row(PaidPhase::ReplayStructure).unknown, 1);
        // Child-ledger installation invalidates the entire enclosing interval.
        assert_eq!(row(PaidPhase::CleanupValidation).unknown, 1);
        for payer in [
            PhasePayer::Unobserved,
            PhasePayer::Selected,
            PhasePayer::Oracle,
            PhasePayer::Unknown,
        ] {
            let row = report
                .paid_phases
                .row(PaidPhase::HirPaths, payer, PhaseOrigin::Shipping);
            assert_eq!(
                (row.entered, row.ended, row.unknown, row.known_paid),
                (1, 1, 1, 0)
            );
        }
        let child = report.paid_phases.row(
            PaidPhase::HirPaths,
            PhasePayer::ActiveBuilder,
            PhaseOrigin::TestCrosscheck,
        );
        assert_eq!((child.known_paid, child.unknown), (4, 0));
        assert_eq!(
            report
                .paid_phases
                .row(
                    PaidPhase::HirPaths,
                    PhasePayer::ActiveBuilder,
                    PhaseOrigin::Shipping
                )
                .entered,
            0
        );
        let unwind = report.paid_phases.row(
            PaidPhase::PlanPaths,
            PhasePayer::Unobserved,
            PhaseOrigin::Shipping,
        );
        assert_eq!((unwind.ended, unwind.unwound), (1, 1));
    }

    #[inline(never)]
    fn paid_phase_counter_overflow_case() {
        let capture = Capture::begin(binding()).unwrap();
        access(|s| s.paid_phases.rows[0].entered = u64::MAX).unwrap();
        {
            let _overflow = paid_phase(PaidPhase::CleanupValidation, PhasePayer::ActiveBuilder);
        }
        let report = capture.finish().unwrap();
        assert!(report.paid_phases.overflow);
        assert_eq!(report.counts.started, 0);
        assert!(report.counts_complete);
    }

    #[inline(never)]
    fn paid_phase_live_scope_refusal_case() {
        let capture = Capture::begin(binding()).unwrap();
        let live = active_paid_phase(PaidPhase::CleanupValidation);
        assert!(matches!(
            capture.finish(),
            Err(CaptureError::LiveInvocation)
        ));
        drop(live);
    }

    #[inline(never)]
    fn paid_phase_reentrant_access_case() {
        let capture = Capture::begin(binding()).unwrap();
        STATE.with(|state| {
            let _held = state.borrow_mut();
            let _lost = active_paid_phase(PaidPhase::CleanupValidation);
        });
        let report = capture.finish().unwrap();
        assert!(!report.paid_phases.balanced);
        assert!(report.counts_complete);
    }

    #[test]
    fn ordinary_ledger_paid_is_observed_not_inferred_and_scope_changes_stay_unknown() {
        let capture = Capture::begin(binding()).unwrap();
        let (_, refused, paid) = crate::bounded_output::with_limit_usage(100, || {
            {
                let mut guard = invocation("same-ledger-success");
                assert!(crate::bounded_output::reserve_active_required(13));
                guard.success();
            }
            {
                let _guard = invocation("restored-child-is-not-continuous");
                let (_, refused, child_paid) = crate::bounded_output::with_limit_usage(20, || {
                    assert!(crate::bounded_output::reserve_active_required(7));
                });
                assert!(!refused);
                assert_eq!(child_paid, 7);
            }
            {
                let _guard = invocation("same-ledger-refusal-prefix");
                assert!(crate::bounded_output::reserve_active_required(3));
                assert!(!crate::bounded_output::reserve_active_required(100));
            }
        });
        assert!(refused);
        assert_eq!(paid, 23);
        {
            let _guard = invocation("no-active-ledger");
        }
        let report = capture.finish().unwrap();
        assert!(report.counts_complete && report.detail_complete && !report.overflow);
        assert_eq!(report.rows[0].ledger_entry_remaining, Some(100));
        assert_eq!(report.rows[0].ledger_exit_remaining, Some(87));
        assert_eq!(report.rows[0].ledger_paid, Some(13));
        assert_eq!(report.rows[1].ledger_entry_remaining, Some(87));
        assert_eq!(report.rows[1].ledger_exit_remaining, Some(80));
        assert_eq!(report.rows[1].ledger_paid, None);
        assert_eq!(report.rows[2].ledger_paid, Some(3));
        assert_eq!(report.rows[2].counts.succeeded, 0);
        assert_eq!(report.rows[3].ledger_paid, None);
        let before = LedgerSnapshot {
            identity: 1,
            epoch: 2,
            remaining: 10,
        };
        assert_eq!(
            before.paid_until(LedgerSnapshot {
                identity: 2,
                ..before
            }),
            None
        );
        assert_eq!(
            before.paid_until(LedgerSnapshot {
                remaining: 11,
                ..before
            }),
            None
        );
        // Isolate intentional sticky exhaustion from all other test threads.
        std::thread::spawn(|| {
            OBSERVED_LEDGER_EPOCH.with(|epoch| epoch.set(Some(u64::MAX)));
            observe_ledger_transition();
            assert!(LedgerSnapshot::observe(1, 10).is_none());
            observe_ledger_transition();
            assert!(LedgerSnapshot::observe(1, 10).is_none());
        })
        .join()
        .unwrap();
    }
    #[test]
    fn non_builder_payers_never_claim_ordinary_ledger_paid() {
        let capture = Capture::begin(binding()).unwrap();
        let (_, refused, paid) = crate::bounded_output::with_limit_usage(100, || {
            for payer in [Payer::Unobserved, Payer::SelectedReservation, Payer::Oracle] {
                let _guard = begin(
                    "other-payer",
                    crate::ast::Span {
                        start: 0,
                        end: 0,
                        line: 0,
                        column: 0,
                    },
                    payer,
                    false,
                    Invoice::EMPTY,
                );
                assert!(crate::bounded_output::reserve_active_required(1));
            }
        });
        assert!(!refused);
        assert_eq!(paid, 3);
        let report = capture.finish().unwrap();
        assert_eq!(report.row_count, 3);
        assert!(report.rows[..3]
            .iter()
            .all(|r| r.ledger_entry_remaining.is_none()
                && r.ledger_exit_remaining.is_none()
                && r.ledger_paid.is_none()));
    }
    #[test]
    fn reached_prefix_and_success_have_separate_exact_counts() {
        let capture = Capture::begin(binding()).unwrap();
        {
            let mut guard = invocation("first");
            event(Site::PairPrepared, 0);
            event(Site::Round, 0);
            event(Site::StepAdmitted, 73);
            event(Site::LeftExhausted, 0);
            guard.success();
        }
        {
            let _refused = invocation("refused-before-owner-bank");
        }
        let report = capture.finish().unwrap();
        assert_eq!(report.build, Build::InstrumentedLibraryTest);
        assert!(report.counts_complete && report.detail_complete);
        assert_eq!(
            (
                report.counts.started,
                report.counts.ended,
                report.counts.succeeded
            ),
            (2, 2, 1)
        );
        assert_eq!(report.counts.at(Site::PairPrepared), 1);
        assert_eq!(
            report.counts.admitted_bytes[Site::StepAdmitted as usize],
            73
        );
        assert_eq!(report.rows[1].counts.at(Site::OwnerAdmitted), 0);
        let expected: [u8; 32] = Sha256::digest(b"refused-before-owner-bank").into();
        assert_eq!(report.rows[1].function.sha256, expected);
        assert_eq!(report.rows[0].span.start, 1);
    }
    #[test]
    fn busy_capture_and_unwind_preserve_the_original_operation() {
        let capture = Capture::begin(binding()).unwrap();
        assert!(matches!(Capture::begin(binding()), Err(CaptureError::Busy)));
        let original = std::panic::catch_unwind(|| {
            let _guard = invocation("unwind");
            event(Site::Round, 0);
            panic!("original payload");
        })
        .unwrap_err();
        assert_eq!(original.downcast_ref::<&str>(), Some(&"original payload"));
        let report = capture.finish().unwrap();
        assert!(report.counts_complete);
        assert_eq!(
            (
                report.counts.ended,
                report.counts.unwound,
                report.counts.succeeded
            ),
            (1, 1, 0)
        );
        let generation = report.generation;
        let next = Capture::begin(binding()).unwrap().finish().unwrap();
        assert!(next.generation > generation);
        assert_eq!(next.counts, Counts::EMPTY);
    }
    #[test]
    fn fixed_detail_limit_keeps_aggregate_counts_honest() {
        let capture = Capture::begin(binding()).unwrap();
        for _ in 0..ROW_CAPACITY + 3 {
            let mut guard = invocation("many");
            event(Site::Round, 0);
            guard.success();
        }
        let report = capture.finish().unwrap();
        assert!(!report.detail_complete);
        assert!(report.counts_complete);
        assert_eq!(report.row_count, ROW_CAPACITY);
        assert_eq!(report.counts.started, (ROW_CAPACITY + 3) as u64);
        assert_eq!(report.counts.at(Site::Round), report.counts.started);
        assert_eq!(report.counts.ended, report.counts.started);
    }
    #[test]
    fn long_identity_is_digest_bound_and_never_claimed_complete_prefix() {
        let text = "é".repeat(100);
        let capture = Capture::begin(binding()).unwrap();
        {
            let _guard = invocation(&text);
        }
        let report = capture.finish().unwrap();
        let identity = report.rows[0].function;
        assert_eq!(identity.full_len, 200);
        assert_eq!(identity.prefix_len, 96);
        assert_eq!(&identity.prefix[..], &text.as_bytes()[..96]);
        let expected: [u8; 32] = Sha256::digest(text.as_bytes()).into();
        assert_eq!(identity.sha256, expected);
        drop(text);
        assert_eq!(report.rows[0].function, identity);
    }
    #[test]
    fn overflow_reentry_and_live_finish_refuse_complete_evidence() {
        let capture = Capture::begin(binding()).unwrap();
        access(|s| {
            s.counts.events[Site::Round as usize] = u64::MAX;
        });
        {
            let _guard = invocation("overflow");
            event(Site::Round, 0);
        }
        STATE.with(|s| {
            let _held = s.borrow_mut();
            event(Site::Round, 0);
        });
        let report = capture.finish().unwrap();
        assert!(report.overflow && !report.counts_complete);
        assert_eq!(report.counts.at(Site::Round), u64::MAX);
        let capture = Capture::begin(binding()).unwrap();
        let guard = invocation("live");
        assert!(matches!(
            capture.finish(),
            Err(CaptureError::LiveInvocation)
        ));
        drop(guard);
        assert!(
            Capture::begin(binding())
                .unwrap()
                .finish()
                .unwrap()
                .counts_complete
        );
    }
    #[test]
    fn nested_rows_and_threads_do_not_cross_attribute_events() {
        let capture = Capture::begin(binding()).unwrap();
        {
            let _outer = invocation("outer");
            event(Site::LeftHead, 0);
            {
                let _inner = invocation("inner");
                event(Site::RightHead, 0);
            }
            let child = std::thread::spawn(|| {
                let capture = Capture::begin(binding()).unwrap();
                {
                    let _guard = invocation("child");
                    event(Site::TerminalCompared, 0);
                }
                capture.finish().unwrap()
            })
            .join()
            .unwrap();
            assert_eq!(child.counts.at(Site::TerminalCompared), 1);
        }
        let report = capture.finish().unwrap();
        assert_eq!(report.counts.at(Site::TerminalCompared), 0);
        assert_eq!(report.rows[0].counts.at(Site::RightHead), 0);
        assert_eq!(report.rows[1].counts.at(Site::RightHead), 1);
        assert!(report.counts_complete);
    }
}
