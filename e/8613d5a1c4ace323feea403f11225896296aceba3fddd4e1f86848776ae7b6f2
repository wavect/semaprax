//! Invocation-local empty status-directory backing; no key/index proof survives.
use super::*;
use crate::bounded_output::classifier_string::ClassifierLedger;
use std::mem::size_of;

type Nodes<'hir> = Vec<Node<&'hir StatusSourceId>>;
const PAYMENT: usize = 2 * size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>()
    + 2 * size_of::<Result<(), Diagnostic>>();

pub(in crate::cleanup_plan::replay) struct Backing<'hir> {
    program: &'hir ResolvedProgram,
    budget: *const ReplayBudget,
    payer: AllocationPayer,
    ledger: ClassifierLedger,
    next_function: usize,
    empty: Option<Nodes<'hir>>,
}
impl<'hir> Backing<'hir> {
    // This is an ADDITIONAL bank, never a replacement for the fact-owner bank.
    // It precedes count/product arithmetic, constructor arguments and capture.
    // Six owners include the final invocation Drop actual/formal transport.
    pub(in crate::cleanup_plan::replay) const ACQUISITION_BYTES: usize = 6 * size_of::<Self>()
        + 2 * size_of::<Result<Self, Diagnostic>>()
        + 4 * size_of::<ClassifierLedger>()
        + ClassifierLedger::capture_into_controls()
        + 2 * size_of::<(&ResolvedProgram, &ResolvedFunction, &mut ReplayBudget)>()
        + 2 * size_of::<&Vec<ResolvedFunction>>()
        + 2 * size_of::<&Vec<crate::hir::ResolvedFunctionInstance>>()
        + size_of::<([usize; 24], [Option<usize>; 8], [bool; 8])>()
        + 2 * PAYMENT;
    pub(in crate::cleanup_plan::replay) const PER_FUNCTION_BYTES: usize =
        // Complete seventh-argument caller/driver transport, not a discounted
        // difference from the independently retained six-argument producer.
        2 * size_of::<(&dyn crate::cleanup::ReplayContext, &ResolvedFunction,
            &mut ReplayBudget, Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
            Option<&mut continuation_storage::Storage<'_>>,
            &mut expression_facts::arena::invocation::Backing<'_>, &mut Self)>()
        + 4 * size_of::<ClassifierLedger>()
        + 8 * size_of::<Result<(), Diagnostic>>() + 8 * size_of::<Diagnostic>()
        // Authentication before/after the full_program callback, directory
        // acquisition and successful completion each retain actual/formals.
        + 4 * size_of::<(&mut Self, &dyn crate::cleanup::ReplayContext,
            &ResolvedFunction, &mut ReplayBudget)>()
        + 4 * ClassifierLedger::current_receiving_controls()
        + 4 * size_of::<Option<Nodes<'hir>>>() + 6 * size_of::<Nodes<'hir>>()
        + 4 * size_of::<Directory<&'hir StatusSourceId>>()
        + 2 * size_of::<Result<Directory<&'hir StatusSourceId>, Diagnostic>>()
        // Return consumes only the completed directory at its original last-use
        // frontier: Option take, dropped index, Vec clear and empty assignment.
        + 4 * size_of::<(&mut Self, super::super::retained::StatusSources<'hir>)>()
        + 2 * size_of::<(crate::cleanup_plan::replay::leaf_index::Leaves,
            std::collections::BTreeSet<crate::cleanup_plan::StorageId>,
            super::super::retained::StatusCases<'static>,
            expression_facts::arena::invocation::Lease<'static, 'static>)>()
        + 4 * size_of::<Option<Directory<&'hir StatusSourceId>>>()
        + 4 * size_of::<Option<Box<status_prefix::FrozenStatusIndex>>>()
        + 4 * size_of::<&mut Nodes<'hir>>() + 2 * size_of::<&mut Directory<&'hir StatusSourceId>>()
        + size_of::<([usize; 32], [Option<usize>; 12], [bool; 24])>() + 4 * PAYMENT;

    pub(in crate::cleanup_plan::replay) fn acquire(
        program: &'hir ResolvedProgram,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Self, Diagnostic> {
        let count = program
            .functions
            .len()
            .checked_add(program.function_instances.len())
            .ok_or_else(|| {
                budget.budget_refusal(function, "status backing function count overflowed", "")
            })?;
        let total = count.checked_mul(Self::PER_FUNCTION_BYTES).ok_or_else(|| {
            budget.budget_refusal(function, "status backing receiving bank overflowed", "")
        })?;
        budget.reserve_bytes(
            function,
            total,
            "invocation status complete receiving and return banks",
        )?;
        let mut ledger = ClassifierLedger::empty_receiving();
        if budget.payer == AllocationPayer::ActiveBuilder {
            ledger.capture_current_into();
        }
        Ok(Self {
            program,
            budget,
            payer: budget.payer,
            ledger,
            next_function: 0,
            empty: None,
        })
    }
    fn authenticate(
        &self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        if !std::ptr::eq(self.budget, budget)
            || self.payer != budget.payer
            || (self.payer == AllocationPayer::ActiveBuilder && !self.ledger.current())
        {
            return Err(budget.budget_refusal(
                function,
                "status backing belongs to another invocation",
                "",
            ));
        }
        let next = if self.next_function < self.program.functions.len() {
            self.program.functions.get(self.next_function)
        } else {
            self.program
                .function_instances
                .get(self.next_function - self.program.functions.len())
                .map(|instance| &instance.function)
        };
        if next.is_none_or(|expected| !std::ptr::eq(expected, function)) {
            return Err(budget.budget_refusal(
                function,
                "status backing function order differs from held program",
                "",
            ));
        }
        budget.reserve_bytes(function, 0, "invocation status backing authentication")
    }
    fn context(
        &self,
        program: &dyn crate::cleanup::ReplayContext,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        self.authenticate(function, budget)?;
        if !program
            .full_program()
            .is_some_and(|full| std::ptr::eq(full, self.program))
        {
            return Err(budget.budget_refusal(
                function,
                "status backing belongs to another typed program",
                "",
            ));
        }
        // No positive payment may follow a callback until affinity is rechecked.
        self.authenticate(function, budget)
    }
    pub(in crate::cleanup_plan::replay::expression_facts::receiving) fn directory(
        &mut self,
        program: &dyn crate::cleanup::ReplayContext,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Directory<&'hir StatusSourceId>, Diagnostic> {
        self.context(program, function, budget)?;
        // Keep the COMPLETE original constructor, including its fresh empty
        // Vec, optional-index constructor, work and sticky/zero checks.
        let mut directory = Directory::new(function, budget, "cleanup status identity directory")?;
        if let Some(nodes) = self.empty.take() {
            directory.nodes = nodes;
        }
        Ok(directory)
    }
    pub(in crate::cleanup_plan::replay::expression_facts::receiving) fn recycle(
        &mut self,
        mut directory: Directory<&'hir StatusSourceId>,
    ) {
        // All return/Drop roles prepaid before the invocation; infallible and
        // authority-free. The immutable index drops at the old directory frontier.
        // Directory fields originally drop nodes before the frozen index.
        directory.nodes.clear();
        directory.root = NIL;
        directory.first = NIL;
        drop(directory.lookup.take());
        self.empty = Some(directory.nodes);
    }
    pub(in crate::cleanup_plan::replay) fn complete(
        &mut self,
        program: &dyn crate::cleanup::ReplayContext,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        self.context(program, function, budget)?;
        // Same non-ZST immutable program bound as the independent fact owner.
        const {
            assert!(size_of::<ResolvedFunction>() > 1);
        }
        self.next_function += 1;
        Ok(())
    }
}

#[cfg(test)]
#[path = "invocation_tests.rs"]
mod tests;
