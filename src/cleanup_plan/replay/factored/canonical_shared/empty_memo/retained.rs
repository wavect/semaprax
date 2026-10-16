//! Exact-comparison receiving owner; the complete ordinary Memo stays intact.
use super::{Diagnostic, Memo, ReplayBudget, ResolvedFunction};
use std::mem::size_of;

pub(in crate::cleanup_plan::replay::factored::canonical_shared) struct RetainedMemo<T> {
    memo: Memo<T>,
    refusal: Option<Diagnostic>,
    controls_paid: bool,
}
impl<T> RetainedMemo<T> {
    pub(in crate::cleanup_plan::replay::factored::canonical_shared) fn new() -> Self {
        Self {
            memo: Memo::new(),
            refusal: None,
            controls_paid: false,
        }
    }
    pub(in crate::cleanup_plan::replay::factored::canonical_shared) fn authenticate_retained(
        &mut self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) {
        if self.memo.failed {
            return;
        }
        // Only a fully paid owner with a live binding may use the held slot.
        // The cold ordinary receiver below, including every real Result fee,
        // remains the activation boundary.
        if self.controls_paid {
            if let Some(binding) = &self.memo.binding {
                let ledger_matches = binding.ledger.current();
                if binding.function != std::ptr::from_ref(function)
                    || binding.budget != std::ptr::from_ref(budget)
                    || !ledger_matches
                {
                    self.memo.failed = true;
                    self.refusal = Some(Diagnostic::io("SPX-H006", String::new()));
                    return;
                }
                budget.authenticate_zero_into(
                    function,
                    "factored closure memo authentication",
                    &mut self.refusal,
                );
                if self.refusal.is_some() {
                    self.memo.failed = true;
                }
                return;
            }
        }
        // The ordinary function still constructs a Diagnostic-bearing Result
        // on every invocation, including warm zero-byte authentication. Keep
        // its actual per-call authorization-receipt charge. A unit-returning
        // wrapper does not eliminate that inner construction.
        if let Err(error) = self.memo.authenticate(function, budget) {
            self.memo.failed = true;
            self.refusal = Some(error);
            return;
        }
        if !self.controls_paid {
            if budget.pays_allocations() {
                if let Err(error) = budget.reserve_bytes(
                    function,
                    size_of::<Self>() - size_of::<Memo<T>>()
                        + size_of::<Option<Diagnostic>>()
                        + size_of::<Diagnostic>()
                        + size_of::<Result<(), Diagnostic>>(),
                    "factored retained memo controls and failure receiving",
                ) {
                    self.memo.failed = true;
                    self.refusal = Some(error);
                    return;
                }
            }
            self.controls_paid = true;
        }
    }
    pub(in crate::cleanup_plan::replay::factored::canonical_shared) fn failed(&self) -> bool {
        self.memo.failed
    }
    pub(in crate::cleanup_plan::replay::factored::canonical_shared) fn failure(
        &mut self,
    ) -> Diagnostic {
        assert!(self.memo.failed);
        self.refusal
            .take()
            .unwrap_or_else(|| Diagnostic::io("SPX-H006", String::new()))
    }
    pub(in crate::cleanup_plan::replay::factored::canonical_shared) fn reject_after_error(
        &mut self,
    ) {
        self.memo.reject_after_error();
    }
    pub(in crate::cleanup_plan::replay::factored::canonical_shared) fn contains_key(
        &self,
        key: &usize,
    ) -> bool {
        self.memo.contains_key(key)
    }
    pub(in crate::cleanup_plan::replay::factored::canonical_shared) fn state(
        &self,
        key: &usize,
    ) -> u8 {
        self.memo.state(key)
    }
    pub(in crate::cleanup_plan::replay::factored::canonical_shared) fn get(
        &self,
        key: &usize,
    ) -> Option<&T> {
        self.memo.get(key)
    }
    pub(in crate::cleanup_plan::replay::factored::canonical_shared) fn insert_retained(
        &mut self,
        function: &ResolvedFunction,
        key: usize,
        budget: &mut ReplayBudget,
        make: impl FnOnce() -> T,
    ) -> Result<(), Diagnostic> {
        self.authenticate_retained(function, budget);
        if self.memo.failed {
            return Err(self.failure());
        }
        let result = self.memo.insert_bound(function, key, budget, make);
        if result.is_err() {
            self.memo.failed = true;
        }
        result
    }
    pub(in crate::cleanup_plan::replay::factored::canonical_shared) fn insert_empty_retained(
        &mut self,
        function: &ResolvedFunction,
        key: usize,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        self.authenticate_retained(function, budget);
        if self.memo.failed {
            return Err(self.failure());
        }
        let result = self.memo.insert_empty_bound(function, key, budget);
        if result.is_err() {
            self.memo.failed = true;
        }
        result
    }
}
