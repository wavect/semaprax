//! Direct immutable PLAN construction on the actual ordinary public or paid payer.
//! This does not select a shipping comparison, admit a retained profile, mint a
//! lease, or alter the original HIR/PLAN root's pending qualification gate.
use super::super::*;

// Exactly the predecessor eligibility boundary's five Boolean results and
// actual/formal references. PRODUCER_CONTROLS already prepays those typed slots.
pub(super) fn eligible(function: &ResolvedFunction, budget: &ReplayBudget) -> bool {
    budget.admission == AdmissionMode::Ordinary
        && matches!(
            budget.payer,
            AllocationPayer::LegacyUnobserved | AllocationPayer::ActiveBuilder
        )
        && !budget.lent_skeleton
        && !budget.merge_paths
        && function.cleanup_plan.lexical_control.is_none()
}
