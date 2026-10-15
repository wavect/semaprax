//! Complete pre-reuse funded facade; original helpers remain in its parent.
use super::*;

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
        let mut work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
        work.allocation = Some(payer);
        #[cfg(test)]
        work_boundary::apply(&mut work);
        if paid_candidate::inspect(program, function, payer, &mut work)? {
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
