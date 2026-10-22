//! Optional exclusive debit observation. Never allocates or grants compiler authority.
use super::{Budget, ReservationStage, ACTIVE};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub const ATTEMPT_CAPACITY: usize = 16;
const DEPTH: usize = 64;
pub const STAGE_COUNT: usize = ReservationStage::RetainedLoanSidecars as usize + 2;
pub const OPERATION_COUNT: usize = 24;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Operation {
    FunctionDirectory,
    CanonicalLoanStorage,
    CoreValidation,
    ValidatorConstructor,
    FunctionValues,
    AttachedIdentities,
    CleanupInventory,
    CleanupReplay,
    TextRuntime,
    TextContains,
    TextSubstitution,
    ScopeCheckpoint,
    Lifecycle,
    ReplayStructure,
    ReplayExpressionFacts,
    ReplayPathStates,
    ReplayHirSkeleton,
    ReplayPlanSkeleton,
    ReplayFactored,
    ReplayFactoredConstruction,
    ReplayFactoredCanonical,
    ModuleIndex,
    ConstructorSeed,
}
const STAGES: [&str; STAGE_COUNT] = [
    "unscoped",
    "resolved core construction",
    "compact validation inventory construction",
    "uncached synthetic AST peak reservation",
    "authored module signature validation",
    "frontend checked module lookup",
    "frontend selective function resolution",
    "frontend resolved module retention",
    "imported Vec instance inventory",
    "resolved-core prebound",
    "expected workspace edges",
    "resolved call edges",
    "workspace stub signatures",
    "workspace declaration facts",
    "retained workspace facts",
    "resolved module directories",
    "workspace edge proof",
    "synthetic module construction",
    "cached HIR validation",
    "fresh HIR resolution",
    "HIR expression and type lowering",
    "HIR function loan plans",
    "HIR instance loan plans",
    "HIR function cleanup inventories",
    "HIR instance cleanup inventories",
    "HIR canonical cleanup plans",
    "HIR yield profile validation",
    "HIR final validation",
    "canonical shared-loan storage preparation",
    "loan candidate inventory",
    "loan CFG expression discovery",
    "loan CFG point directory construction",
    "loan CFG edge construction",
    "canonical loan directory append",
    "loan ordered directory node allocation",
    "loan vector backing allocation",
    "resolved module retention",
    "loan identity inventory",
    "retained loan scratch allocation",
    "retained loan sidecar capacity",
];
const OPERATIONS: [&str; OPERATION_COUNT] = [
    "unscoped",
    "FunctionDirectory",
    "CanonicalLoanStorage",
    "CoreValidation",
    "ValidatorConstructor",
    "FunctionValues",
    "AttachedIdentities",
    "CleanupInventory",
    "CleanupReplay",
    "TextRuntime",
    "TextContains",
    "TextSubstitution",
    "ScopeCheckpoint",
    "Lifecycle",
    "ReplayStructure",
    "ReplayExpressionFacts",
    "ReplayPathStates",
    "ReplayHirSkeleton",
    "ReplayPlanSkeleton",
    "ReplayFactored",
    "ReplayFactoredConstruction",
    "ReplayFactoredCanonical",
    "ModuleIndex",
    "ConstructorSeed",
];
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome {
    Pending,
    Succeeded,
    Failed,
    Refused,
    Unwound,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Algorithm {
    InstrumentedProductionAlgorithm,
    InstrumentedLibraryTest,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Refusal {
    pub requested: usize,
    pub remaining: usize,
    pub floor: usize,
    pub stage: usize,
    pub operation: usize,
}
#[derive(Clone, Copy, Debug)]
pub struct Attempt {
    pub ordinal: usize,
    pub parent_ordinal: Option<usize>,
    pub mode: u8,
    pub prebound: usize,
    pub layouts: usize,
    pub initial: usize,
    pub remaining: usize,
    pub consumed: usize,
    pub stage_debits: [usize; STAGE_COUNT],
    pub operation_debits: [usize; OPERATION_COUNT],
    /// Immediate-child debits (including their real descendant settlements)
    /// stay separate from this ledger's exclusive sum. Deeper raw events are
    /// not counted again before they settle into that immediate child.
    pub nested_ledger_debits: usize,
    pub foreign_ledger_debits: usize,
    /// Subset of consumed, NOT additional consumption. Exceptional settlement
    /// reports only the actual remaining.min(child_consumed) subtraction.
    pub parent_settlement_debits: usize,
    pub exceptional_parent_settlement_debits: usize,
    pub first_refusal: Option<Refusal>,
    pub outcome: Outcome,
    pub refused: bool,
    pub unwound: bool,
    pub complete: bool,
    pub invalid: bool,
}
impl Attempt {
    const EMPTY: Self = Self {
        ordinal: 0,
        parent_ordinal: None,
        mode: 0,
        prebound: 0,
        layouts: 0,
        initial: 0,
        remaining: 0,
        consumed: 0,
        stage_debits: [0; STAGE_COUNT],
        operation_debits: [0; OPERATION_COUNT],
        nested_ledger_debits: 0,
        foreign_ledger_debits: 0,
        parent_settlement_debits: 0,
        exceptional_parent_settlement_debits: 0,
        first_refusal: None,
        outcome: Outcome::Pending,
        refused: false,
        unwound: false,
        complete: false,
        invalid: false,
    };
    pub fn stage_sum(&self) -> Option<usize> {
        sum(&self.stage_debits)
    }
    pub fn operation_sum(&self) -> Option<usize> {
        sum(&self.operation_debits)
    }
}
fn sum(values: &[usize]) -> Option<usize> {
    values
        .iter()
        .try_fold(0usize, |total, value| total.checked_add(*value))
}
#[derive(Clone, Copy, Debug)]
pub struct Report {
    pub algorithm: Algorithm,
    pub stage_labels: [&'static str; STAGE_COUNT],
    pub operation_labels: [&'static str; OPERATION_COUNT],
    pub attempts: [Attempt; ATTEMPT_CAPACITY],
    pub stored_attempts: usize,
    pub entered_attempts: usize,
    /// Explicit final entered ordinal, including a lost slot; never the largest debit.
    pub final_attempt_ordinal: Option<usize>,
    pub complete: bool,
    pub overflow: bool,
    pub lost: bool,
    pub invalid: bool,
    /// Fixed TLS carrier sizes, not allocator/RSS or semantic-budget usage.
    pub observer_storage_bytes: usize,
    pub report_layout_bytes: usize,
    pub capture_report_layout_bytes: usize,
    pub attempt_guard_bytes: usize,
    pub operation_guard_bytes: usize,
}
impl Report {
    const EMPTY: Self = Self {
        algorithm: if cfg!(test) {
            Algorithm::InstrumentedLibraryTest
        } else {
            Algorithm::InstrumentedProductionAlgorithm
        },
        stage_labels: STAGES,
        operation_labels: OPERATIONS,
        attempts: [Attempt::EMPTY; ATTEMPT_CAPACITY],
        stored_attempts: 0,
        entered_attempts: 0,
        final_attempt_ordinal: None,
        complete: true,
        overflow: false,
        lost: false,
        invalid: false,
        observer_storage_bytes: std::mem::size_of::<RefCell<State>>()
            + std::mem::size_of::<Cell<bool>>(),
        report_layout_bytes: std::mem::size_of::<Report>(),
        capture_report_layout_bytes: std::mem::size_of::<
            crate::workflow_profile::plan_comparison::Report,
        >(),
        attempt_guard_bytes: std::mem::size_of::<AttemptGuard>(),
        operation_guard_bytes: std::mem::size_of::<OperationGuard>(),
    };
}
#[derive(Clone, Copy)]
struct Ledger {
    owner: usize,
    initial: usize,
    remaining: usize,
}
impl Ledger {
    const EMPTY: Self = Self {
        owner: 0,
        initial: 0,
        remaining: 0,
    };
}
#[derive(Clone, Copy)]
struct Frame {
    owner: usize,
    operation: Option<Operation>,
    operation_depth: usize,
}
impl Frame {
    const EMPTY: Self = Self {
        owner: 0,
        operation: None,
        operation_depth: 0,
    };
}
struct State {
    enabled: bool,
    generation: u64,
    report: Report,
    ledgers: [Ledger; DEPTH],
    ledger_depth: usize,
    frames: [Frame; ATTEMPT_CAPACITY],
    stack: [usize; ATTEMPT_CAPACITY],
    depth: usize,
    pending_settlement: Option<(usize, usize)>,
}
impl State {
    const fn new() -> Self {
        Self {
            enabled: false,
            generation: 0,
            report: Report::EMPTY,
            ledgers: [Ledger::EMPTY; DEPTH],
            ledger_depth: 0,
            frames: [Frame::EMPTY; ATTEMPT_CAPACITY],
            stack: [0; ATTEMPT_CAPACITY],
            depth: 0,
            pending_settlement: None,
        }
    }
    fn invalid(&mut self) {
        self.report.invalid = true;
        self.report.complete = false;
    }
    fn add(&mut self, index: usize, coordinate: Option<(bool, usize)>, bytes: usize) {
        let row = &mut self.report.attempts[index];
        let target = match coordinate {
            Some((true, i)) => &mut row.stage_debits[i],
            Some((false, i)) => &mut row.operation_debits[i],
            None => &mut row.nested_ledger_debits,
        };
        if let Some(value) = target.checked_add(bytes) {
            *target = value;
        } else {
            row.invalid = true;
            self.report.overflow = true;
            self.report.complete = false;
        }
    }
}
thread_local! {
    static STATE:RefCell<State>=const{RefCell::new(State::new())};
    static LOST:Cell<bool>=const{Cell::new(false)};
}
fn lost() {
    let _ = LOST.try_with(|cell| cell.set(true));
}
fn observed_active() -> Option<Rc<Budget>> {
    match ACTIVE.try_with(|cell| cell.try_borrow().ok().map(|value| value.clone())) {
        Ok(Some(value)) => value,
        _ => {
            lost();
            None
        }
    }
}
fn access<T>(f: impl FnOnce(&mut State) -> T) -> Option<T> {
    STATE
        .try_with(|cell| match cell.try_borrow_mut() {
            Ok(mut state) => Some(f(&mut state)),
            Err(_) => {
                lost();
                None
            }
        })
        .ok()
        .flatten()
}
#[inline(never)]
pub(crate) fn begin(generation: u64) {
    let _ = LOST.try_with(|cell| cell.set(false));
    access(|s| {
        // Reset the existing fixed TLS bank in place. Do not build another
        // whole State/Report on the caller's ordinary unoptimized stack.
        s.report.attempts.fill(Attempt::EMPTY);
        s.report.stored_attempts = 0;
        s.report.entered_attempts = 0;
        s.report.final_attempt_ordinal = None;
        s.report.complete = true;
        s.report.overflow = false;
        s.report.lost = false;
        s.report.invalid = false;
        s.ledgers.fill(Ledger::EMPTY);
        s.frames.fill(Frame::EMPTY);
        s.stack.fill(0);
        s.ledger_depth = 0;
        s.depth = 0;
        s.pending_settlement = None;
        s.generation = generation;
        s.enabled = true;
    });
    transition();
}
pub(crate) fn cancel(generation: u64) {
    access(|s| {
        if s.generation == generation {
            s.enabled = false;
        }
    });
}
#[inline(never)]
pub(crate) fn finish(generation: u64) -> Report {
    match access(|s| {
        if !s.enabled || s.generation != generation {
            s.invalid();
        }
        if s.depth != 0 || s.pending_settlement.is_some() {
            s.invalid();
        }
        s.enabled = false;
        s.report.lost = LOST.try_with(Cell::get).unwrap_or(true);
        s.report.complete &= !s.report.overflow && !s.report.lost && !s.report.invalid;
        s.report
    }) {
        Some(report) => report,
        None => Report {
            complete: false,
            lost: true,
            invalid: true,
            ..Report::EMPTY
        },
    }
}
// Called after the existing ACTIVE install/restore, before parent settlement.
// The complete old budget algorithm decides the topology; addresses only bind
// telemetry to that topology and are never accepted by a compiler operation.
pub(crate) fn transition() {
    let current = observed_active();
    access(|s| {
        if !s.enabled {
            return;
        }
        let Some(budget) = current else {
            s.ledger_depth = 0;
            s.pending_settlement = None;
            return;
        };
        let owner = Rc::as_ptr(&budget) as usize;
        if let Some(index) = s.ledgers[..s.ledger_depth]
            .iter()
            .position(|x| x.owner == owner)
        {
            if index + 1 < s.ledger_depth {
                // One actual Restore settles its direct child. No debit is
                // manufactured here: only the later real subtraction counts.
                let child = s.ledgers[index + 1];
                if index + 2 != s.ledger_depth || s.pending_settlement.is_some() {
                    s.invalid();
                }
                match child.initial.checked_sub(child.remaining) {
                    Some(bytes) => s.pending_settlement = Some((owner, bytes)),
                    None => s.invalid(),
                }
            }
            s.ledger_depth = index + 1;
            s.ledgers[index].remaining = budget.remaining.get();
        } else if s.ledger_depth < DEPTH {
            s.ledgers[s.ledger_depth] = Ledger {
                owner,
                initial: budget.remaining.get(),
                remaining: budget.remaining.get(),
            };
            s.ledger_depth += 1;
        } else {
            s.report.overflow = true;
            s.invalid();
        }
    });
}
pub(super) fn debit(budget: &Budget, bytes: usize) {
    record_debit(budget, bytes, false);
}
pub(super) fn exceptional_settlement(budget: &Budget, bytes: usize) {
    record_debit(budget, bytes, true);
}
fn record_debit(budget: &Budget, bytes: usize, exceptional: bool) {
    let owner = budget as *const Budget as usize;
    access(|s| {
        if !s.enabled {
            return;
        }
        let ledger = s.ledgers[..s.ledger_depth]
            .iter()
            .position(|x| x.owner == owner);
        if let Some(index) = ledger {
            s.ledgers[index].remaining = budget.remaining.get();
        }
        let settlement = match s.pending_settlement {
            Some((payer, expected)) if payer == owner => {
                s.pending_settlement = None;
                if bytes > expected {
                    s.invalid();
                }
                true
            }
            _ => false,
        };
        if exceptional && !settlement {
            s.invalid();
        }
        for depth in 0..s.depth {
            let index = s.stack[depth];
            let frame = s.frames[index];
            if frame.owner == owner {
                let stage = budget.stage.get().map_or(0, |stage| stage as usize + 1);
                let operation = frame
                    .operation
                    .map_or(0, |operation| operation as usize + 1);
                s.add(index, Some((true, stage)), bytes);
                s.add(index, Some((false, operation)), bytes);
                if exceptional {
                    let row = &mut s.report.attempts[index];
                    if let Some(value) = row.exceptional_parent_settlement_debits.checked_add(bytes)
                    {
                        row.exceptional_parent_settlement_debits = value;
                    } else {
                        row.invalid = true;
                        s.report.overflow = true;
                        s.report.complete = false;
                    }
                }
                if settlement {
                    let row = &mut s.report.attempts[index];
                    if let Some(value) = row.parent_settlement_debits.checked_add(bytes) {
                        row.parent_settlement_debits = value;
                    } else {
                        row.invalid = true;
                        s.report.overflow = true;
                        s.report.complete = false;
                    }
                }
            } else {
                let root = s.ledgers[..s.ledger_depth]
                    .iter()
                    .position(|x| x.owner == frame.owner);
                match root.zip(ledger) {
                    Some((a, b)) if b == a + 1 => s.add(index, None, bytes),
                    Some((a, b)) if b > a => {} // only the eventual immediate-child settlement counts
                    _ => {
                        let row = &mut s.report.attempts[index];
                        if let Some(value) = row.foreign_ledger_debits.checked_add(bytes) {
                            row.foreign_ledger_debits = value;
                        } else {
                            row.invalid = true;
                            s.report.overflow = true;
                            s.report.complete = false;
                        }
                    }
                }
            }
        }
    });
}
pub(super) fn refusal(budget: &Budget, requested: usize) {
    let owner = budget as *const Budget as usize;
    access(|s| {
        if !s.enabled {
            return;
        }
        for depth in 0..s.depth {
            let index = s.stack[depth];
            let frame = s.frames[index];
            let row = &mut s.report.attempts[index];
            if frame.owner == owner && row.first_refusal.is_none() {
                row.first_refusal = Some(Refusal {
                    requested,
                    remaining: budget.remaining.get(),
                    floor: budget.floor.get(),
                    stage: budget.stage.get().map_or(0, |v| v as usize + 1),
                    operation: frame.operation.map_or(0, |v| v as usize + 1),
                });
            }
        }
    });
}
pub(crate) struct AttemptGuard {
    registered: Option<(u64, usize, usize)>,
    outcome: Outcome,
}
pub(crate) fn attempt(prebound: usize, layouts: usize, mode: u8) -> AttemptGuard {
    let budget = observed_active();
    let registered = access(|s| {
        if !s.enabled {
            return None;
        }
        let ordinal = s.report.entered_attempts;
        let Some(next) = ordinal.checked_add(1) else {
            s.report.overflow = true;
            s.invalid();
            return None;
        };
        s.report.entered_attempts = next;
        s.report.final_attempt_ordinal = Some(ordinal);
        let Some(budget) = budget else {
            s.invalid();
            return None;
        };
        if s.report.stored_attempts == ATTEMPT_CAPACITY || s.depth == ATTEMPT_CAPACITY {
            s.report.overflow = true;
            s.report.complete = false;
            return None;
        }
        let index = s.report.stored_attempts;
        s.report.stored_attempts += 1;
        let parent_ordinal = if s.depth == 0 {
            None
        } else {
            Some(s.report.attempts[s.stack[s.depth - 1]].ordinal)
        };
        s.report.attempts[index] = Attempt {
            ordinal,
            parent_ordinal,
            mode,
            prebound,
            layouts,
            initial: budget.remaining.get(),
            remaining: budget.remaining.get(),
            ..Attempt::EMPTY
        };
        s.frames[index] = Frame {
            owner: Rc::as_ptr(&budget) as usize,
            ..Frame::EMPTY
        };
        s.stack[s.depth] = index;
        s.depth += 1;
        Some((s.generation, index, s.depth))
    })
    .flatten();
    AttemptGuard {
        registered,
        outcome: Outcome::Pending,
    }
}
impl AttemptGuard {
    pub(crate) fn complete(&mut self, success: bool) {
        self.outcome = if success {
            Outcome::Succeeded
        } else {
            Outcome::Failed
        };
    }
}
impl Drop for AttemptGuard {
    fn drop(&mut self) {
        let Some((generation, index, depth)) = self.registered else {
            return;
        };
        let budget = observed_active();
        let unwound = std::thread::panicking();
        access(|s| {
            if !s.enabled || s.generation != generation {
                lost();
                return;
            }
            let ordered = s.depth == depth
                && s.stack[depth - 1] == index
                && s.frames[index].operation_depth == 0;
            let owner = s.frames[index].owner;
            let row = &mut s.report.attempts[index];
            row.unwound = unwound;
            row.outcome = if unwound {
                Outcome::Unwound
            } else {
                self.outcome
            };
            if let Some(budget) = budget.filter(|b| Rc::as_ptr(b) as usize == owner) {
                row.remaining = budget.remaining.get();
                row.refused = budget.overflowed.get();
                if row.refused && !unwound {
                    row.outcome = Outcome::Refused;
                }
                match row.initial.checked_sub(row.remaining) {
                    Some(bytes) => row.consumed = bytes,
                    None => row.invalid = true,
                }
            } else {
                row.invalid = true;
            }
            row.invalid |= !ordered
                || row.stage_sum() != Some(row.consumed)
                || row.operation_sum() != Some(row.consumed);
            row.complete = !row.invalid && row.outcome != Outcome::Pending;
            s.report.invalid |= row.invalid;
            s.report.complete &= row.complete;
            if s.depth == depth {
                s.depth -= 1;
            } else {
                s.invalid();
            }
        });
    }
}
pub(crate) struct OperationGuard {
    registered: Option<(u64, usize, usize, Option<Operation>)>,
}
pub(crate) fn operation_guard(operation: Operation) -> OperationGuard {
    let owner = observed_active().map(|b| Rc::as_ptr(&b) as usize);
    let registered = access(|s| {
        if !s.enabled || s.depth == 0 {
            return None;
        }
        let index = s.stack[s.depth - 1];
        let frame = &mut s.frames[index];
        if owner != Some(frame.owner) {
            return None;
        }
        let Some(depth) = frame.operation_depth.checked_add(1) else {
            s.report.overflow = true;
            s.invalid();
            return None;
        };
        let previous = frame.operation.replace(operation);
        frame.operation_depth = depth;
        Some((s.generation, index, depth, previous))
    })
    .flatten();
    OperationGuard { registered }
}
impl Drop for OperationGuard {
    fn drop(&mut self) {
        let Some((generation, index, depth, previous)) = self.registered else {
            return;
        };
        access(|s| {
            if !s.enabled || s.generation != generation {
                lost();
                return;
            }
            let frame = &mut s.frames[index];
            if frame.operation_depth != depth {
                s.invalid();
                return;
            }
            frame.operation = previous;
            frame.operation_depth -= 1;
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::bounded_output::{
        reserve_active_required, with_limit_usage_refusal, with_reservation_stage,
    };
    use crate::workflow_profile::plan_comparison::{Binding, Capture};
    use std::panic::{catch_unwind, AssertUnwindSafe};
    fn capture() -> Capture {
        Capture::begin(Binding::caller_labelled(
            "exclusive actual ledger control",
            [0; 32],
        ))
        .unwrap()
    }
    fn scenario(
        observe: bool,
        limit: usize,
        refused: bool,
        unwind: bool,
    ) -> (
        bool,
        usize,
        Option<super::super::ReservationRefusal>,
        Option<Report>,
    ) {
        let capture = observe.then(capture);
        let observed = with_limit_usage_refusal(limit, || {
            let mut attempt = attempt(17, 19, 6);
            let _old = crate::bounded_output::stage_debits::start(17, 19, 6);
            {
                let _operation = operation_guard(Operation::CoreValidation);
                with_reservation_stage(ReservationStage::FreshResolution, || {
                    if refused {
                        assert!(crate::bounded_output::set_active_floor(
                            limit.saturating_sub(1)
                        ));
                    }
                    let panic = catch_unwind(AssertUnwindSafe(|| {
                        with_limit_usage_refusal(3, || {
                            assert!(reserve_active_required(2.min(limit)));
                            if unwind {
                                std::panic::panic_any(17usize);
                            }
                        });
                    }));
                    assert_eq!(panic.is_err(), unwind);
                    if refused {
                        assert!(!reserve_active_required(0));
                    } else {
                        let _ = reserve_active_required(4);
                    }
                });
            }
            attempt.complete(!crate::bounded_output::active_refused());
        });
        let old = crate::bounded_output::stage_debits::take_last_completed().unwrap();
        assert_eq!(
            (old.sum(), old.operation_debits.sum()),
            (observed.2, observed.2)
        );
        let report = capture.map(|c| c.finish().unwrap().resolved_core);
        (observed.1, observed.2, observed.3, report)
    }
    #[test]
    fn exclusive_core_attempts_keep_original_ledger_refusal_nested_settlement_and_fixed_zero_allocation(
    ) {
        for (limit, refused, unwind) in [
            (10, false, false),
            (6, false, false),
            (5, false, false),
            (0, false, false),
            (10, true, false),
            (10, false, true),
        ] {
            exclusive_original_ledger_case(limit, refused, unwind);
        }
        exclusive_zero_allocation_case();
    }
    #[inline(never)]
    fn exclusive_original_ledger_case(limit: usize, refused: bool, unwind: bool) {
        let plain = scenario(false, limit, refused, unwind);
        let watched = scenario(true, limit, refused, unwind);
        assert_eq!(
            (plain.0, plain.1, plain.2),
            (watched.0, watched.1, watched.2)
        );
        let r = watched.3.unwrap();
        assert!(r.complete && !r.overflow && !r.lost && !r.invalid, "{r:?}");
        assert_eq!(
            (
                r.entered_attempts,
                r.stored_attempts,
                r.final_attempt_ordinal
            ),
            (1, 1, Some(0))
        );
        let a = r.attempts[0];
        assert!(a.complete && !a.invalid);
        assert_eq!(
            (a.consumed, a.stage_sum(), a.operation_sum()),
            (watched.1, Some(watched.1), Some(watched.1))
        );
        assert_eq!(
            a.operation_debits[Operation::CoreValidation as usize + 1],
            a.consumed
        );
        assert_eq!(a.parent_settlement_debits, a.nested_ledger_debits);
        assert_eq!(
            a.exceptional_parent_settlement_debits,
            if refused {
                a.parent_settlement_debits
            } else {
                0
            }
        );
        assert_eq!(
            a.first_refusal.map(|x| (x.requested, x.remaining, x.floor)),
            watched.2.map(|x| (x.requested, x.remaining, x.floor))
        );
        assert_eq!((a.prebound, a.layouts, a.mode), (17, 19, 6));
        assert_eq!(a.refused, watched.0);
    }
    #[inline(never)]
    fn exclusive_zero_allocation_case() {
        let c = capture();
        with_limit_usage_refusal(10, || {
            let (_, metrics) = crate::test_allocation_observer::measure(|| {
                let mut a = attempt(0, 0, 0);
                {
                    let _operation = operation_guard(Operation::ModuleIndex);
                    assert!(reserve_active_required(2));
                }
                {
                    let _operation = operation_guard(Operation::ConstructorSeed);
                    assert!(reserve_active_required(3));
                }
                a.complete(true);
            });
            assert!(!metrics.invalid);
            assert_eq!(
                (
                    metrics.allocations,
                    metrics.reallocations,
                    metrics.allocated,
                    metrics.live
                ),
                (0, 0, 0, 0)
            );
        });
        let r = c.finish().unwrap().resolved_core;
        assert!(r.complete);
        assert_eq!(r.attempts[0].consumed, 5);
        assert_eq!(
            r.attempts[0].operation_debits[Operation::ModuleIndex as usize + 1],
            2
        );
        assert_eq!(
            r.attempts[0].operation_debits[Operation::ConstructorSeed as usize + 1],
            3
        );
        assert_eq!(
            r.observer_storage_bytes,
            std::mem::size_of::<RefCell<State>>() + std::mem::size_of::<Cell<bool>>()
        );
        assert_eq!(r.report_layout_bytes, std::mem::size_of::<Report>());
        assert_eq!(
            r.capture_report_layout_bytes,
            std::mem::size_of::<crate::workflow_profile::plan_comparison::Report>()
        );
    }
    #[test]
    fn exclusive_core_attempts_keep_final_not_largest_retries_foreign_restored_unwind_drop_and_loss(
    ) {
        exclusive_case_0();
        exclusive_case_1();
        exclusive_case_2();
        exclusive_case_3();
        exclusive_case_4();
        exclusive_case_5();
        exclusive_case_6();
        exclusive_case_7();
        // Preserve the entire independent old observer and all its original tests.
        use sha2::Digest;
        let old: [u8; 32] = sha2::Sha256::digest(include_bytes!("stage_debits.rs")).into();
        let tests: [u8; 32] = sha2::Sha256::digest(include_bytes!("stage_debits/tests.rs")).into();
        assert_eq!(old, OLD_OBSERVER_SHA);
        assert_eq!(tests, OLD_TESTS_SHA);
    }
    #[inline(never)]
    fn exclusive_case_0() {
        let c = capture();
        for (mode, bytes) in [(0, 7), (1, 3)] {
            with_limit_usage_refusal(10, || {
                let mut a = attempt(bytes, 0, mode);
                assert!(reserve_active_required(bytes));
                a.complete(true);
            });
        }
        let r = c.finish().unwrap().resolved_core;
        assert!(r.complete);
        assert_eq!(r.final_attempt_ordinal, Some(1));
        assert_eq!((r.attempts[0].consumed, r.attempts[1].consumed), (7, 3));
    }
    #[inline(never)]
    fn exclusive_case_1() {
        let c = capture();
        with_limit_usage_refusal(20, || {
            let foreign = super::super::active().unwrap();
            with_limit_usage_refusal(10, || {
                let mut outer = attempt(0, 0, 0);
                assert!(super::super::reserve(Some(&foreign), 3));
                {
                    let _op = operation_guard(Operation::CoreValidation);
                    with_limit_usage_refusal(3, || {
                        let mut inner = attempt(0, 0, 1);
                        assert!(reserve_active_required(2));
                        inner.complete(true);
                    });
                }
                assert!(reserve_active_required(1));
                outer.complete(true);
            });
        });
        let r = c.finish().unwrap().resolved_core;
        assert!(r.complete, "{r:?}");
        assert_eq!(r.attempts[0].consumed, 3);
        assert_eq!(r.attempts[0].foreign_ledger_debits, 3);
        assert_eq!(r.attempts[0].nested_ledger_debits, 2);
        assert_eq!(r.attempts[0].parent_settlement_debits, 2);
        assert_eq!(r.attempts[1].parent_ordinal, Some(0));
        assert_eq!(r.attempts[1].consumed, 2);
    }
    #[inline(never)]
    fn exclusive_case_2() {
        let c = capture();
        with_limit_usage_refusal(20, || {
            let mut a = attempt(0, 0, 0);
            with_limit_usage_refusal(10, || {
                with_limit_usage_refusal(5, || assert!(reserve_active_required(2)));
                assert!(reserve_active_required(1));
            });
            assert!(reserve_active_required(1));
            a.complete(true);
        });
        let r = c.finish().unwrap().resolved_core;
        assert!(r.complete);
        assert_eq!(r.attempts[0].consumed, 4);
        assert_eq!(r.attempts[0].nested_ledger_debits, 3);
        assert_eq!(r.attempts[0].parent_settlement_debits, 3);
    }
    #[inline(never)]
    fn exclusive_case_3() {
        struct Owner;
        impl Drop for Owner {
            fn drop(&mut self) {
                assert!(reserve_active_required(1));
            }
        }
        let c = capture();
        let panic = catch_unwind(AssertUnwindSafe(|| {
            with_limit_usage_refusal(10, || {
                let _a = attempt(0, 0, 0);
                let _owner = Owner;
                assert!(reserve_active_required(2));
                std::panic::panic_any(29usize);
            })
        }));
        assert!(panic.is_err());
        let r = c.finish().unwrap().resolved_core;
        assert!(r.complete);
        assert_eq!(r.attempts[0].outcome, Outcome::Unwound);
        assert!(r.attempts[0].unwound);
        assert_eq!(r.attempts[0].consumed, 3);
    }
    #[inline(never)]
    fn exclusive_case_4() {
        let c = capture();
        for _ in 0..=ATTEMPT_CAPACITY {
            with_limit_usage_refusal(0, || {
                let mut a = attempt(0, 0, 0);
                a.complete(true);
            });
        }
        let r = c.finish().unwrap().resolved_core;
        assert!(r.overflow && !r.complete);
        assert_eq!(r.final_attempt_ordinal, Some(ATTEMPT_CAPACITY));
        assert_eq!(r.entered_attempts, ATTEMPT_CAPACITY + 1);
    }
    #[inline(never)]
    fn exclusive_case_5() {
        let c = capture();
        with_limit_usage_refusal(2, || {
            let mut a = attempt(0, 0, 0);
            STATE.with(|s| {
                let _borrow = s.borrow_mut();
                let _lost = operation_guard(Operation::CoreValidation);
            });
            assert!(reserve_active_required(1));
            a.complete(true);
        });
        let r = c.finish().unwrap().resolved_core;
        assert!(r.lost && !r.complete);
    }
    #[inline(never)]
    fn exclusive_case_6() {
        let c = capture();
        let (_, refused, used, _) = with_limit_usage_refusal(2, || {
            let mut a = attempt(0, 0, 0);
            access(|s| s.report.attempts[0].stage_debits[0] = usize::MAX);
            assert!(reserve_active_required(1));
            a.complete(true);
        });
        assert!(!refused && used == 1);
        let r = c.finish().unwrap().resolved_core;
        assert!(r.overflow && r.invalid && !r.complete);
    }
    #[inline(never)]
    fn exclusive_case_7() {
        let c = capture();
        assert!(super::super::active_remaining().is_none());
        let mut unknown = attempt(0, 0, 0);
        unknown.complete(false);
        drop(unknown);
        let r = c.finish().unwrap().resolved_core;
        assert!(r.invalid && !r.complete);
    }
    const OLD_OBSERVER_SHA: [u8; 32] = [
        16, 213, 138, 22, 236, 137, 67, 246, 93, 84, 181, 111, 54, 92, 147, 201, 237, 218, 64, 41,
        52, 8, 104, 27, 167, 229, 128, 193, 60, 79, 51, 30,
    ];
    const OLD_TESTS_SHA: [u8; 32] = [
        216, 24, 146, 225, 204, 167, 135, 142, 173, 253, 116, 116, 62, 107, 70, 16, 136, 67, 18,
        224, 138, 68, 107, 83, 194, 255, 172, 30, 103, 192, 90, 6,
    ];
}
