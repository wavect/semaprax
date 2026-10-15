//! Private funding route for the authenticated canonical caller. Legacy
//! semantic diagnostics and upstream compiler preflight remain outside this
//! storage protocol; no whole-validator funding claim follows.
use super::storage::{classify, PlannerError};
use super::*;
use crate::bounded_output::cfg_reservation::CfgReservation;

pub(super) fn build(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    payer: &CfgReservation,
) -> Result<LoanPlan, PlannerError> {
    // Same immutable current subject, with no new source/profile authority.
    if !program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
        .any(|current| std::ptr::eq(current, function))
    {
        return Err(PlannerError::Semantic(error(
            "funded loan planner requires the exact current function",
        )));
    }
    payer
        .reserve(SUBJECT_BYTES)
        .map_err(PlannerError::Physical)?;
    let subject = CurrentSubject { program, function };
    // Lexical preflight precedes the legacy guidance boundary. Its semantic
    // refusal must retain its original span/path/help, even for a work limit.
    let lexical = lexical_preflight(program, function, payer, &mut |program, region| {
        traversal::paid::inspect(program, region, payer).map(|proof| proof.is_some())
    })?;
    let result = (|| {
        let mut work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
        work.allocation = Some(payer);
        #[cfg(test)]
        work_boundary::apply(&mut work);
        let inventory = {
            let _stage = crate::bounded_output::reservation_stage_guard(
                crate::bounded_output::ReservationStage::LoanCandidateInventory,
            );
            paid_candidate::inspect_retained(subject, payer, &mut work)?
        };
        if let Some(inventory) = inventory {
            paid_plan::build_retained(program, function, &mut work, inventory)
        } else {
            let mut plan = LoanPlan::empty_v1();
            if lexical {
                plan.schema = LOAN_PLAN_SCHEMA_V3;
            }
            Ok(plan)
        }
    })();
    // Guidance is semantic rendering and may allocate. Never run it on a
    // physical marker or use its text to decide which receipt to publish.
    match classify(result, payer) {
        Err(PlannerError::Semantic(error)) => Err(PlannerError::Semantic(guidance::work_limit(
            error, function,
        ))),
        result => result,
    }
}

fn lexical_preflight(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    payer: &CfgReservation,
    inspect: &mut dyn FnMut(
        &dyn crate::cleanup::ReplayContext,
        &ResolvedExpr,
    ) -> Result<bool, Diagnostic>,
) -> Result<bool, PlannerError> {
    classify(
        (|| {
            payer.reserve(0).map_err(|_| storage::marker())?;
            let lexical = crate::hir::lexical_exit::requires(function)?;
            if lexical {
                let owner =
                    crate::hir::lexical_exit::owner_with(program, function, &mut |bytes| {
                        payer.reserve(bytes).map_err(|_| storage::marker())
                    })?;
                crate::hir::lexical_exit::validate_function_with(
                    program, function, &owner, inspect,
                )?;
            }
            Ok(lexical)
        })(),
        payer,
    )
}

#[cfg(test)]
#[allow(
    clippy::duplicate_mod,
    reason = "Run the same controls against independent receivers."
)]
mod tests;

#[cfg(test)]
#[allow(
    clippy::duplicate_mod,
    reason = "Run the same controls against independent receivers."
)]
mod directories_tests;

#[cfg(test)]
#[allow(
    clippy::duplicate_mod,
    reason = "Run the same controls against independent receivers."
)]
mod work_boundary;

// Constructible only after this producer's exact current-function check.
pub(super) struct CurrentSubject<'a> {
    program: &'a ResolvedProgram,
    function: &'a ResolvedFunction,
}
const SUBJECT_BYTES: usize = 3 * std::mem::size_of::<CurrentSubject<'static>>();
impl<'a> CurrentSubject<'a> {
    pub(super) fn into_refs(self) -> (&'a ResolvedProgram, &'a ResolvedFunction) {
        (self.program, self.function)
    }
}

#[cfg(test)]
mod inventory_before;
#[cfg(test)]
pub(super) fn with_inventory_work_limit<T>(limit: usize, operation: impl FnOnce() -> T) -> T {
    work_boundary::with_limit(limit, operation)
}
#[cfg(test)]
pub(super) fn subject_for_test<'a>(
    program: &'a ResolvedProgram,
    function: &'a ResolvedFunction,
    payer: &CfgReservation,
) -> Result<CurrentSubject<'a>, Diagnostic> {
    payer
        .reserve(SUBJECT_BYTES + std::mem::size_of::<Result<CurrentSubject<'a>, Diagnostic>>())
        .map_err(|_| storage::marker())?;
    if !program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
        .any(|current| std::ptr::eq(current, function))
    {
        return Err(Diagnostic::io_static(
            "SPX-H006",
            "funded loan planner requires the exact current function",
        ));
    }
    Ok(CurrentSubject { program, function })
}

#[cfg(test)]
pub(super) fn before_inventory_for_test(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    payer: &CfgReservation,
) -> Result<LoanPlan, PlannerError> {
    inventory_before::build(program, function, payer)
}

#[cfg(test)]
mod before_dense;
#[cfg(test)]
pub(super) use before_dense::build as build_before_dense;
