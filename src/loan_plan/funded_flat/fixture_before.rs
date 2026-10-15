//! Private funding route for the authenticated canonical caller. Legacy
//! semantic diagnostics and upstream compiler preflight remain outside this
//! storage protocol; no whole-validator funding claim follows.
use super::storage::{classify, PlannerError};
use super::*;
use crate::loan_plan::{LOAN_PLAN_SCHEMA_V3, paid_candidate_before};
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
    // Lexical preflight precedes the legacy guidance boundary. Its semantic
    // refusal must retain its original span/path/help, even for a work limit.
    let lexical = lexical_preflight(program, function, payer, &mut |program, region| {
        traversal::paid::inspect(program, region, payer).map(|proof| proof.is_some())
    })?;
    let result = (|| {
        if paid_candidate_before::inspect(program, function, payer)? {
            let mut work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
            work.allocation = Some(payer);
            paid_plan::build(program, function, &mut work)
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

#[path = "tests_before.rs"]
mod tests;
