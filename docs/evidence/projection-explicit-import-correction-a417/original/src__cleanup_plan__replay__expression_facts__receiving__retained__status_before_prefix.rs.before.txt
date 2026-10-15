//! Complete status-fact reuse receiver before prefix insertion, independently compiled.
use super::*;

pub(in crate::cleanup_plan::replay) fn validate_status_sources_retained<'hir>(
    function: &'hir ResolvedFunction,
    facts: &Facts<'_>,
    budget: &mut ReplayBudget,
) -> Result<StatusSources<'hir>, Diagnostic> {
    if let Facts::Tree(tree) = facts {
        super::super::super::validate_status_sources(function, tree)?;
        budget.reserve_bytes(function, CONTROL_BYTES, "retained status full controls")?;
        return Ok(StatusSources { function, budget, ledger: ClassifierLedger::capture(), ids: None });
    }
    let expressions: &dyn Lookup = facts;
    let mut ids = Directory::new(function, budget, "cleanup status identity directory")?;
    for source in &function.cleanup_plan.status_sources {
        if !ids.insert(function, &source.id, budget)? {
            return Err(<Diagnostic as semantic_refusal::Error>::structural(
                function,
                semantic_refusal::Atom::RepeatedStatus(source.id.expression.clone()),
            ));
        }
        // Retain the actual first lookup's borrowed payload, only through this
        // immutable source's producer match. Pay its complete receiving pointer
        // and one assignment step; no previous result/header credit is used.
        budget.reserve_bytes(function,
            std::mem::size_of::<&Option<CallFact<'_>>>(),
            "retained status fact receiving control")?;
        budget.charge(function, 1, "retained status fact receiving assignment")?;
        let Some(fact) = expressions.fact(function, &source.id.expression, budget)? else {
            return Err(<Diagnostic as semantic_refusal::Error>::structural(
                function,
                semantic_refusal::Atom::UnknownStatus(source.id.expression.clone()),
            ));
        };
        match (&source.id.lane, &source.producer) {
            (
                StatusLane::OperationFailure | StatusLane::OwnerAdmission,
                StatusProducer::PropagatedCall { callee },
            ) => {
                // The payload reference is a second simultaneous receiving
                // control; the eliminated lookup's result is not a credit.
                budget.reserve_bytes(function, std::mem::size_of::<&CallFact<'_>>(),
                    "retained status call receiving control")?;
                budget.charge(function, 1, "retained status call receiving assignment")?;
                let Some(call) = fact
                else {
                    return Err(replay_error(
                        function,
                        "propagated-call status source does not name a call expression",
                    ));
                };
                if call.callee.as_ref() != callee {
                    return Err(replay_error(
                        function,
                        "propagated-call status source names the wrong callee",
                    ));
                }
            }
            (
                StatusLane::OperationFailure,
                StatusProducer::CheckedArithmetic {
                    normalized_cases, ..
                },
            ) => {
                let mut cases = Directory::new(
                    function,
                    budget,
                    "cleanup checked arithmetic case directory",
                )?;
                for case in normalized_cases {
                    cases.insert(function, *case, budget)?;
                }
                if cases.is_empty() || cases.len() != normalized_cases.len() {
                    return Err(replay_error(
                        function,
                        "checked-arithmetic status cases are empty or repeated",
                    ));
                }
            }
            (
                StatusLane::AllocationFailure,
                StatusProducer::CheckedAllocation {
                    normalized_cases, ..
                },
            ) if function.cleanup_plan.schema == crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V20
                && *normalized_cases
                    == [
                        crate::cleanup_plan::AllocationCase::PayloadRefused,
                        crate::cleanup_plan::AllocationCase::SizeOverflow,
                    ] => {}
            (StatusLane::ContractFalse, StatusProducer::ContractFalse { .. }) => {}
            _ => {
                return Err(replay_error(
                    function,
                    "status source lane does not match its producer",
                ));
            }
        }
    }
    budget.reserve_bytes(function, CONTROL_BYTES, "retained status full controls")?;
    Ok(StatusSources { function, budget, ledger: ClassifierLedger::capture(), ids: Some(ids) })
}

