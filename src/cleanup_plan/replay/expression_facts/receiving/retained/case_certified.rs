//! Complete retained status receiver; only the certified arithmetic branch differs.
use super::{
    case_proof, directory, replay_error, semantic_refusal, CallFact, ClassifierLedger, Diagnostic,
    Directory, Facts, Lookup, ReplayBudget, ResolvedFunction, StatusLane, StatusProducer,
    StatusSources, CONTROL_BYTES,
};

pub(in crate::cleanup_plan::replay) fn validate_status_sources_certified<'hir>(
    function: &'hir ResolvedFunction,
    facts: &Facts<'_>,
    budget: &mut ReplayBudget,
    proof: &case_proof::StatusCases<'hir>,
) -> Result<StatusSources<'hir>, Diagnostic> {
    let mut attempt = super::case_proof::begin_receiving!(proof, function, budget);
    if let Facts::Tree(tree) = facts {
        super::super::super::super::validate_status_sources(function, tree)?;
        budget.reserve_bytes(function, CONTROL_BYTES, "retained status full controls")?;
        attempt.complete(function, budget)?;
        return Ok(StatusSources {
            function,
            budget,
            ledger: ClassifierLedger::capture(),
            ids: None,
        });
    }
    let expressions: &dyn Lookup = facts;
    let mut ids = Directory::new(function, budget, "cleanup status identity directory")?;
    // This direct borrowed predicate replaces the absent complete:false
    // initializer in the unchanged, already-funded lexical begin frame.
    if matches!(
        &budget.payer,
        &crate::cleanup_plan::replay::allocation::AllocationPayer::ActiveBuilder
            | &crate::cleanup_plan::replay::allocation::AllocationPayer::LegacyUnobserved
    ) {
        // Pay the complete new lexical selection receivers before length
        // inspection. This is an additional real fee, never credit from the proof
        // or either insertion owner. The tree lane above retains its old path.
        budget.reserve_bytes(
            function,
            3 * std::mem::size_of::<Result<(), Diagnostic>>()
                + 2 * std::mem::size_of::<(&mut ReplayBudget, &ResolvedFunction, &str, usize)>()
                + std::mem::size_of::<&ReplayBudget>()
                + std::mem::size_of::<&Vec<crate::cleanup_plan::StatusSource>>()
                + 2 * std::mem::size_of::<usize>()
                + 4 * std::mem::size_of::<bool>(),
            "certified held-prefix selection controls",
        )?;
        budget.charge(function, 2, "certified held-prefix selection")?;
        if function.cleanup_plan.status_sources.len() >= 3 {
            let mut prefix_owner = ids.held_prefix(function, budget)?;
            budget.reserve_bytes(
                function,
                std::mem::size_of::<usize>(),
                "status case row ordinal",
            )?;
            budget.charge(function, 1, "status case row ordinal")?;
            let mut ordinal = 0usize;
            // This paid ordinal advances only after the complete fallible row body.
            // A direct while-index keeps that frontier without an Enumerate carrier.
            while ordinal < function.cleanup_plan.status_sources.len() {
                let source = &function.cleanup_plan.status_sources[ordinal];
                if !prefix_owner.insert(function, &source.id, budget)? {
                    return Err(<Diagnostic as semantic_refusal::Error>::structural(
                        function,
                        semantic_refusal::Atom::RepeatedStatus(source.id.expression.clone()),
                    ));
                }
                // Retain the actual first lookup's borrowed payload, only through this
                // immutable source's producer match. Pay its complete receiving pointer
                // and one assignment step; no previous result/header credit is used.
                budget.reserve_bytes(
                    function,
                    std::mem::size_of::<&Option<CallFact<'_>>>(),
                    "retained status fact receiving control",
                )?;
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
                        budget.reserve_bytes(
                            function,
                            std::mem::size_of::<&CallFact<'_>>(),
                            "retained status call receiving control",
                        )?;
                        budget.charge(function, 1, "retained status call receiving assignment")?;
                        let Some(call) = fact else {
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
                        // Complete independent discovery proved the exact held row.
                        // No case value is read here and no second case directory exists.
                        proof.member(function, source, ordinal, budget)?;
                        let _ = normalized_cases;
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
                budget.charge(function, 1, "status case row ordinal increment")?;
                ordinal += 1;
            }
            prefix_owner.freeze(function, budget)?;
            budget.reserve_bytes(function, CONTROL_BYTES, "retained status full controls")?;
            attempt.complete(function, budget)?;
            return Ok(StatusSources {
                function,
                budget,
                ledger: ClassifierLedger::capture(),
                ids: Some(ids),
            });
        }
    }
    let prefix_owner = directory::StatusInsertionOwner::new(function, &ids, budget)?;
    budget.reserve_bytes(
        function,
        std::mem::size_of::<usize>(),
        "status case row ordinal",
    )?;
    budget.charge(function, 1, "status case row ordinal")?;
    let mut ordinal = 0usize;
    // This paid ordinal advances only after the complete fallible row body.
    // A direct while-index keeps that frontier without an Enumerate carrier.
    while ordinal < function.cleanup_plan.status_sources.len() {
        let source = &function.cleanup_plan.status_sources[ordinal];
        if !ids.insert_certified(function, &source.id, budget, &prefix_owner)? {
            return Err(<Diagnostic as semantic_refusal::Error>::structural(
                function,
                semantic_refusal::Atom::RepeatedStatus(source.id.expression.clone()),
            ));
        }
        // Retain the actual first lookup's borrowed payload, only through this
        // immutable source's producer match. Pay its complete receiving pointer
        // and one assignment step; no previous result/header credit is used.
        budget.reserve_bytes(
            function,
            std::mem::size_of::<&Option<CallFact<'_>>>(),
            "retained status fact receiving control",
        )?;
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
                budget.reserve_bytes(
                    function,
                    std::mem::size_of::<&CallFact<'_>>(),
                    "retained status call receiving control",
                )?;
                budget.charge(function, 1, "retained status call receiving assignment")?;
                let Some(call) = fact else {
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
                // Complete independent discovery proved the exact held row.
                // No case value is read here and no second case directory exists.
                proof.member(function, source, ordinal, budget)?;
                let _ = normalized_cases;
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
        budget.charge(function, 1, "status case row ordinal increment")?;
        ordinal += 1;
    }
    drop(prefix_owner);
    budget.reserve_bytes(function, CONTROL_BYTES, "retained status full controls")?;
    attempt.complete(function, budget)?;
    Ok(StatusSources {
        function,
        budget,
        ledger: ClassifierLedger::capture(),
        ids: Some(ids),
    })
}

#[cfg(test)]
mod counter_before;
#[cfg(test)]
mod counter_tests;
