//! Draft #941: explicit bool receivers and one invocation-local terminal slot.
//! No key, index proof or validation verdict survives a function boundary.
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
    error: Option<Diagnostic>,
    terminal_taken: bool,
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
        + 2 * PAYMENT
        + Self::TERMINAL_BYTES
        + Self::PUBLICATION_PANIC_BYTES;
    pub(in crate::cleanup_plan::replay) const PER_FUNCTION_BYTES: usize =
        // Complete seventh-argument caller/driver transport, not a discounted
        // difference from the independently retained six-argument producer.
        2 * size_of::<(&dyn crate::cleanup::ReplayContext, &ResolvedFunction,
            &mut ReplayBudget, Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
            Option<&mut continuation_storage::Storage<'_>>,
            &mut expression_facts::arena::invocation::Backing<'_>, &mut Self)>()
        + 4 * size_of::<ClassifierLedger>()
        // Eight bool producer/caller pairs replace the absent outer
        // authenticate/context/directory/complete Diagnostic Result packets.
        + 16 * size_of::<bool>()
        // Four authentication entries each load/test terminal_taken, then
        // execute the complete new Option::is_some actual/formal and bool pair.
        + 4 * (2 * size_of::<bool>()
            + 2 * size_of::<&Option<Diagnostic>>() + 2 * size_of::<bool>())
        // Complete None initializer, Some publication and owned match subject;
        // pattern payload, lexical expansion value and caller ids receiver.
        + 3 * size_of::<Option<Directory<&'hir StatusSourceId>>>()
        + 3 * size_of::<Directory<&'hir StatusSourceId>>()
        + 2 * size_of::<bool>()
        // The filled-output guard has its own actual/formal Option loan and
        // returned/received bool; none comes from the authentication guard.
        + 2 * size_of::<&Option<Directory<&'hir StatusSourceId>>>()
        + 2 * size_of::<bool>()
        + 2 * size_of::<(&mut Self, &dyn crate::cleanup::ReplayContext,
            &ResolvedFunction, &mut ReplayBudget,
            &mut Option<Directory<&'hir StatusSourceId>>)>()
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

    // Only the first failed receiver reaches this terminal path, and its
    // caller immediately leaves the entire invocation. Surviving budget and
    // Directory::new Result transports remain fully funded per function.
    const TERMINAL_BYTES: usize = 6 * size_of::<Diagnostic>()
        + 4 * size_of::<Option<Diagnostic>>()
        + 2 * size_of::<Result<(), Diagnostic>>()
        // Complete take_error self loan plus expect's string actual/formal,
        // Option::take receiver pair, flag false/true literals and stores.
        + 2 * size_of::<(&mut Self, &str)>()
        + 2 * size_of::<&mut Option<Diagnostic>>()
        + 4 * size_of::<bool>();

    // The owned Some-pattern has no take/expect call. Its impossible None arm
    // retains a cold internal panic, with its literal/format/location and
    // boxed static-payload caller/receiving packet prepaid before acquisition.
    const PUBLICATION_PANIC_BYTES: usize = 2 * size_of::<std::fmt::Arguments<'static>>()
        + size_of::<std::panic::Location<'static>>()
        + 2 * size_of::<&std::panic::Location<'static>>()
        + 2 * size_of::<&'static str>()
        + 2 * size_of::<Box<&'static str>>();

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
            error: None,
            terminal_taken: false,
        })
    }
    fn authenticate_into(
        &mut self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> bool {
        if self.terminal_taken {
            return false;
        }
        if self.error.is_some() {
            return false;
        }
        if !std::ptr::eq(self.budget, budget)
            || self.payer != budget.payer
            || (self.payer == AllocationPayer::ActiveBuilder && !self.ledger.current())
        {
            self.error = Some(budget.budget_refusal(
                function,
                "status backing belongs to another invocation",
                "",
            ));
            return false;
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
            self.error = Some(budget.budget_refusal(
                function,
                "status backing function order differs from held program",
                "",
            ));
            return false;
        }
        // This original inner Result and all its PAYMENT bytes survive.
        if let Err(error) =
            budget.reserve_bytes(function, 0, "invocation status backing authentication")
        {
            self.error = Some(error);
            return false;
        }
        true
    }
    fn context_into(
        &mut self,
        program: &dyn crate::cleanup::ReplayContext,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> bool {
        if !self.authenticate_into(function, budget) {
            return false;
        }
        if !program
            .full_program()
            .is_some_and(|full| std::ptr::eq(full, self.program))
        {
            self.error = Some(budget.budget_refusal(
                function,
                "status backing belongs to another typed program",
                "",
            ));
            return false;
        }
        // Re-authenticate after the source callback before any positive debit.
        self.authenticate_into(function, budget)
    }
    pub(in crate::cleanup_plan::replay::expression_facts::receiving) fn directory_into(
        &mut self,
        program: &dyn crate::cleanup::ReplayContext,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
        output: &mut Option<Directory<&'hir StatusSourceId>>,
    ) -> bool {
        if !self.context_into(program, function, budget) {
            return false;
        }
        if output.is_some() {
            self.error = Some(budget.budget_refusal(
                function,
                "status directory receiving slot is not empty",
                "",
            ));
            return false;
        }
        // COMPLETE original constructor and fresh empty Vec/index controls,
        // work and sticky/zero checks. Its Diagnostic Result remains charged.
        let mut directory =
            match Directory::new(function, budget, "cleanup status identity directory") {
                Ok(directory) => directory,
                Err(error) => {
                    self.error = Some(error);
                    return false;
                }
            };
        if let Some(nodes) = self.empty.take() {
            directory.nodes = nodes;
        }
        *output = Some(directory);
        true
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
    pub(in crate::cleanup_plan::replay) fn complete_into(
        &mut self,
        program: &dyn crate::cleanup::ReplayContext,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> bool {
        if !self.context_into(program, function, budget) {
            return false;
        }
        const {
            assert!(size_of::<ResolvedFunction>() > 1);
        }
        self.next_function += 1;
        true
    }
    pub(in crate::cleanup_plan::replay) fn take_error(&mut self) -> Diagnostic {
        // Every owning call site returns immediately. Keep the affine terminal
        // state even after moving out its diagnostic, so no later function or
        // callback can resume this backing after extraction.
        self.terminal_taken = true;
        self.error
            .take()
            .expect("failed invocation receiver owns its first diagnostic")
    }
}

macro_rules! receive_directory {
    ($owner:ident, $program:ident, $function:ident, $budget:ident) => {{
        let mut received = None;
        if !$owner.directory_into($program, $function, $budget, &mut received) {
            return Err($owner.take_error());
        }
        let Some(directory) = received else {
            unreachable!("fresh status directory publication");
        };
        directory
    }};
}
pub(in crate::cleanup_plan::replay::expression_facts::receiving) use receive_directory;

#[cfg(test)]
#[path = "held_invocation_tests.rs"]
mod tests;
