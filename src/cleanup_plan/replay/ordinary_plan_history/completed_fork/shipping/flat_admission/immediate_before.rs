//! Fresh shape selection for independent PLAN histories over already-flat HIR.
//!
//! A branch is a real copy site in the old flat PLAN walk; a Goto is not. This
//! scan never certifies equality, shares HIR evidence, or enables the old pending
//! selector. Every accepted function still constructs PLAN independently and
//! passes the original complete content, terminal and multiplicity comparator.
use super::super::{
    AdmissionMode, AllocationPayer, Diagnostic, ReplayBudget, ResolvedFunction, SkeletonPath,
    PAYMENT,
};
use crate::bounded_output::classifier_string::ClassifierLedger;
use crate::cleanup_plan::{
    CleanupBlock, CleanupTerminator, CleanupTransition, ExitContinuation, ExitTarget, ExitTargetId,
};
use std::mem::size_of;

pub(in crate::cleanup_plan::replay) const CALL: usize =
    2 * size_of::<(&ResolvedFunction, &mut ReplayBudget, &mut [SkeletonPath])>()
        + 2 * size_of::<Result<bool, Diagnostic>>()
        + 2 * size_of::<&mut Vec<SkeletonPath>>()
        + 2 * size_of::<bool>()
        + PAYMENT;
const AUTH_ERROR: usize =
    2 * size_of::<Diagnostic>() + 2 * size_of::<String>() + 2 * size_of::<&str>();
fn current(ledger: &ClassifierLedger) -> Result<(), Diagnostic> {
    if ledger.current() {
        Ok(())
    } else {
        Err(Diagnostic::io("SPX-H006", String::new()))
    }
}
const AUTH: usize =
    2 * size_of::<&ClassifierLedger>() + 2 * size_of::<Result<(), Diagnostic>>() + AUTH_ERROR;
const SETUP: usize = 2 * size_of::<ClassifierLedger>()
    + 2 * size_of::<&Vec<CleanupBlock>>()
    + 4 * size_of::<&[CleanupBlock]>()
    + 2 * size_of::<std::slice::Iter<'static, CleanupBlock>>()
    + 12 * size_of::<bool>()
    + 2 * PAYMENT;
// The current-ledger calls below are separately prepaid at each reached step.
// Each iterator next, its terminal None, work call and following reservation
// has a distinct renewed bank. No setup frame funds an unbounded loop.
const BLOCK: usize = 2 * size_of::<&mut std::slice::Iter<'static, CleanupBlock>>()
    + 2 * size_of::<Option<&CleanupBlock>>()
    + 2 * size_of::<&CleanupBlock>()
    + 2 * size_of::<&ExitTargetId>()
    + 2 * size_of::<&CleanupTerminator>()
    + 2 * size_of::<&Vec<crate::cleanup_plan::EdgeId>>()
    + 2 * size_of::<usize>()
    + 6 * size_of::<bool>()
    + 4 * PAYMENT;
const TRANSITIONS: usize = 2 * size_of::<&Vec<CleanupTransition>>()
    + 4 * size_of::<&[CleanupTransition]>()
    + 2 * size_of::<std::slice::Iter<'static, CleanupTransition>>()
    + PAYMENT;
const TRANSITION: usize = 2 * size_of::<&mut std::slice::Iter<'static, CleanupTransition>>()
    + 2 * size_of::<Option<&CleanupTransition>>()
    + 2 * size_of::<&CleanupTransition>()
    + 2 * size_of::<bool>()
    + 2 * PAYMENT;
const EXIT: usize = 2 * size_of::<&Vec<ExitTarget>>()
    + 4 * size_of::<&[ExitTarget]>()
    + 4 * size_of::<usize>() // ID cast, get actual/formal index and receive
    + 2 * size_of::<Option<&ExitTarget>>()
    + 2 * size_of::<&ExitTarget>()
    + 2 * size_of::<&ExitContinuation>()
    + 2 * size_of::<bool>();

pub(in crate::cleanup_plan::replay) fn try_compare(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    expected: &mut [SkeletonPath],
) -> Result<bool, Diagnostic> {
    budget.reserve_bytes(
        function,
        SETUP
            + AUTH
            + ClassifierLedger::capture_into_controls()
            + ClassifierLedger::current_receiving_controls(),
        "flat HIR independent PLAN selection setup",
    )?;
    let mut ledger = ClassifierLedger::empty_receiving();
    ledger.capture_current_into();
    current(&ledger)?;
    // CALL pays these received places and selection results. Unsupported payers
    // are normally excluded by finish_current before it reserves CALL at all.
    match budget {
        ReplayBudget {
            admission: AdmissionMode::Ordinary,
            payer: AllocationPayer::LegacyUnobserved | AllocationPayer::ActiveBuilder,
            lent_skeleton: false,
            merge_paths: false,
            ..
        } => {}
        _ => return Ok(false),
    }
    match function.cleanup_plan.lexical_control {
        None => {}
        Some(_) => return Ok(false),
    }
    let mut branch = false;
    let mut blocks = function.cleanup_plan.blocks.iter();
    loop {
        budget.reserve_bytes(
            function,
            BLOCK + AUTH + ClassifierLedger::current_receiving_controls(),
            "flat HIR PLAN block selection receivers",
        )?;
        current(&ledger)?;
        let Some(block) = blocks.next() else {
            break;
        };
        budget.charge(function, 1, "flat HIR PLAN block selection")?;
        budget.reserve_bytes(function, TRANSITIONS, "flat HIR PLAN transition iterator")?;
        let mut transitions = block.transitions.iter();
        loop {
            budget.reserve_bytes(
                function,
                TRANSITION + AUTH + ClassifierLedger::current_receiving_controls(),
                "flat HIR PLAN transition selection receivers",
            )?;
            current(&ledger)?;
            let Some(transition) = transitions.next() else {
                break;
            };
            budget.charge(function, 1, "flat HIR PLAN transition selection")?;
            match transition {
                CleanupTransition::ReserveRenewal { .. } | CleanupTransition::Renew { .. } => {
                    return Ok(false)
                }
                CleanupTransition::Initialize { .. }
                | CleanupTransition::Transfer { .. }
                | CleanupTransition::InitializeVariant { .. }
                | CleanupTransition::TransferVariant { .. }
                | CleanupTransition::AuthenticateVariantCase { .. }
                | CleanupTransition::CallCommit { .. }
                | CleanupTransition::SelectFailure { .. }
                | CleanupTransition::StageCopyResult { .. } => {}
            }
        }
        match &block.terminator {
            CleanupTerminator::Goto(_) => {}
            CleanupTerminator::Branch(edges) => {
                if edges.len() > 1 {
                    branch = true;
                }
            }
            CleanupTerminator::Exit(index) => {
                budget.reserve_bytes(function, EXIT, "flat HIR PLAN exit selection receivers")?;
                let Some(exit) = function.cleanup_plan.exits.get(index.0 as usize) else {
                    return Ok(false);
                };
                match &exit.continuation {
                    ExitContinuation::Continue(_)
                    | ExitContinuation::CommitResult { .. }
                    | ExitContinuation::ReturnFailure { .. }
                    | ExitContinuation::ReturnUnit => {}
                    ExitContinuation::LoopBreak { .. }
                    | ExitContinuation::LoopLatch { .. }
                    | ExitContinuation::FunctionReturn { .. } => return Ok(false),
                }
            }
        }
    }
    if !branch {
        return Ok(false);
    }
    // Preserve PLAN-before-HIR-sort failure ordering. Neither producer derives
    // observations from the other, and no pointer/ordinal authorizes equality.
    budget.reserve_bytes(
        function,
        super::super::view::PRODUCE_CALL + AUTH + PAYMENT + ClassifierLedger::current_receiving_controls(),
        "flat HIR independent PLAN caller",
    )?;
    current(&ledger)?;
    let produced = super::super::view::produce(function, budget)?;
    budget.reserve_bytes(
        function,
        super::super::enumerated_prefix::ENTRY_CALL + PAYMENT,
        "flat HIR complete content sort caller",
    )?;
    super::super::enumerated_prefix::sort(expected, function, budget)?;
    budget.reserve_bytes(
        function,
        super::super::view::COMPARE_CALL,
        "flat HIR independent full comparison caller",
    )?;
    super::super::view::compare(produced, expected, budget)?;
    Ok(true)
}
