//! Authority comes only from complete independent typed-HIR status discovery.
//! This affine proof borrows the immutable table, never its names as authority.
use super::*;
use std::cell::Cell;
use std::mem::size_of;

const PHASE: &str = "complete status case proof";

#[derive(Clone, Copy, Eq, PartialEq)]
enum State {
    Ready,
    Active,
    Finished,
    Failed,
}

pub(in crate::cleanup_plan::replay) struct StatusCases<'hir> {
    function: &'hir ResolvedFunction,
    rows: &'hir [StatusSource],
    budget: *const ReplayBudget,
    ledger: ClassifierLedger,
    state: Cell<State>,
}

// Affine receiving attempt: every early error/unwind poisons the held proof.
pub(super) struct Attempt<'proof, 'hir> {
    proof: &'proof StatusCases<'hir>,
    complete: bool,
}
impl Attempt<'_, '_> {
    pub(super) fn complete(
        &mut self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        self.proof.authenticate(function, budget)?;
        if self.proof.state.get() != State::Active {
            return Err(self.proof.reject());
        }
        budget.reserve_bytes(function, size_of::<Result<(), Diagnostic>>(), PHASE)?;
        budget.charge(function, 2, PHASE)?;
        self.proof.state.set(State::Finished);
        self.complete = true;
        Ok(())
    }
}
impl Drop for Attempt<'_, '_> {
    fn drop(&mut self) {
        if !self.complete {
            self.proof.state.set(State::Failed);
        }
    }
}

impl<'hir> StatusCases<'hir> {
    pub(in crate::cleanup_plan::replay) fn validate(
        program: &dyn crate::cleanup::ReplayContext,
        function: &'hir ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Self, Diagnostic> {
        // Keep the entire independent walk, including its late HIR-error priority.
        // No certificate, new payer request or early table mismatch precedes it.
        crate::cleanup_plan::replay::validate_required_status_sources(program, function)?;
        // Initializer, returned Result and actual caller's held receiving token.
        // An error from the old walk above is forwarded unchanged, not republished.
        budget.reserve_bytes(
            function,
            2 * size_of::<Self>() + size_of::<Result<Self, Diagnostic>>(),
            PHASE,
        )?;
        budget.charge(function, 5, PHASE)?;
        Ok(Self {
            function,
            rows: &function.cleanup_plan.status_sources,
            budget,
            ledger: ClassifierLedger::capture(),
            state: Cell::new(State::Ready),
        })
    }

    // Do not consult/debit a foreign payer or format text after selected refusal.
    // The first actual byte/work diagnostic is returned unchanged to its caller;
    // subsequent attempts retain H006 and cannot publish another acquisition.
    fn reject(&self) -> Diagnostic {
        self.state.set(State::Failed);
        Diagnostic::io("SPX-H006", String::new())
    }

    pub(super) fn authenticate(
        &self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        if matches!(self.state.get(), State::Failed | State::Finished)
            || !std::ptr::eq(self.function, function)
            || !std::ptr::eq(self.budget, budget)
        {
            return Err(self.reject());
        }
        if !self.ledger.current() {
            return Err(self.reject());
        }
        // The zero request authenticates the original owner's sticky state.
        let result = budget
            .reserve_bytes(function, 0, PHASE)
            .and_then(|()| budget.charge(function, 0, PHASE))
            .and_then(|()| {
                budget.reserve_bytes(function, size_of::<Result<(), Diagnostic>>(), PHASE)
            });
        if result.is_err() {
            self.state.set(State::Failed);
        }
        result
    }

    pub(super) fn begin<'proof>(
        &'proof self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Attempt<'proof, 'hir>, Diagnostic> {
        if self.state.get() != State::Ready {
            return Err(self.reject());
        }
        self.authenticate(function, budget)?;
        if let Err(error) = budget
            .reserve_bytes(
                function,
                2 * size_of::<Attempt<'_, '_>>() + size_of::<Result<Attempt<'_, '_>, Diagnostic>>(),
                PHASE,
            )
            .and_then(|()| budget.charge(function, 3, PHASE))
        {
            self.state.set(State::Failed);
            return Err(error);
        }
        self.state.set(State::Active);
        Ok(Attempt {
            proof: self,
            complete: false,
        })
    }

    pub(super) fn member(
        &self,
        function: &ResolvedFunction,
        source: &StatusSource,
        ordinal: usize,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        self.authenticate(function, budget)?;
        if self.state.get() != State::Active {
            return Err(self.reject());
        }
        // Inner actual lookup receipt and the successful outer member Result.
        // Neither the old directory's controls nor prior auth are a credit.
        let result = budget
            .reserve_bytes(
                function,
                size_of::<Option<&StatusSource>>() + size_of::<Result<(), Diagnostic>>(),
                PHASE,
            )
            .and_then(|()| budget.charge(function, 2, PHASE));
        if let Err(error) = result {
            self.state.set(State::Failed);
            return Err(error);
        }
        let held = self.rows.get(ordinal);
        if held.is_none_or(|held| !std::ptr::eq(held, source)) {
            return Err(self.reject());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
