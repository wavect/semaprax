//! Shared ordinary HIR paths enter independent full PLAN comparison.
//! Flat, lexical, selected, oracle, merged and lent receivers keep their route.
use super::super::producer::view;
use super::*;
use skeleton_work::enumerated_prefix;

const PAYMENT: usize = 2 * size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>()
    + 2 * size_of::<Result<(), Diagnostic>>();
// Setup owns the iterator and the first advance's reservation transport.
// Every advance renews STEP before next(), including its terminal None.
pub(in crate::cleanup_plan::replay) const PROBE: usize = 2 * size_of::<&Vec<SkeletonPath>>()
    + 2 * size_of::<&[SkeletonPath]>()
    + 2 * size_of::<std::slice::Iter<'static, SkeletonPath>>()
    + 2 * size_of::<bool>() // retained shared flag and its initializer
    + PAYMENT;
pub(in crate::cleanup_plan::replay) const PROBE_STEP: usize = 2 * size_of::<&mut std::slice::Iter<'static, SkeletonPath>>()
        + 2 * size_of::<Option<&SkeletonPath>>()
        + 2 * size_of::<&Option<Rc<skeleton_work::ObservationDag>>>()
        + 4 * size_of::<bool>() // Some/None, prefix query and shared publication
        + 2 * PAYMENT; // work call, then either next advance or finalizer call reservation
pub(in crate::cleanup_plan::replay) const CALL: usize = 2 * size_of::<(&ResolvedFunction, &mut ReplayBudget, &mut [SkeletonPath])>()
        + 2 * size_of::<&mut Vec<SkeletonPath>>() // caller Vec borrow before slice coercion
        + 2 * size_of::<Result<(), Diagnostic>>()
        + PAYMENT;
// Fixed invocation setup only. No per-block/transition receiver is donated
// from this bank to an unbounded number of loop advances.
const BODY: usize = 2 * size_of::<&Option<Box<crate::cleanup_plan::LexicalControlPlan>>>()
    + 2 * size_of::<&Vec<CleanupBlock>>()
    + 2 * size_of::<&[CleanupBlock]>()
    + 2 * size_of::<std::slice::Iter<'static, CleanupBlock>>()
    + 12 * size_of::<bool>() // fixed admission expression and conditional branches
    + 2 * size_of::<Diagnostic>() // at most one terminal refusal receiver
    + 2 * size_of::<(&mut ReplayBudget, &ResolvedFunction, &str, &str)>()
    + 4 * PAYMENT; // first block step, then PLAN, HIR-sort and comparison reservations
const BLOCK_STEP: usize = 2 * size_of::<&mut std::slice::Iter<'static, CleanupBlock>>()
    + 2 * size_of::<Option<&CleanupBlock>>()
    + 2 * size_of::<bool>()
    + 3 * PAYMENT; // block charge, inner setup, next block-step reservation
const TRANSITION_SETUP: usize = 2 * size_of::<&Vec<CleanupTransition>>()
    + 2 * size_of::<&[CleanupTransition]>()
    + 2 * size_of::<std::slice::Iter<'static, CleanupTransition>>()
    + PAYMENT; // first TRANSITION_STEP reservation
const TRANSITION_STEP: usize = 2 * size_of::<&mut std::slice::Iter<'static, CleanupTransition>>()
    + 2 * size_of::<Option<&CleanupTransition>>()
    + 4 * size_of::<bool>() // next Some/None and exact renewal-kind selection
    + 2 * PAYMENT; // transition charge and next transition-step reservation

pub(in crate::cleanup_plan::replay) fn compare(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    expected: &mut [SkeletonPath],
) -> Result<(), Diagnostic> {
    budget.reserve_bytes(
        function,
        BODY,
        "complete ordinary history finalizer receivers",
    )?;
    if budget.admission != AdmissionMode::Ordinary
        || !matches!(
            budget.payer,
            AllocationPayer::LegacyUnobserved | AllocationPayer::ActiveBuilder
        )
        || budget.lent_skeleton
        || budget.merge_paths
        || function.cleanup_plan.lexical_control.is_some()
    {
        return Err(budget.budget_refusal(
            function,
            "complete history finalizer requires its ordinary owner",
            "",
        ));
    }
    let mut blocks = function.cleanup_plan.blocks.iter();
    loop {
        budget.reserve_bytes(
            function,
            BLOCK_STEP,
            "complete history block advance receivers",
        )?;
        let Some(block) = blocks.next() else {
            break;
        };
        budget.charge(function, 1, "complete history finalizer block")?;
        budget.reserve_bytes(
            function,
            TRANSITION_SETUP,
            "complete history transition iterator receivers",
        )?;
        let mut transitions = block.transitions.iter();
        loop {
            budget.reserve_bytes(
                function,
                TRANSITION_STEP,
                "complete history transition advance receivers",
            )?;
            let Some(transition) = transitions.next() else {
                break;
            };
            budget.charge(function, 1, "complete history finalizer transition")?;
            if matches!(
                transition,
                CleanupTransition::ReserveRenewal { .. } | CleanupTransition::Renew { .. }
            ) {
                return Err(budget.budget_refusal(
                    function,
                    "complete history finalizer excludes renewal scope",
                    "",
                ));
            }
        }
    }
    // Independent PLAN construction precedes comparison/sort just as it does
    // in the original typed verifier. The two producers share no history ID.
    budget.reserve_bytes(
        function,
        view::PRODUCE_CALL,
        "complete independent PLAN history caller",
    )?;
    let produced = view::produce(function, budget)?;
    budget.reserve_bytes(
        function,
        enumerated_prefix::ENTRY_CALL,
        "complete HIR content sort caller",
    )?;
    enumerated_prefix::sort(expected, function, budget)?;
    budget.reserve_bytes(
        function,
        view::COMPARE_CALL,
        "complete independent content comparison caller",
    )?;
    view::compare(produced, expected, budget)
}

macro_rules! finish {
    ($function:ident, $budget:ident, $expected:ident) => {{
        if let $crate::cleanup_plan::replay::ReplayBudget {
            admission: $crate::cleanup_plan::replay::AdmissionMode::Ordinary,
            payer: $crate::cleanup_plan::replay::AllocationPayer::LegacyUnobserved
                | $crate::cleanup_plan::replay::AllocationPayer::ActiveBuilder,
            lent_skeleton: false, merge_paths: false, ..
        } = $budget {
            #[allow(clippy::redundant_pattern_matching,
                reason = "Unsupported lexical callers retain their original payer; no is_none ABI or Boolean result is constructed before admission.")]
            if let None = $function.cleanup_plan.lexical_control {
            $budget.reserve_bytes($function,
                $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::shipping::PROBE,
                "complete ordinary history finalizer probe")?;
                let mut shared = false;
                {
                    let mut paths = $expected.iter();
                    loop {
                        $budget.reserve_bytes($function,
                            $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::shipping::PROBE_STEP,
                            "complete ordinary history probe advance receivers")?;
                        let Some(path) = paths.next() else { break; };
                        $budget.charge($function, 1, "complete ordinary history finalizer probe")?;
                        if path.observations.prefix.is_some() { shared = true; break; }
                    }
                }
                if shared {
                    $budget.reserve_bytes($function,
                        $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::shipping::CALL,
                        "complete ordinary history finalizer caller")?;
                    return $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::shipping::compare(
                        $function, $budget, &mut $expected);
                }
            }
        }
    }};
}
pub(in crate::cleanup_plan::replay) use finish;
