//! Invocation-local empty backing. No facts, verdict, payload or cursor crosses
//! a function boundary. Every function runs the original complete producer.
use super::*;
use crate::bounded_output::classifier_string::ClassifierLedger;
use std::mem::size_of;

type Frames<'hir> = Vec<FactFrame<'hir>>;
const PAYMENT: usize = 2 * size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>()
    + 2 * size_of::<Result<(), Diagnostic>>();

pub(in crate::cleanup_plan::replay) struct Backing<'hir> {
    program: &'hir ResolvedProgram,
    budget: *const ReplayBudget,
    payer: AllocationPayer,
    next_function: usize,
    ledger: ClassifierLedger,
    empty: Option<(Arena<'hir>, Frames<'hir>)>,
}
impl<'hir> Backing<'hir> {
    // Called at the original program preflight's already selected first
    // function, after its unchanged skeleton-work reservation. No new first
    // selector, global owner or cross-invocation cache is introduced.
    pub(in crate::cleanup_plan::replay) fn acquire(
        program: &'hir ResolvedProgram,
        _function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Self, Diagnostic> {
        // The original preflight first-function arm prepaid the full call.
        // Count/product/Options and both Vec length receiver families were
        // prepaid at the original first-function arm BEFORE this call.
        let count = program
            .functions
            .len()
            .checked_add(program.function_instances.len())
            .ok_or_else(|| {
                budget.budget_refusal(_function, "invocation fact function count overflowed", "")
            })?;
        let total = count
            .checked_mul(
                Self::CALL_BYTES
                    + Self::RECEIVING_BYTES
                    + expression_facts::receiving::INVOCATION_RELEASE_BYTES,
            )
            .ok_or_else(|| {
                budget.budget_refusal(_function, "invocation fact receiving bank overflowed", "")
            })?;
        budget.reserve_bytes(
            _function,
            total,
            "invocation fact complete receiving and release banks",
        )?;
        let mut ledger = ClassifierLedger::empty_receiving();
        if budget.payer == AllocationPayer::ActiveBuilder {
            ledger.capture_current_into();
        }
        Ok(Self {
            program,
            budget,
            payer: budget.payer,
            next_function: 0,
            ledger,
            empty: None,
        })
    }
    pub(in crate::cleanup_plan::replay) const ACQUISITION_BYTES: usize = {
        // Owner initializer, Ok payload and returned/receiving owner, actual
        // Result<Self> transport, ledger acquisition/drop and constructor loans.
        // The lexical preflight forms NO Option<Self> or enlarged return ABI.
        4 * size_of::<Self>()
            + 2 * size_of::<Result<Self, Diagnostic>>()
            + 4 * size_of::<ClassifierLedger>()
            + ClassifierLedger::capture_into_controls()
            + 2 * size_of::<(&ResolvedProgram, &ResolvedFunction, &mut ReplayBudget)>()
            + size_of::<([usize; 32], [Option<usize>; 8], [bool; 8])>()
            + 2 * size_of::<&Vec<ResolvedFunction>>()
            + 2 * size_of::<&Vec<crate::hir::ResolvedFunctionInstance>>()
            + 2 * PAYMENT
    };
    pub(in crate::cleanup_plan::replay) const CALL_BYTES: usize = {
        // Six-argument driver actual/formal packet and return/receiver, extra
        // owner loans, every authentication binding and Option/take transport.
        2 * size_of::<(
            &dyn crate::cleanup::ReplayContext,
            &ResolvedFunction,
            &mut ReplayBudget,
            Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
            Option<&mut continuation_storage::Storage<'_>>,
            &mut Self,
        )>() + 4 * size_of::<Result<(), Diagnostic>>()
            + 4 * size_of::<Option<(Arena<'hir>, Frames<'hir>)>>()
            + 4 * size_of::<&mut Self>()
            + ClassifierLedger::current_receiving_controls()
            + 2 * size_of::<(
                &ResolvedFunction,
                &dyn crate::cleanup::ReplayContext,
                &mut ReplayBudget,
                &mut Self,
            )>()
            + size_of::<([usize; 16], [bool; 12])>()
            + 2 * PAYMENT
    };
    pub(in crate::cleanup_plan::replay) const RECEIVING_BYTES: usize = {
        4 * size_of::<Option<(Arena<'hir>, Frames<'hir>)>>()
            + 4 * size_of::<&mut Self>()
            + ClassifierLedger::current_receiving_controls()
            + PAYMENT
            + 4 * size_of::<Lease<'_, '_>>()
            + 4 * size_of::<&Lease<'_, '_>>()
            + 4 * size_of::<&Facts<'hir>>()
            + 4 * size_of::<Result<Lease<'_, '_>, Diagnostic>>()
            + 4 * size_of::<Arena<'_>>()
            + 4 * size_of::<Frames<'_>>()
            + 4 * size_of::<capacity::ArenaCapacity<'_, '_>>()
            + size_of::<([usize; 32], [Option<usize>; 12], [bool; 16])>()
    };
    // Lexical receivers intentionally never construct expression facts. Keep
    // their complete original receiver first, then advance only the ordinal;
    // empty backing is unchanged and no verdict is retained.
    pub(in crate::cleanup_plan::replay) fn complete_lexical(
        &mut self,
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
                "expression fact backing belongs to another typed program",
                "",
            ));
        }
        self.authenticate(function, budget)?;
        self.next_function += 1;
        Ok(())
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
                "expression fact backing belongs to another invocation",
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
                "expression fact backing function sequence differs from the held program",
                "",
            ));
        }
        budget.reserve_bytes(function, 0, "invocation fact backing authentication")
    }
}

pub(in crate::cleanup_plan::replay) struct Lease<'owner, 'hir> {
    facts: Option<Facts<'hir>>,
    frames: Frames<'hir>,
    owner: &'owner mut Backing<'hir>,
}
impl<'hir> std::ops::Deref for Lease<'_, 'hir> {
    type Target = Facts<'hir>;
    fn deref(&self) -> &Self::Target {
        self.facts.as_ref().expect("held invocation facts")
    }
}
impl Drop for Lease<'_, '_> {
    fn drop(&mut self) {
        // Entire reset/return/drop transport is prepaid before constructing
        // this lease. No fallible payment, authority query or callback in Drop.
        if let Some(Facts::Arena(mut arena)) = self.facts.take() {
            arena.nodes.clear();
            arena.root = NIL;
            arena.first = NIL;
            for frame in &mut self.frames {
                *frame = None;
            }
            self.owner.empty = Some((arena, std::mem::take(&mut self.frames)));
        }
        // Authentication admitted an actual element of a non-ZST Vec. Its
        // physical length bounds the ordinal below usize::MAX; no mutation
        // of that immutable program or ordinal occurs while the lease is held.
        const {
            assert!(size_of::<ResolvedFunction>() > 1);
        }
        self.owner.next_function += 1;
    }
}

pub(in crate::cleanup_plan::replay) fn with_backing<'owner, 'hir>(
    function: &'hir ResolvedFunction,
    program: &dyn crate::cleanup::ReplayContext,
    budget: &mut ReplayBudget,
    owner: &'owner mut Backing<'hir>,
) -> Result<Lease<'owner, 'hir>, Diagnostic> {
    // Complete fixed roles were prepaid for all immutable program functions at
    // acquisition, before any callback could replace the held ledger. There is
    // NO new positive debit before this affinity check.
    owner.authenticate(function, budget)?;
    if !program
        .full_program()
        .is_some_and(|full| std::ptr::eq(full, owner.program))
    {
        return Err(budget.budget_refusal(
            function,
            "expression fact backing belongs to another typed program",
            "",
        ));
    }
    owner.authenticate(function, budget)?;
    let payer = match budget.payer {
        AllocationPayer::ActiveBuilder => capacity::Payer::ActiveBuilder,
        #[cfg(test)]
        AllocationPayer::Oracle(lease) => capacity::Payer::Oracle(lease),
        _ => {
            // Complete old standalone/selected/tree producer remains current
            // for payers that do not own this invocation's ordinary backing.
            let facts = super::super::with_budget(function, budget)?;
            return Ok(Lease {
                facts: Some(facts),
                frames: Vec::new(),
                owner,
            });
        }
    };
    #[cfg(test)]
    let _operation = crate::bounded_output::stage_debits::operation_guard(
        crate::bounded_output::stage_debits::Operation::ReplayExpressionFacts,
    );
    #[cfg(feature = "unstable-workflow-profiling")]
    let _exclusive_operation = crate::bounded_output::exclusive_debits::operation_guard(
        crate::bounded_output::exclusive_debits::Operation::ReplayExpressionFacts,
    );
    budget.prepare_builder(function)?;
    let result = build(function, payer, budget, owner.empty.take());
    let (facts, frames) = match result {
        Err(errors::EngineError::Fact(receipt)) if receipt.physical() => {
            return Err(budget.budget_refusal(function, receipt.detail(), ""))
        }
        Err(errors::EngineError::Fact(receipt)) => return Err(receipt.legacy(function)),
        Err(errors::EngineError::Owned(error)) => return Err(error),
        Err(errors::EngineError::Structural(receipt)) => return Err(receipt.legacy(function)),
        Err(errors::EngineError::Failed) => {
            return Err(budget.budget_refusal(function, "expression facts already failed", ""))
        }
        Ok(result) => result,
    };
    // Payload construction is complete. Pay every reset slot and both return
    // moves before lease formation. A refused cleanup acquisition drops all
    // local backing instead of returning partially cleared storage.
    // Node payload destruction is the SAME already-funded old Facts Drop;
    // clear only changes the Vec header's later deallocation boundary. The
    // new per-slot work is resetting the retained cursor Options to None.
    let cleanup = frames
        .len()
        .checked_mul(size_of::<(
            [FactFrame<'hir>; 2],
            [Option<&mut FactFrame<'hir>>; 2],
            [&mut FactFrame<'hir>; 2],
            [usize; 4],
            [bool; 2],
        )>())
        .ok_or_else(|| {
            budget.budget_refusal(
                function,
                "expression fact backing cleanup layout overflowed",
                "",
            )
        })?;
    budget.reserve_bytes(function, cleanup, "invocation fact backing reset slots")?;
    Ok(Lease {
        facts: Some(Facts::Arena(facts)),
        frames,
        owner,
    })
}

fn build<'hir>(
    function: &'hir ResolvedFunction,
    payer: capacity::Payer<'_>,
    budget: &mut ReplayBudget,
    empty: Option<(Arena<'hir>, Frames<'hir>)>,
) -> Result<(Arena<'hir>, Frames<'hir>), errors::EngineError> {
    use errors::Policy;
    // Original capacity and Arena owner tariffs remain, even on reuse.
    let mut capacity = capacity::ArenaCapacity::new(function, payer)
        .map_err(|failure| errors::Selected::capacity(function, failure))?;
    budget.charge(function, 1, "expression-fact arena owner construction")?;
    let mut facts = match empty {
        Some((arena, frames)) => {
            debug_assert!(arena.nodes.is_empty() && arena.root == NIL && arena.first == NIL);
            capacity.install_cleared_frames(frames);
            // Pay the same Arena initializer/header as the complete old build;
            // only actual absent replacement Vec allocations can be saved.
            capacity
                .arena_reserve_array::<Arena<'hir>>(1)
                .map_err(|failure| errors::Selected::capacity(function, failure))?;
            arena
        }
        None => Arena::new(&mut capacity)
            .map_err(|failure| errors::Selected::capacity(function, failure))?,
    };
    for expression in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        producer::collect_arena_facts::<errors::Selected>(
            function,
            expression,
            &mut facts,
            Some(&mut capacity),
            budget,
        )?;
    }
    Ok((facts, capacity.take_frames()))
}

#[cfg(test)]
#[path = "invocation_tests.rs"]
mod tests;
