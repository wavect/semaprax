//! Test-only, allocation-free debit attribution for one exact resolved-core
//! ledger. Counters describe reservations, never physical allocation or refunds.
use super::{active, Budget, ReservationStage};
pub(crate) mod route_debits;
use std::cell::Cell;
use std::rc::Rc;

/// Fixed diagnostic coordinates only; no allocation, source key or authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Operation {
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
}
// This family is inclusive only for the separate route observer. The operation
// totals below still attribute each committed debit to one exclusive child.
fn cleanup_family(operation: Option<Operation>) -> bool {
    match operation {
        Some(
            Operation::CleanupReplay
            | Operation::ReplayStructure
            | Operation::ReplayExpressionFacts
            | Operation::ReplayPathStates
            | Operation::ReplayHirSkeleton
            | Operation::ReplayPlanSkeleton
            | Operation::ReplayFactored
            | Operation::ReplayFactoredConstruction
            | Operation::ReplayFactoredCanonical,
        ) => true,
        None
        | Some(
            Operation::FunctionDirectory
            | Operation::CanonicalLoanStorage
            | Operation::CoreValidation
            | Operation::ValidatorConstructor
            | Operation::FunctionValues
            | Operation::AttachedIdentities
            | Operation::CleanupInventory
            | Operation::TextRuntime
            | Operation::TextContains
            | Operation::TextSubstitution
            | Operation::ScopeCheckpoint
            | Operation::Lifecycle,
        ) => false,
    }
}
// Exhaustive matching makes a new operation require its own distinct counter.
// These are exclusive scopes: child operations do not also increment parents.
macro_rules! operation_totals {
    ($($operation:ident => $field:ident),+ $(,)?) => {
        #[derive(Clone, Copy)]
        pub(crate) struct OperationTotals {
            unscoped: usize,
            $($field: usize,)+
        }
        impl OperationTotals {
            const EMPTY: Self = Self { unscoped: 0, $($field: 0,)+ };
            fn add(&mut self, operation: Option<Operation>, bytes: usize) {
                let counter = match operation {
                    None => &mut self.unscoped,
                    $(Some(Operation::$operation) => &mut self.$field,)+
                };
                *counter = counter.checked_add(bytes)
                    .expect("operation debit overflow cannot exceed the finite budget");
            }
            pub(crate) fn get(self, operation: Option<Operation>) -> usize {
                match operation {
                    None => self.unscoped,
                    $(Some(Operation::$operation) => self.$field,)+
                }
            }
            pub(crate) fn sum(self) -> usize { self.unscoped $(+ self.$field)+ }
        }
        impl std::fmt::Debug for OperationTotals {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let mut entries = f.debug_map();
                if self.unscoped != 0 { entries.entry(&"unscoped", &self.unscoped); }
                $(if self.$field != 0 { entries.entry(&stringify!($field), &self.$field); })+
                entries.finish()
            }
        }
    };
}
operation_totals! {
    FunctionDirectory => function_directory,
    CanonicalLoanStorage => canonical_loan_storage,
    CoreValidation => core_validation,
    ValidatorConstructor => validator_constructor,
    FunctionValues => function_values,
    AttachedIdentities => attached_identities,
    CleanupInventory => cleanup_inventory,
    CleanupReplay => cleanup_replay,
    TextRuntime => text_runtime,
    TextContains => text_contains,
    TextSubstitution => text_substitution,
    ScopeCheckpoint => scope_checkpoint,
    Lifecycle => lifecycle,
    ReplayStructure => replay_structure,
    ReplayExpressionFacts => replay_expression_facts,
    ReplayPathStates => replay_path_states,
    ReplayHirSkeleton => replay_hir_skeleton,
    ReplayPlanSkeleton => replay_plan_skeleton,
    ReplayFactored => replay_factored,
    ReplayFactoredConstruction => replay_factored_construction,
    ReplayFactoredCanonical => replay_factored_canonical,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OperationRefusal {
    pub(crate) operation: Option<Operation>,
    pub(crate) requested: usize,
    pub(crate) remaining: usize,
    pub(crate) floor: usize,
    pub(crate) stage: Option<ReservationStage>,
}
const COUNT: usize = ReservationStage::RetainedLoanSidecars as usize + 2;
#[derive(Clone, Copy, Debug)]
pub(crate) struct Trace {
    owner: usize,
    operation: Option<Operation>,
    pub(crate) first_operation_refusal: Option<OperationRefusal>,
    pub(crate) first_site: Option<super::ReservationSite>,
    pub(crate) initial: usize,
    pub(crate) consumed: usize,
    pub(crate) prebound: usize,
    pub(crate) layouts: usize,
    pub(crate) mode: u8,
    pub(crate) debits: [usize; COUNT],
    pub(crate) operation_debits: OperationTotals,
}
impl Trace {
    const EMPTY: Self = Self {
        owner: 0,
        operation: None,
        first_operation_refusal: None,
        first_site: None,
        initial: 0,
        consumed: 0,
        prebound: 0,
        layouts: 0,
        mode: 0,
        debits: [0; COUNT],
        operation_debits: OperationTotals::EMPTY,
    };
    pub(crate) fn sum(self) -> usize {
        self.debits.into_iter().sum()
    }
}
thread_local! {
    static CURRENT: Cell<Trace> = const { Cell::new(Trace::EMPTY) };
    static COMPLETED: Cell<Option<Trace>> = const { Cell::new(None) };
    static LAST_COMPLETED: Cell<Option<Trace>> = const { Cell::new(None) };
}
// No Budget field or allocation: address equality only selects this diagnostic
// observation scope and grants no source, identity, or budget authority.
pub(super) fn debit(budget: &Budget, bytes: usize) {
    CURRENT.with(|current| {
        let mut trace = current.get();
        if trace.owner == budget as *const Budget as usize {
            route_debits::debit(
                trace.owner,
                bytes,
                budget.stage.get() == Some(ReservationStage::HirValidation),
                cleanup_family(trace.operation),
            );
            let ordinal = budget.stage.get().map_or(0, |stage| stage as usize + 1);
            trace.debits[ordinal] = trace.debits[ordinal]
                .checked_add(bytes)
                .expect("stage debit overflow cannot exceed the finite budget");
            trace.operation_debits.add(trace.operation, bytes);
            current.set(trace);
        }
    });
}
/// The existing budget Rc is held solely to prevent address reuse while this
/// observation guard lives. It cannot acquire storage or change budget state.
pub(crate) struct OperationGuard {
    owner: Option<Rc<Budget>>,
    previous: Option<Operation>,
}
pub(crate) fn operation_guard(operation: Operation) -> OperationGuard {
    let mut previous = None;
    let owner = active().filter(|budget| {
        CURRENT.with(|current| {
            let mut trace = current.get();
            if trace.owner != Rc::as_ptr(budget) as usize {
                return false;
            }
            previous = trace.operation.replace(operation);
            current.set(trace);
            true
        })
    });
    OperationGuard { owner, previous }
}
impl Drop for OperationGuard {
    fn drop(&mut self) {
        if let Some(owner) = &self.owner {
            CURRENT.with(|current| {
                let mut trace = current.get();
                if trace.owner == Rc::as_ptr(owner) as usize {
                    trace.operation = self.previous;
                    current.set(trace);
                }
            });
        }
    }
}
// Called only at the original first-refusal write. Nested/foreign budgets do
// not enter this exact core receipt; their original settlement still runs.
pub(super) fn refusal(budget: &Budget, requested: usize) {
    CURRENT.with(|current| {
        let mut trace = current.get();
        if trace.owner == budget as *const Budget as usize
            && trace.first_operation_refusal.is_none()
        {
            route_debits::refusal(
                trace.owner,
                budget.stage.get() == Some(ReservationStage::HirValidation)
                    && cleanup_family(trace.operation),
            );
            trace.first_operation_refusal = Some(OperationRefusal {
                operation: trace.operation,
                requested,
                remaining: budget.remaining.get(),
                floor: budget.floor.get(),
                stage: budget.stage.get(),
            });
            current.set(trace);
        }
    });
}
pub(crate) struct Guard {
    previous: Trace,
}
pub(crate) fn start(prebound: usize, layouts: usize, mode: u8) -> Guard {
    let budget = active().expect("resolved core has its existing budget");
    let trace = Trace {
        owner: Rc::as_ptr(&budget) as usize,
        initial: budget.remaining.get(),
        prebound,
        layouts,
        mode,
        ..Trace::EMPTY
    };
    route_debits::start(trace.owner, prebound, mode);
    let previous = CURRENT.with(|current| current.replace(trace));
    Guard { previous }
}
impl Drop for Guard {
    fn drop(&mut self) {
        let mut trace = CURRENT.with(|current| current.replace(self.previous));
        let budget = active().expect("core trace settles before its ledger");
        assert_eq!(trace.owner, Rc::as_ptr(&budget) as usize);
        // Static first-site evidence only, after the existing exact owner check.
        trace.first_site = budget.first_site.get();
        trace.consumed = trace.initial - budget.remaining.get();
        assert_eq!(
            trace.sum(),
            trace.consumed,
            "every committed debit exactly once"
        );
        assert_eq!(
            trace.operation_debits.sum(),
            trace.consumed,
            "every committed operation debit exactly once"
        );
        route_debits::finish(
            trace.owner,
            trace.consumed,
            trace.debits[ReservationStage::HirValidation as usize + 1],
        );
        LAST_COMPLETED.with(|completed| completed.set(Some(trace)));
        COMPLETED.with(|completed| {
            if completed
                .get()
                .is_none_or(|old| trace.consumed > old.consumed)
            {
                completed.set(Some(trace));
            }
        });
    }
}
pub(crate) fn take_completed() -> Option<Trace> {
    COMPLETED.with(|completed| completed.take())
}
/// Final attempt, separately from the historical maximum-consumed attempt.
/// Observation only: neither receipt changes the ordinary budget or retry path.
pub(crate) fn take_last_completed() -> Option<Trace> {
    LAST_COMPLETED.with(|completed| completed.take())
}

#[cfg(test)]
mod tests;
