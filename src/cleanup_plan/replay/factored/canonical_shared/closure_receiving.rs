//! One invocation-paid success/failure destination for cold closure producers.
use super::{ordered_weights, Diagnostic, Rc, ReplayBudget, ResolvedFunction};
use crate::bounded_output::classifier_string::ClassifierLedger;
use std::mem::size_of;
type Value = Option<Rc<ordered_weights::OrderedWeights>>;
pub(in crate::cleanup_plan::replay::factored) struct ClosureReceiving {
    pub(super) value: Value,
    pub(super) refusal: Option<Diagnostic>,
    pub(super) ready: bool,
    pub(super) overflow: bool,
    pub(super) empty: bool,
    failed: bool,
    function: *const ResolvedFunction,
    budget: *const ReplayBudget,
    ledger: ClassifierLedger,
}
impl ClosureReceiving {
    // Direct aggregate initializer + receiving owner; no Result<Self> exists.
    // named Option initializers, held Diagnostic receiving and bootstrap Result.
    pub(in crate::cleanup_plan::replay::factored) const DIRECT_CONTROLS: usize = 2
        * size_of::<Self>()
        + size_of::<Option<Diagnostic>>()
        + size_of::<Diagnostic>()
        + size_of::<Value>()
        + size_of::<Result<(), Diagnostic>>();
    // The complete old Result-returning constructor stays independently usable
    // by its original controls. Production direct construction makes no such Result.
    #[cfg(test)]
    pub(super) const CONTROLS: usize = 2 * size_of::<Self>()
        + size_of::<Result<Self, Diagnostic>>()
        + size_of::<Option<Diagnostic>>()
        + size_of::<Diagnostic>()
        + size_of::<Value>()
        + size_of::<Result<(), Diagnostic>>();
    pub(in crate::cleanup_plan::replay::factored) fn from_prepaid(
        function: &ResolvedFunction,
        budget: &ReplayBudget,
    ) -> Self {
        Self {
            value: None,
            refusal: None,
            ready: false,
            overflow: false,
            empty: false,
            failed: false,
            function: std::ptr::from_ref(function),
            budget: std::ptr::from_ref(budget),
            ledger: ClassifierLedger::capture(),
        }
    }
    // The unit producer borrows this owner. Its small success argument and
    // take/reset transport remain actual constructors; only the large inner
    // Result disappears. The outer Result and all nested Results still exist.
    #[cfg(test)]
    pub(super) const PER_MISS_CONTROLS: usize = size_of::<&mut Self>() + 3 * size_of::<Value>();
    pub(super) const QUERY_CONTROLS: usize = size_of::<&mut Self>();
    #[cfg(test)]
    pub(in crate::cleanup_plan::replay::factored) fn acquire(
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Self, Diagnostic> {
        if budget.pays_allocations() {
            budget.reserve_bytes(
                function,
                Self::CONTROLS,
                "factored held closure invocation controls",
            )?;
        }
        Ok(Self {
            value: None,
            refusal: None,
            ready: false,
            overflow: false,
            empty: false,
            failed: false,
            function: std::ptr::from_ref(function),
            budget: std::ptr::from_ref(budget),
            ledger: ClassifierLedger::capture(),
        })
    }
    pub(super) fn valid(&mut self, function: &ResolvedFunction, budget: &ReplayBudget) -> bool {
        if self.failed {
            return false;
        }
        // Inspect the held ledger before comparing pointers, exactly as Memo.
        if !self.ledger.current()
            || self.function != std::ptr::from_ref(function)
            || self.budget != std::ptr::from_ref(budget)
        {
            self.failed = true;
            return false;
        }
        true
    }
    pub(super) fn begin(&mut self) {
        self.overflow = false;
        self.empty = false;
        assert!(
            !self.ready && self.value.is_none() && self.refusal.is_none(),
            "previous receiving outcome consumed"
        );
    }
    #[cfg(test)]
    pub(super) fn complete(&mut self, value: Value) {
        assert!(!self.ready && self.refusal.is_none());
        self.value = value;
        self.ready = true;
    }
    pub(super) fn finish_overflow(&mut self) {
        self.overflow = true;
        self.ready = true;
    }
    pub(super) fn refuse(&mut self, error: Diagnostic) {
        assert!(!self.ready && self.refusal.is_none());
        self.refusal = Some(error);
        self.ready = true;
    }
}
#[cfg(test)]
thread_local! { static PRODUCERS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
#[cfg(test)]
pub(super) fn note_producer() {
    PRODUCERS.with(|value| value.set(value.get().checked_add(1).unwrap()));
}
#[cfg(test)]
pub(super) fn producer_count() -> usize {
    PRODUCERS.with(std::cell::Cell::get)
}
#[cfg(test)]
pub(in crate::cleanup_plan::replay) mod source;
#[cfg(test)]
mod tests;
