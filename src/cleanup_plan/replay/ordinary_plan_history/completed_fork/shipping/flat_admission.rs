//! Fresh shape selection for independent PLAN histories over already-flat HIR.
//!
//! A branch is a real copy site in the old flat PLAN walk; a Goto is not. This
//! scan never certifies equality, shares HIR evidence, or enables the old pending
//! selector. Every accepted function still constructs PLAN independently and
//! passes the original complete content, terminal and multiplicity comparator.
use super::{
    AdmissionMode, AllocationPayer, Diagnostic, ReplayBudget, ResolvedFunction, SkeletonPath,
    PAYMENT,
};
use crate::bounded_output::classifier_string::ClassifierLedger;
use crate::cleanup_plan::replay::ordinary_receiving::{
    charge_into, prepare_into, refuse_into, reserve_into, Failure,
};
use crate::cleanup_plan::replay::replay_error;
#[cfg(test)]
use crate::cleanup_plan::replay::{ReplayWorkRefusal, REPLAY_WORK_REFUSAL};
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
// This producer writes the one invocation-owned error; successful checks form
// neither a Diagnostic nor a Result. Keep current() itself and every renewed
// ACTIVE clone/TLS receipt exactly as before (including sticky mismatch).
fn current_into(ledger: &ClassifierLedger, authenticated: &mut bool, failure: &mut Failure<'_>) {
    if failure.error.is_some() {
        *authenticated = false;
        return;
    }
    *authenticated = ledger.current();
    if !*authenticated {
        failure.error = Some(Diagnostic::io("SPX-H006", String::new()));
    }
}
const AUTH: usize = 2 * size_of::<(&ClassifierLedger, &mut bool, &mut Failure<'_>)>()
    + 2 * size_of::<&Option<Diagnostic>>()
    + 4 * size_of::<bool>();
// Macro expansion physically removes the returning method's Diagnostic sum.
// Retain the complete old argument transport conservatively. Add the lexical
// byte/phase bindings, both exact-source/payer ptr::eq actual/formal inputs,
// Option::is_some loans and Boolean branches for prepare/reserve/work/terminal.
// No old budget-internal bank or ACTIVE/TLS inventory is a credit here.
const INTO_PAYMENT: usize = 2 * size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>()
    + 2 * size_of::<usize>()
    + 2 * size_of::<&str>()
    + 4 * size_of::<*const ResolvedFunction>()
    + 4 * size_of::<*const ReplayBudget>()
    + 4 * size_of::<&Option<Diagnostic>>()
    + 24 * size_of::<bool>();
// One initialized output owner, reset at the same paid advance/check sites.
struct Scan<'a> {
    block: Option<&'a CleanupBlock>,
    transition: Option<&'a CleanupTransition>,
    authenticated: bool,
}
const OWNER: usize =
    Failure::acquisition_bytes() + AUTH_ERROR + size_of::<Scan<'_>>() + size_of::<bool>();
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
    + 4 * INTO_PAYMENT;
const TRANSITIONS: usize = 2 * size_of::<&Vec<CleanupTransition>>()
    + 4 * size_of::<&[CleanupTransition]>()
    + 2 * size_of::<std::slice::Iter<'static, CleanupTransition>>()
    + INTO_PAYMENT;
const TRANSITION: usize = 2 * size_of::<&mut std::slice::Iter<'static, CleanupTransition>>()
    + 2 * size_of::<Option<&CleanupTransition>>()
    + 2 * size_of::<&CleanupTransition>()
    + 2 * size_of::<bool>()
    + 2 * INTO_PAYMENT;
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
            + OWNER
            + AUTH
            + ClassifierLedger::capture_into_controls()
            + ClassifierLedger::current_receiving_controls(),
        "flat HIR independent PLAN selection setup",
    )?;
    let mut ledger = ClassifierLedger::empty_receiving();
    ledger.capture_current_into();
    let mut failure = Failure {
        error: None,
        function,
        budget: &*budget,
    };
    let mut scan = Scan {
        block: None,
        transition: None,
        authenticated: false,
    };
    // These lexical producers preserve the same debit sites and labels. A
    // terminal branch consumes the single paid first-error owner exactly once.
    macro_rules! paid {
        ($bytes:expr, $phase:expr) => {
            if !reserve_into!(budget, function, failure, $bytes, $phase) {
                return Err(failure
                    .error
                    .take()
                    .expect("flat selection first reservation error"));
            }
        };
    }
    macro_rules! work {
        ($phase:expr) => {
            if !charge_into!(budget, function, failure, 1, $phase) {
                return Err(failure
                    .error
                    .take()
                    .expect("flat selection first work error"));
            }
        };
    }
    macro_rules! authenticate {
        () => {
            current_into(&ledger, &mut scan.authenticated, &mut failure);
            if !scan.authenticated {
                return Err(failure
                    .error
                    .take()
                    .expect("flat selection first authentication error"));
            }
        };
    }
    authenticate!();
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
        paid!(
            BLOCK + AUTH + ClassifierLedger::current_receiving_controls(),
            "flat HIR PLAN block selection receivers"
        );
        authenticate!();
        scan.block = blocks.next();
        let Some(block) = scan.block else {
            break;
        };
        work!("flat HIR PLAN block selection");
        paid!(TRANSITIONS, "flat HIR PLAN transition iterator");
        let mut transitions = block.transitions.iter();
        loop {
            paid!(
                TRANSITION + AUTH + ClassifierLedger::current_receiving_controls(),
                "flat HIR PLAN transition selection receivers"
            );
            authenticate!();
            scan.transition = transitions.next();
            let Some(transition) = scan.transition else {
                break;
            };
            work!("flat HIR PLAN transition selection");
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
                paid!(EXIT, "flat HIR PLAN exit selection receivers");
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
    paid!(
        super::view::PRODUCE_CALL + AUTH + PAYMENT + ClassifierLedger::current_receiving_controls(),
        "flat HIR independent PLAN caller"
    );
    authenticate!();
    let produced = super::view::produce(function, budget)?;
    paid!(
        super::enumerated_prefix::ENTRY_CALL + PAYMENT,
        "flat HIR complete content sort caller"
    );
    super::enumerated_prefix::sort(expected, function, budget)?;
    paid!(
        super::view::COMPARE_CALL,
        "flat HIR independent full comparison caller"
    );
    super::view::compare(produced, expected, budget)?;
    Ok(true)
}

#[cfg(test)]
#[path = "flat_admission/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "flat_admission/immediate_before.rs"]
mod immediate_before;
