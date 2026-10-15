//! Caller-owned fallible identity index for ordinary lexical return operands.
//! Membership carries no admission, source freshness or certificate authority.
use super::*;
use crate::ast::LexicalExitKind;
use crate::bounded_output::classifier_string::ClassifierLedger;
use crate::hir::FunctionExecutionId;
use std::cell::Cell;
mod exits;
const PHASE: &str = "ordinary lexical return operands";
#[derive(Clone, Copy, Eq, PartialEq)]
enum State {
    Ready,
    ExitsValidated,
    Authenticated,
    Failed,
}

pub(in crate::cleanup_plan::replay) struct ReturnOperands<'facts, 'hir> {
    function: &'hir ResolvedFunction,
    facts: &'facts lexical_hir::LexicalFacts<'hir>,
    budget: *const ReplayBudget,
    ledger: ClassifierLedger,
    ids: Directory<&'hir ExpressionId>,
    state: Cell<State>,
}
impl<'facts, 'hir> ReturnOperands<'facts, 'hir> {
    pub(in crate::cleanup_plan::replay) fn new<E>(
        function: &'hir ResolvedFunction,
        facts: &'facts lexical_hir::LexicalFacts<'hir>,
        budget: &mut ReplayBudget,
    ) -> Result<Self, Diagnostic> {
        if budget.selected() || !budget.pays_allocations() {
            return Err(Diagnostic::io_static(
                "SPX-H006",
                "ordinary lexical return operands require their paid ordinary caller",
            ));
        }
        // Actual initializer/caller, directory temporary and all new receiving
        // controls. Identity handle backing is borrowed, never cloned or priced
        // as if another reservation retroactively owned it.
        type Controls<'facts, 'hir, E> = (
            ReturnOperands<'facts, 'hir>,
            ReturnOperands<'facts, 'hir>,
            Result<ReturnOperands<'facts, 'hir>, Diagnostic>,
            ClassifierLedger,
            std::slice::Iter<'facts, lexical_hir::ExitFact<'hir>>,
            Option<&'facts lexical_hir::ExitFact<'hir>>,
            Option<&'hir ResolvedExpr>,
            Directory<&'hir ExpressionId>,
            Result<Directory<&'hir ExpressionId>, Diagnostic>,
            Result<bool, Diagnostic>,
            Result<(), Diagnostic>,
            Result<(), E>,
            Result<lexical_cert::LexicalAuth, Diagnostic>,
            &'facts ReturnOperands<'facts, 'hir>,
        );
        budget.reserve_array::<Controls<'_, '_, E>>(function, 1, PHASE)?;
        budget.charge(function, 14, PHASE)?;
        let ledger = ClassifierLedger::capture();
        let mut ids = Directory::new(function, budget, PHASE)?;
        for exit in &facts.exits {
            // One inspected lexical fact, including non-return and valueless
            // facts. Directory comparisons, relocation and rotations retain
            // their existing independently declared per-operation charges.
            budget.charge(function, 1, PHASE)?;
            if exit.kind == LexicalExitKind::Return
                && let Some(operand) = exit.value
            {
                ids.insert(function, &operand.id, budget)?;
            }
        }
        Ok(Self {
            function,
            facts,
            budget,
            ledger,
            ids,
            state: Cell::new(State::Ready),
        })
    }
    fn reject(&self) -> Diagnostic {
        self.state.set(State::Failed);
        Diagnostic::io_static(
            "SPX-H006",
            "ordinary lexical return operands lost their owner",
        )
    }
    fn authenticate(
        &self,
        function: &ResolvedFunction,
        facts: &lexical_hir::LexicalFacts<'_>,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        if matches!(self.state.get(), State::Failed | State::Authenticated)
            || !std::ptr::eq(self.function, function)
            || !std::ptr::eq(self.facts, facts)
            || !std::ptr::eq(self.budget, budget)
            || !self.ledger.current()
        {
            return Err(self.reject());
        }
        let result = budget
            .reserve_bytes(function, 0, PHASE)
            .and_then(|()| budget.charge(function, 0, PHASE));
        if result.is_err() {
            self.state.set(State::Failed);
        }
        result
    }
    fn contains(
        &self,
        function: &ResolvedFunction,
        facts: &lexical_hir::LexicalFacts<'_>,
        expression: &ExpressionId,
        budget: &mut ReplayBudget,
    ) -> Result<bool, Diagnostic> {
        self.authenticate(function, facts, budget)?;
        if self.state.get() != State::Ready {
            return Err(self.reject());
        }
        let result = self.ids.contains(function, &expression, budget);
        if result.is_err() {
            self.state.set(State::Failed);
        }
        result
    }
    pub(in crate::cleanup_plan::replay) fn validate_exits(
        &self,
        program: &dyn crate::cleanup::ReplayContext,
        function: &ResolvedFunction,
        facts: &lexical_hir::LexicalFacts<'_>,
        storage: &BTreeSet<StorageId>,
        leaves: &Leaves,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        self.authenticate(function, facts, budget)?;
        if self.state.get() != State::Ready {
            return Err(self.reject());
        }
        let result = exits::validate_exits(program, function, storage, leaves, self, facts, budget);
        if result.is_ok() {
            self.authenticate(function, facts, budget)?;
        }
        self.state.set(if result.is_ok() {
            State::ExitsValidated
        } else {
            State::Failed
        });
        result
    }
    pub(in crate::cleanup_plan::replay) fn authenticate_certificates(
        &self,
        program: &dyn crate::cleanup::ReplayContext,
        function: &ResolvedFunction,
        owner: &FunctionExecutionId,
        facts: &lexical_hir::LexicalFacts<'_>,
        budget: &mut ReplayBudget,
    ) -> Result<lexical_cert::LexicalAuth, Diagnostic> {
        self.authenticate(function, facts, budget)?;
        if self.state.get() != State::ExitsValidated {
            return Err(self.reject());
        }
        let result = lexical_cert::authenticate(program, function, owner, facts, budget);
        if result.is_ok() {
            self.authenticate(function, facts, budget)?;
        }
        self.state.set(if result.is_ok() {
            State::Authenticated
        } else {
            State::Failed
        });
        result
    }
}

#[cfg(test)]
mod tests;
