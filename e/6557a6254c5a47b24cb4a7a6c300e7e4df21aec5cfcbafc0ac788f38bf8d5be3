//! Same current program and invocation owner; no work or physical allowance.
use super::*;
#[derive(Clone, Copy)]
pub(super) enum Failure {
    Program,
    Detached,
}
impl<'hir> Storage<'hir> {
    pub(in crate::cleanup_plan::replay) fn for_program(
        program: &'hir ResolvedProgram,
        budget: &mut ReplayBudget,
    ) -> Result<Self, Diagnostic> {
        let first = program
            .functions
            .iter()
            .chain(
                program
                    .function_instances
                    .iter()
                    .map(|instance| &instance.function),
            )
            .next();
        let Some(function) = first.filter(|_| budget.payer == AllocationPayer::ActiveBuilder)
        else {
            return Ok(Self::new(program));
        };
        // Actual two simultaneous storage initializers and returned receiver;
        // the new Rc identity is inline, captured only after this acquisition.
        budget.reserve_bytes(
            function,
            2 * std::mem::size_of::<Self>() + std::mem::size_of::<Result<Self, Diagnostic>>(),
            "held continuation owner carrier",
        )?;
        let mut storage = Self::new(program);
        storage.receiving_owner =
            Some(crate::bounded_output::classifier_string::ClassifierLedger::capture());
        Ok(storage)
    }
    #[cfg(test)]
    pub(in crate::cleanup_plan::replay) fn has_receiving_owner(&self) -> bool {
        self.receiving_owner.is_some()
    }
    pub(in crate::cleanup_plan::replay) fn authenticate(
        &mut self,
        program: &dyn crate::cleanup::ReplayContext,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        if self.receiving_first.is_none() {
            if !self.matches(program) {
                self.receiving_first = Some(Failure::Program);
            } else if self
                .receiving_owner
                .as_ref()
                .is_some_and(|owner| !owner.current())
            {
                self.receiving_first = Some(Failure::Detached);
            }
        }
        if let Some(first) = self.receiving_first {
            let reason = match first {
                Failure::Program => "typed-HIR continuation storage belongs to another program",
                Failure::Detached => "typed-HIR continuation storage belongs to another invocation",
            };
            return Err(budget.budget_refusal(function, reason, ""));
        }
        budget.reserve_bytes(function, 0, "typed HIR continuation storage authentication")
    }
}
