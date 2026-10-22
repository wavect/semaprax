//! Closed first-lookup continuation. No callbacks, ACTIVE switch or guard bank.
use super::directory::HeldPrefix;
use super::*;
use crate::cleanup_plan::replay::expression_facts::arena::{retained::Session, Arena};
use crate::cleanup_plan::replay::ordinary_receiving::{
    prepare_into, refuse_into, reserve_into, Failure,
};
use std::mem::size_of;

// Disjoint first-frontier site ledger. The borrowed tag pattern constructs
// no owned AllocationPayer and invokes no PartialEq actual/formal arguments.
const PAYER_TAG: usize =
    size_of::<&crate::cleanup_plan::replay::allocation::AllocationPayer>() + 3 * size_of::<bool>(); // true/false match arms and tag-result receiver
const ROW_LENGTH: usize =
    2 * size_of::<&Vec<crate::cleanup_plan::StatusSource>>() + 2 * size_of::<usize>(); // Vec::len actual/formal, return/receiving
const ROW_BOUND: usize = size_of::<usize>() // MIN_ROWS operand
    + 2 * size_of::<bool>(); // primitive >= result and receiving
const SHORT_CIRCUIT: usize = 2 * size_of::<bool>(); // && result and if receiver
const FACTS_ARM: usize = size_of::<&Facts<'_>>() // borrowed scrutinee receiver
    + size_of::<&Arena<'_>>() // Arena arm binding
    + size_of::<bool>(); // variant-selection branch
const NONEMPTY: usize = Session::arena_len_bytes()
    + size_of::<usize>() // zero operand
    + 2 * size_of::<bool>(); // primitive != result and if receiver

// Extra first zero-byte/work authentication (four Result roles), selector
// reservation (two), and helper return/receiving (two), all at this frontier.
const PAYMENT_AND_RETURN: usize = 8 * size_of::<Result<(), Diagnostic>>();
// Actual/formal frames for the extra first reserve(0), charge(0), and selector
// reserve. These are additional sites, not credit from original lookup fees.
const PAYMENT_TRANSPORT: usize =
    6 * size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>();
const CONTINUATION: usize = 2 * size_of::<(
    &ResolvedFunction,
    &Arena<'_>,
    &mut ReplayBudget,
    &case_proof::StatusCases<'_>,
    &mut HeldPrefix<'_, '_>,
    &mut usize,
)>(); // complete helper actual/formal transports
pub(in crate::cleanup_plan::replay::expression_facts::receiving::retained) const SELECT_BYTES:
    usize = PAYER_TAG
    + ROW_LENGTH
    + ROW_BOUND
    + SHORT_CIRCUIT
    + FACTS_ARM
    + NONEMPTY
    + PAYMENT_AND_RETURN
    + PAYMENT_TRANSPORT
    + CONTINUATION;
const A: usize = SELECT_BYTES + Session::acquisition_bytes();
const S: usize = Session::OLD_SCRATCH;
const R: usize = Session::query_bytes();
// Conservative full-composition break-even; no success-count claim for an
// early semantic error. Preserve the complete first old scratch tariff too.
pub(in crate::cleanup_plan::replay::expression_facts::receiving::retained) const MIN_ROWS: usize =
    if S > R {
        (A + S) / (S - R) + 1
    } else {
        usize::MAX
    };

pub(super) fn complete_rows<'hir, 'facts>(
    function: &'hir ResolvedFunction,
    arena: &Arena<'facts>,
    budget: &mut ReplayBudget,
    proof: &case_proof::StatusCases<'hir>,
    prefix_owner: &mut HeldPrefix<'_, 'hir>,
    ordinal: &mut usize,
) -> Result<(), Diagnostic> {
    budget.reserve_bytes(
        function,
        S + Session::acquisition_bytes() + Failure::acquisition_bytes(),
        "retained expression-fact owner acquisition",
    )?;
    budget.charge(
        function,
        12,
        "retained expression-fact owner initialization",
    )?;
    #[cfg(test)]
    crate::cleanup_plan::replay::ordinary_receiving::observe_site(1);
    let mut failure = Failure {
        error: None,
        function,
        budget,
    };
    let mut received = None;
    let mut session = Session::new_prepaid(arena, function, budget);
    loop {
        let source = &function.cleanup_plan.status_sources[*ordinal];
        {
            if !reserve_into!(
                session.budget,
                function,
                failure,
                Session::into_query_bytes(),
                "ordinary fact query actual and formal receiving"
            ) {
                return Err(failure
                    .error
                    .take()
                    .expect("query frontier selected first error"));
            }
        }
        if !session.fact_into(&source.id.expression, &mut received, &mut failure) {
            return Err(failure
                .error
                .take()
                .expect("fact query selected first error"));
        }
        let Some(fact) = received else {
            return Err(<Diagnostic as semantic_refusal::Error>::structural(
                function,
                semantic_refusal::Atom::UnknownStatus(source.id.expression.clone()),
            ));
        };
        let budget = &mut *session.budget;
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
                proof.member(function, source, *ordinal, budget)?;
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
        *ordinal += 1;
        if *ordinal == function.cleanup_plan.status_sources.len() {
            break;
        }
        let source = &function.cleanup_plan.status_sources[*ordinal];
        let budget = &mut *session.budget;
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
    }
    Ok(())
}
