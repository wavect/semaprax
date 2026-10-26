//! Complete fork product in the existing immutable sequence carrier.
//! No selection gate, minimum tail length, equality cache or alternative union.
use super::*;
use skeleton_work::enumerated_prefix as sequence;
use skeleton_work::{ObservationDag, ObservationDagKind};

pub(in crate::cleanup_plan::replay) enum Emission {
    Unsupported(Vec<ExprSkeletonPath>),
    Complete(Vec<ExprSkeletonPath>),
}

// Actual/formal owned input, borrowed suffix, work and returned owners. The
// per-path bank below is renewed before every partial output construction.
pub(in crate::cleanup_plan::replay) const ENTRY: usize = 2 * size_of::<Vec<ExprSkeletonPath>>()
    + 2 * size_of::<&[ExprSkeletonPath]>()
    + 2 * size_of::<&mut SkeletonWork<'static, 'static>>()
    + size_of::<Emission>()
    + size_of::<Result<Emission, Diagnostic>>()
    + size_of::<Vec<ExprSkeletonPath>>()
    + 2 * size_of::<ExprSkeletonPath>()
    + size_of::<std::vec::IntoIter<ExprSkeletonPath>>()
    + size_of::<Option<ExprSkeletonPath>>()
    + 2 * size_of::<ClassifierLedger>()
    + 12 * size_of::<bool>()
    + PAYMENT;
// These are the actual emitter's move/clone/extension/publication roles, not
// the older arena Path/Chunk layout. Callee backing remains separately paid.
const STEP: usize = 2 * size_of::<ExprSkeletonPath>()
    + 2 * size_of::<Observations>()
    + 4 * size_of::<Option<Observations>>() // retained owner and owned expect actual/formal
    + 2 * size_of::<&Option<Observations>>() // as_ref actual/formal receiver
    + 2 * size_of::<&mut Option<Observations>>() // take actual/formal receiver
    + 4 * size_of::<Option<&Observations>>() // as_ref result/receive, expect actual/formal
    + 2 * size_of::<Option<CleanupPlace>>()
    + 2 * size_of::<lexical_skeleton::Continuation>()
    + 4 * size_of::<&ExprSkeletonPath>()
    + 4 * size_of::<&Observations>()
    + 2 * size_of::<&Option<CleanupPlace>>()
    + 2 * size_of::<&mut Vec<ExprSkeletonPath>>()
    + 2 * size_of::<Result<Observations, Diagnostic>>()
    + 2 * size_of::<Result<Option<CleanupPlace>, Diagnostic>>()
    + 6 * size_of::<&mut SkeletonWork<'static, 'static>>()
    + 2 * size_of::<std::iter::Enumerate<std::slice::Iter<'static, ExprSkeletonPath>>>()
    + 2 * size_of::<Option<(usize, &ExprSkeletonPath)>>()
    + 4 * size_of::<usize>()
    + 8 * size_of::<bool>()
    + 6 * size_of::<&str>()
    + 2 * size_of::<Result<(), Diagnostic>>() // push result and caller receive
    + 4 * size_of::<&lexical_skeleton::Continuation>() // sequenced actual/formal args
    + 2 * PAYMENT;
const FREEZE: usize = 2 * size_of::<Observations>()
    + 2 * size_of::<Option<Rc<ObservationDag>>>()
    + 2 * size_of::<Vec<Rc<SkeletonObservation>>>()
    + size_of::<ObservationDagKind>();
const VALIDATE: usize = 2 * size_of::<(&Observations, &mut SkeletonWork<'static, 'static>)>()
    + size_of::<Result<bool, Diagnostic>>()
    + size_of::<Vec<&ObservationDag>>()
    + 2 * size_of::<&mut Vec<&ObservationDag>>()
    + 2 * size_of::<&ObservationDag>()
    + 4 * size_of::<usize>()
    + 4 * size_of::<Option<&ObservationDag>>()
    + 2 * size_of::<&ObservationDagKind>()
    + size_of::<std::slice::Iter<'static, Rc<ObservationDag>>>()
    + 2 * size_of::<Option<&Rc<ObservationDag>>>()
    + 4 * size_of::<bool>()
    + PAYMENT;

// Inspect real node kinds, never a representation marker or shared identity.
// Each borrowed traversal slot/backing is paid before publication. No recursive
// traversal, observation copy, target scan or reusable classification authority.
#[allow(dead_code, reason = "complete immediate-old sequence classifier stays independently executable")]
pub(in crate::cleanup_plan::replay) fn sequence_only(
    observations: &Observations,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<bool, Diagnostic> {
    let mut pending = Vec::new();
    if let Some(root) = &observations.prefix {
        work.budget.reserve_vec(
            work.function,
            &mut pending,
            1,
            "complete HIR sequence validation backing",
        )?;
        pending.push(root.as_ref());
    }
    while let Some(node) = pending.pop() {
        work.budget
            .charge(work.function, 1, "complete HIR sequence validation")?;
        work.budget.reserve_bytes(
            work.function,
            VALIDATE,
            "complete HIR sequence validation step",
        )?;
        match &node.kind {
            ObservationDagKind::Alt(_) => return Ok(false),
            ObservationDagKind::Seq(prefix, _) => {
                if let Some(prefix) = prefix {
                    work.budget.reserve_vec(
                        work.function,
                        &mut pending,
                        1,
                        "complete HIR sequence ancestor backing",
                    )?;
                    pending.push(prefix.as_ref());
                }
            }
            ObservationDagKind::Cat(children) => {
                work.budget.reserve_vec(
                    work.function,
                    &mut pending,
                    children.len(),
                    "complete HIR sequence concatenation backing",
                )?;
                for child in children {
                    pending.push(child.as_ref());
                }
            }
        }
    }
    Ok(true)
}

#[allow(
    unused_macros,
    reason = "whole-caller qualification precedes shipping selection"
)]
macro_rules! emit {
    ($prefixes:expr, $suffixes:expr, $work:ident) => {{
        #[cfg(test)]
        {
            if $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::immediate_selected() {
                $work.budget.reserve_bytes($work.function,
                    $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::ENTRY,
                    "complete HIR fork caller receivers")?;
                $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::emit_prepaid(
                    $prefixes, $suffixes, $work,
                )
            } else {
                $work.budget.reserve_bytes($work.function,
                    $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::ENTRY
                        + $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::retained::ACQUIRE,
                    "complete HIR fork caller receivers")?;
                $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::retained::emit!(
                    $prefixes, $suffixes, $work,
                )
            }
        }
        #[cfg(not(test))]
        {
            $work.budget.reserve_bytes($work.function,
                $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::ENTRY
                    + $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::retained::ACQUIRE,
                "complete HIR fork caller receivers")?;
            $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::retained::emit!(
                $prefixes, $suffixes, $work,
            )
        }
    }};
}
#[allow(
    unused_imports,
    reason = "whole-caller qualification precedes shipping selection"
)]
pub(in crate::cleanup_plan::replay) use emit;

#[cfg(test)]
pub(in crate::cleanup_plan::replay) fn emit_prepaid(
    prefixes: Vec<ExprSkeletonPath>,
    suffixes: &[ExprSkeletonPath],
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Emission, Diagnostic> {
    work.budget.reserve_bytes(
        work.function,
        ClassifierLedger::capture_into_controls()
            + ClassifierLedger::current_receiving_controls()
            + PAYMENT,
        "complete HIR fork exact ledger receivers",
    )?;
    let mut ledger = ClassifierLedger::empty_receiving();
    ledger.capture_current_into();
    if work.budget.admission != AdmissionMode::Ordinary
        || !matches!(
            work.budget.payer,
            AllocationPayer::LegacyUnobserved | AllocationPayer::ActiveBuilder
        )
        || work.budget.lent_skeleton
        || work.budget.merge_paths
        || work.function.cleanup_plan.lexical_control.is_some()
    {
        return Ok(Emission::Unsupported(prefixes));
    }
    work.budget.reserve_bytes(
        work.function,
        sequence::SHARE_CONTROLS,
        "complete HIR renewal selection receivers",
    )?;
    if !sequence::without_renewal(work)? {
        return Ok(Emission::Unsupported(prefixes));
    }
    work.budget.reserve_bytes(
        work.function,
        validation::CALL,
        "complete HIR validation scope receivers",
    )?;
    if !validation::inputs(&prefixes, suffixes, work)? {
        return Ok(Emission::Unsupported(prefixes));
    }
    let mut result = Vec::new();
    for mut prefix in prefixes {
        work.budget
            .reserve_bytes(work.function, STEP, "complete HIR fork prefix receivers")?;
        if prefix.failed || prefix.residual {
            if result.len() >= MAX_REPLAY_PATHS {
                return Err(work.budget.budget_refusal(
                    work.function,
                    "complete HIR fork exceeds finite path profile",
                    "",
                ));
            }
            work.push_expr_path(&mut result, prefix, "short-circuited skeleton path")?;
            continue;
        }
        if suffixes.is_empty() {
            continue;
        }
        // The immediate predecessor's first clone work wins before new node
        // metadata. Later suffixes each retain their original clone work too.
        work.charge(1, "skeleton prefix clone")?;
        if suffixes.len() > 1 && !prefix.observations.items.is_empty() {
            work.budget.reserve_bytes(
                work.function,
                sequence::SHARE_CONTROLS + FREEZE,
                "complete HIR immutable fork publication",
            )?;
            let Observations {
                prefix: previous,
                items,
            } = prefix.observations;
            let kind = ObservationDagKind::Seq(previous, items);
            prefix.observations = Observations {
                prefix: Some(sequence::node(work, kind)?),
                items: Vec::new(),
            };
        }
        let mut remaining = Some(prefix.observations);
        for (index, suffix) in suffixes.iter().enumerate() {
            if !ledger.current() {
                return Err(Diagnostic::io("SPX-H006", String::new()));
            }
            work.budget.reserve_bytes(
                work.function,
                STEP + ClassifierLedger::current_receiving_controls(),
                "complete HIR fork output receivers",
            )?;
            if index != 0 {
                work.charge(1, "skeleton prefix clone")?;
            }
            let observations = if index + 1 == suffixes.len() {
                note_skeleton_materialization();
                remaining
                    .take()
                    .expect("final suffix moves the unique remaining prefix")
            } else {
                let source = remaining
                    .as_ref()
                    .expect("nonfinal suffix retains prefix owner");
                work.budget
                    .reserve_clone(work.function, source, "skeleton prefix clone")?;
                note_skeleton_materialization();
                source.clone()
            };
            let observations = if suffix.observations.prefix.is_some() {
                // Exactly the original suffix event; the existing sequence
                // factory charges additional bound/release traversal as free
                // work on BOTH public and builder payers, never the held loan.
                work.charge(1, "skeleton suffix clone")?;
                note_skeleton_materialization();
                work.budget.reserve_bytes(
                    work.function,
                    sequence::EXTEND_CONTROLS,
                    "complete HIR shared suffix call receivers",
                )?;
                sequence::extend(work, observations, &suffix.observations)?
            } else {
                work.extend_owned_observations(
                    observations,
                    &suffix.observations,
                    "skeleton suffix clone",
                )?
            };
            let owned_source = work.clone_owned(
                &suffix.owned_source,
                "sequenced skeleton owned-source clone",
            )?;
            if result.len() >= MAX_REPLAY_PATHS {
                return Err(work.budget.budget_refusal(
                    work.function,
                    "complete HIR fork exceeds finite path profile",
                    "",
                ));
            }
            work.push_expr_path(
                &mut result,
                ExprSkeletonPath {
                    observations,
                    owned_source,
                    failed: suffix.failed,
                    residual: suffix.residual,
                    continuation: lexical_skeleton::Continuation::sequenced(
                        &prefix.continuation,
                        &suffix.continuation,
                    ),
                },
                "sequenced skeleton path",
            )?;
        }
    }
    Ok(Emission::Complete(result))
}

// The original caller's actual-payer pattern is the authority boundary. It
// neither consults nor debits a Selected/Oracle/lent/merged/lexical owner.
pub(in crate::cleanup_plan::replay) const DISPATCH: usize = 2 * size_of::<&[ExprSkeletonPath]>()
    + 2 * size_of::<Option<&ExprSkeletonPath>>()
    + 2 * size_of::<&Option<Rc<ObservationDag>>>()
    + 2 * size_of::<usize>()
    + 8 * size_of::<bool>()
    + PAYMENT;
macro_rules! dispatch {
    ($prefixes:ident, $suffixes:ident, $work:ident) => {{
        #[cfg(test)]
        if $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::before_selected() {
            return $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::whole_sequence!($prefixes, $suffixes, $work);
        }
        if let $crate::cleanup_plan::replay::ReplayBudget {
            admission: $crate::cleanup_plan::replay::AdmissionMode::Ordinary,
            payer: $crate::cleanup_plan::replay::AllocationPayer::LegacyUnobserved
                | $crate::cleanup_plan::replay::AllocationPayer::ActiveBuilder,
            lent_skeleton: false, merge_paths: false, ..
        } = $work.budget {
            #[allow(clippy::redundant_pattern_matching,
                reason = "Unsupported lexical callers retain their original payer; pattern matching creates no new is_none receiver or Boolean result before admission.")]
            if let None = $work.function.cleanup_plan.lexical_control {
            $work.budget.reserve_bytes($work.function,
                $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::DISPATCH,
                "complete HIR fork dispatch receivers")?;
            if $suffixes.len() > 1 || match $suffixes.first() {
                    Some(path) => path.observations.prefix.is_some(), None => false,
                } {
                match $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::emit!($prefixes, $suffixes, $work)? {
                    $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::Emission::Complete(paths) => return Ok(paths),
                    $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::Emission::Unsupported(paths) => paths,
                }
            } else { $prefixes }
            } else { $prefixes }
        } else { $prefixes }
    }};
}
pub(in crate::cleanup_plan::replay) use dispatch;

// A nested call argument is independently enumerated before its caller's
// Cartesian product. The public ordinary receiver must concatenate its real
// shared sequence without switching to the old factored skeleton-loan payer.
// No original helper body changes, including the independent old receiver.
pub(in crate::cleanup_plan::replay) const CALL_BRIDGE: usize = 2 * VALIDATE
    + 2 * size_of::<Result<Observations, Diagnostic>>()
    + 4 * size_of::<bool>()
    + 3 * PAYMENT; // validation, original suffix work, sequence helper acquisition
macro_rules! extend_call_current {
    ($target:ident, $suffix:ident, $work:ident) => {{
        if let $crate::cleanup_plan::replay::ReplayBudget {
            admission: $crate::cleanup_plan::replay::AdmissionMode::Ordinary,
            payer: $crate::cleanup_plan::replay::AllocationPayer::LegacyUnobserved,
            lent_skeleton: false, merge_paths: false, ..
        } = $work.budget {
            #[allow(clippy::redundant_pattern_matching,
                reason = "Inspect existing places without acquiring receivers on an unsupported lender.")]
            if let None = $work.function.cleanup_plan.lexical_control {
                #[allow(clippy::redundant_pattern_matching,
                    reason = "Match the existing suffix place before any new producer is admitted.")]
                if let Some(_) = $suffix.observations.prefix {
                $work.budget.reserve_bytes($work.function,
                    $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::CALL_BRIDGE,
                    "shared call argument bridge receivers")?;
                if $crate::cleanup_plan::replay::skeleton_work::query_proof::sequence!(&$target, $work)?
                    && $crate::cleanup_plan::replay::skeleton_work::query_proof::sequence!(&$suffix.observations, $work)? {
                    $work.charge(1, "call suffix clone")?;
                    $crate::cleanup_plan::replay::note_skeleton_materialization();
                    $work.budget.reserve_bytes($work.function,
                        $crate::cleanup_plan::replay::skeleton_work::enumerated_prefix::EXTEND_CONTROLS,
                        "shared call argument sequence receivers")?;
                    $crate::cleanup_plan::replay::skeleton_work::enumerated_prefix::extend($work, $target, &$suffix.observations)
                } else { $work.extend_owned_observations($target, &$suffix.observations, "call suffix clone") }
                } else { $work.extend_owned_observations($target, &$suffix.observations, "call suffix clone") }
            } else { $work.extend_owned_observations($target, &$suffix.observations, "call suffix clone") }
        } else { $work.extend_owned_observations($target, &$suffix.observations, "call suffix clone") }
    }};
}
pub(in crate::cleanup_plan::replay) use extend_call_current;
macro_rules! extend_call {
    ($target:ident, $suffix:ident, $work:ident) => {{
        #[cfg(test)]
        {
            if $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::before_selected() {
                $work.extend_owned_observations($target, &$suffix.observations, "call suffix clone")
            } else {
                $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::extend_call_current!($target, $suffix, $work)
            }
        }
        #[cfg(not(test))]
        $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::extend_call_current!($target, $suffix, $work)
    }};
}
pub(in crate::cleanup_plan::replay) use extend_call;

#[cfg(test)]
mod tests;

// Explicit independent old whole-HIR receiver. The default invokes actual
// production dispatch; no observer or accounting callback selects either one.
#[cfg(test)]
thread_local! { static SELECTED: std::cell::Cell<u8> = const { std::cell::Cell::new(0) }; }
#[cfg(test)]
pub(in crate::cleanup_plan::replay) fn selected() -> bool {
    SELECTED.with(|selected| selected.get() != 0)
}
#[cfg(test)]
fn with_route<T>(mode: u8, operation: impl FnOnce() -> T) -> T {
    struct Restore(u8);
    impl Drop for Restore {
        fn drop(&mut self) {
            SELECTED.with(|selected| selected.set(self.0));
        }
    }
    let _restore = Restore(SELECTED.with(|selected| selected.replace(mode)));
    operation()
}
// This is a test-harness switch to the complete prior function, not another
// production producer. It adds no candidate entry/query/forwarding fee to the
// old baseline; that function retains every original payment and work call.
#[cfg(test)]
macro_rules! whole_sequence {
    ($prefixes:expr, $suffixes:expr, $work:ident) => {{
        $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::fallback(
            $prefixes, $suffixes, $work,
        )
    }};
}
#[cfg(test)]
pub(in crate::cleanup_plan::replay) use whole_sequence;

#[cfg(test)]
#[path = "emission/whole_tests.rs"]
mod whole_tests;

#[cfg(test)]
pub(in crate::cleanup_plan::replay) fn fallback(
    prefixes: Vec<ExprSkeletonPath>,
    suffixes: &[ExprSkeletonPath],
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    super::before::sequence_skeleton_paths(prefixes, suffixes, work)
}

#[cfg(test)]
pub(in crate::cleanup_plan::replay) fn before_selected() -> bool {
    SELECTED.with(|selected| selected.get() == 2)
}
#[cfg(test)]
fn with_emission<T>(operation: impl FnOnce() -> T) -> T {
    with_route(1, operation)
}
#[cfg(test)]
pub(in crate::cleanup_plan::replay) fn with_before<T>(operation: impl FnOnce() -> T) -> T {
    with_route(2, operation)
}

pub(in crate::cleanup_plan::replay) mod validation;

#[cfg(test)]
pub(in crate::cleanup_plan::replay) fn shared_query_inputs_for_test(
    prefixes: &[ExprSkeletonPath], suffixes: &[ExprSkeletonPath],
    work: &mut SkeletonWork<'_, '_>,
) -> Result<bool, Diagnostic> {
    work.budget.reserve_bytes(work.function, validation::CALL,
        "test complete sequence validation caller")?;
    validation::inputs(prefixes, suffixes, work)
}

// The complete immediate emitter, STEP/ENTRY invoices and dispatch above stay
// independently executable. Only the selected shipping producer owns this frame.
pub(in crate::cleanup_plan::replay) mod retained;

#[cfg(test)]
pub(in crate::cleanup_plan::replay) fn immediate_selected() -> bool {
    SELECTED.with(|selected| selected.get() == 3)
}
#[cfg(test)]
fn with_immediate<T>(operation: impl FnOnce() -> T) -> T {
    with_route(3, operation)
}
