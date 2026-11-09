//! Lexical invocation-owned fork output and sticky error places (#925).
//! The input allocation is retained only for a true singleton Cartesian suffix.
use super::*;

// Full original ENTRY remains prepaid. Only the retained-only Output/Failure
// owners and helper argument boundaries physically removed below are retired.
// The lexical completion block still constructs/receives its Boolean, and the
// observations/error/reset/take owners remain paid before formation.
pub(in crate::cleanup_plan::replay) const FREEZE_RECEIVERS: usize = FREEZE;
pub(in crate::cleanup_plan::replay) const PAYMENT_RECEIVERS: usize = PAYMENT;

pub(in crate::cleanup_plan::replay) const ACQUIRE: usize = size_of::<Vec<ExprSkeletonPath>>() // lexical paths receiver, not an aggregate credit
    + 2 * size_of::<Observations>() // default initializer and lexical owner
    + 5 * size_of::<Option<Diagnostic>>() // None initializer/slot, Some publication/receive and terminal carrier
    + size_of::<Diagnostic>()
    + size_of::<std::borrow::Cow<'static, str>>() // retain the complete first terminal bank
    + 2 * size_of::<bool>()
    + 2 * size_of::<&mut Observations>()
    + 4 * size_of::<Vec<ExprSkeletonPath>>() // complete general take/reset carrier bank
    + PAYMENT;
// Prefix/output iterator, backing, clone, work and publication banks survive.
pub(in crate::cleanup_plan::replay) const PREFIX: usize = 2 * size_of::<ExprSkeletonPath>()
    + 2 * size_of::<&ExprSkeletonPath>()
    + 2 * size_of::<&mut SkeletonWork<'static, 'static>>()
    + 2 * size_of::<Result<(), Diagnostic>>()
    + 4 * size_of::<usize>()
    + 2 * size_of::<&[ExprSkeletonPath]>()
    + 2 * size_of::<std::slice::Iter<'static, ExprSkeletonPath>>()
    + 2 * size_of::<&mut std::slice::Iter<'static, ExprSkeletonPath>>()
    + 2 * size_of::<Option<&ExprSkeletonPath>>()
    + 2 * size_of::<&mut std::slice::IterMut<'static, ExprSkeletonPath>>()
    + 2 * size_of::<Option<&mut ExprSkeletonPath>>()
    + 2 * size_of::<Option<(&ExprSkeletonPath, &[ExprSkeletonPath])>>()
    + 2 * size_of::<(&ExprSkeletonPath, &[ExprSkeletonPath])>()
    + 8 * size_of::<bool>()
    + 2 * PAYMENT;
// Keep every surviving original clone/node/owned-source/publication transport
// paid. Only the actually absent Option/take/expect roles and Enumerate/index
// transport are removed; the new borrowed split/slot/swap roles are explicit.
pub(in crate::cleanup_plan::replay) const OUTPUT: usize = STEP
    - 2 * size_of::<Result<Observations, Diagnostic>>() // old returning extension + caller receiver; lexical places form neither
    - 4 * size_of::<Option<Observations>>()
    - 2 * size_of::<&Option<Observations>>()
    - 2 * size_of::<&mut Option<Observations>>()
    - 4 * size_of::<Option<&Observations>>()
    - 2 * size_of::<std::iter::Enumerate<std::slice::Iter<'static, ExprSkeletonPath>>>()
    - 2 * size_of::<Option<(usize, &ExprSkeletonPath)>>()
    + 2 * size_of::<std::slice::Iter<'static, ExprSkeletonPath>>()
    + 2 * size_of::<&mut std::slice::Iter<'static, ExprSkeletonPath>>()
    + 2 * size_of::<Option<&ExprSkeletonPath>>()
    + 8 * size_of::<&mut Observations>() // two swaps, two operands, actual/formal
    + 2 * size_of::<&mut ExprSkeletonPath>()
    + 2 * size_of::<Observations>()
    // Inline match scrutinee loan and Some(prefix) binding precede shared payment.
    + size_of::<&Option<Rc<ObservationDag>>>()
    + size_of::<&Rc<ObservationDag>>()
; // The append/extension Boolean return/receive boundaries are absent.
pub(in crate::cleanup_plan::replay) const SELECT: usize = 2 * size_of::<&[ExprSkeletonPath]>()
    + 2 * size_of::<&Vec<ExprSkeletonPath>>()
    + 2 * size_of::<Option<(&ExprSkeletonPath, &[ExprSkeletonPath])>>()
    + 2 * size_of::<(&ExprSkeletonPath, &[ExprSkeletonPath])>()
    + 2 * size_of::<&mut Vec<ExprSkeletonPath>>()
    + 2 * size_of::<&ExprSkeletonPath>() // singleton borrowed slot: actual and local receiver
    + 2 * size_of::<std::slice::IterMut<'static, ExprSkeletonPath>>()
    + 2 * size_of::<Option<&mut ExprSkeletonPath>>()
    + 4 * size_of::<usize>()
    + 4 * size_of::<bool>()
    + PAYMENT;
// The extension changes only the output ABI. No credit is taken from the old
// sequence EXTEND_CONTROLS: every node/backing/clone bank remains complete.
pub(in crate::cleanup_plan::replay) const EXTEND: usize = sequence::EXTEND_CONTROLS
    + 4 * size_of::<&mut Option<Rc<ObservationDag>>>()
    + 4 * size_of::<&mut Vec<Rc<SkeletonObservation>>>()
    + 4 * size_of::<Vec<Rc<SkeletonObservation>>>() // take: default, moved replacement and taken return/receive
    + 2 * size_of::<Observations>()
; // Lexical extension has no returned status owner.

// A retained first error never crosses an Output/Failure forwarding ABI.
// Every existing returning callee keeps its full original Result tariff.
macro_rules! receive {
    ($error:ident, $exit:lifetime, $operation:expr) => {{
        match $operation {
            Ok(value) => value,
            Err(error) => {
                $error = Some(error);
                break $exit false;
            }
        }
    }};
}
pub(in crate::cleanup_plan::replay) use receive;
macro_rules! pay {
    ($error:ident, $exit:lifetime, $work:ident, $bytes:expr, $phase:literal) => {{
        $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::retained::receive!(
            $error, $exit, $work.budget.reserve_bytes($work.function, $bytes, $phase)
        );
    }};
}
pub(in crate::cleanup_plan::replay) use pay;
macro_rules! extend {
    ($target:ident, $suffix:ident, $work:ident, $error:ident, $exit:lifetime) => {{
        use $crate::cleanup_plan::replay::note_skeleton_materialization;
        use $crate::cleanup_plan::replay::skeleton_work::{ObservationDag, ObservationDagKind};
        use $crate::cleanup_plan::replay::skeleton_work::enumerated_prefix as sequence;
        use $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::retained;
        use std::rc::Rc;
    match &$suffix.observations.prefix {
    None => {
        retained::receive!($error, $exit, $work.charge(1, "skeleton suffix clone"));
        note_skeleton_materialization();
        retained::receive!($error, $exit,
            $work.budget.reserve_vec(
                $work.function,
                &mut $target.items,
                $suffix.observations.items.len(),
                "ordinary flat suffix"
            )
        );
        $target.items.extend_from_slice(&$suffix.observations.items);
    },
    Some(prefix) => {
    retained::receive!($error, $exit, $work.charge(1, "skeleton suffix clone"));
    note_skeleton_materialization();
    retained::receive!($error, $exit,
        $work.budget.reserve_bytes(
            $work.function,
            retained::EXTEND,
            "complete HIR shared suffix call receivers"
        )
    );
    let head = if $target.items.is_empty() {
        $target.prefix.take()
    } else {
        Some(retained::receive!($error, $exit,
            sequence::node(
                $work,
                ObservationDagKind::Seq($target.prefix.take(), std::mem::take(&mut $target.items))
            )
        ))
    };
    let joined = match head {
        None => prefix.clone(),
        Some(head) => {
            retained::receive!($error, $exit,
                $work.budget.reserve_array::<Rc<ObservationDag>>(
                    $work.function,
                    2,
                    "enumerated HIR concatenation backing"
                )
            );
            retained::receive!($error, $exit,
                sequence::node($work, ObservationDagKind::Cat(vec![head, prefix.clone()]))
            )
        }
    };
    retained::receive!($error, $exit,
        $work.budget
            .reserve_clone($work.function, &$suffix.observations.items, "ordinary DAG suffix tail")
    );
    $target.prefix = Some(joined);
    $target.items = $suffix.observations.items.clone();
    },
    }
    }};
}
pub(in crate::cleanup_plan::replay) use extend;
macro_rules! append {
    ($paths:ident, $observations:ident, $suffix:ident, $continuation:expr, $work:ident, $error:ident, $exit:lifetime) => {{
        use $crate::cleanup_plan::replay::{ExprSkeletonPath, MAX_REPLAY_PATHS, lexical_skeleton};
        use $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::retained;
    retained::extend!($observations, $suffix, $work, $error, $exit);
    let owned_source = retained::receive!($error, $exit,
        $work.clone_owned(
            &$suffix.owned_source,
            "sequenced skeleton owned-source clone"
        )
    );
    if $paths.len() >= MAX_REPLAY_PATHS {
        $error = Some($work.budget.budget_refusal(
            $work.function,
            "complete HIR fork exceeds finite path profile",
            "",
        ));
        break $exit false;
    }
    retained::receive!($error, $exit,
        $work.push_expr_path(
            &mut $paths,
            ExprSkeletonPath {
                observations: std::mem::take(&mut $observations),
                owned_source,
                failed: $suffix.failed,
                residual: $suffix.residual,
                continuation: lexical_skeleton::Continuation::sequenced(
                    $continuation,
                    &$suffix.continuation
                ),
            },
            "sequenced skeleton path"
        )
    );

    }};
}
pub(in crate::cleanup_plan::replay) use append;
// Places stay in the caller's lexical invocation. No Output, Failure,
// emit_into, append_into, extend_into or singleton_into value gateways remain.
macro_rules! emit {
    ($prefixes:expr, $suffixes:expr, $work:ident $(,)?) => {{
        use $crate::cleanup_plan::replay::{AdmissionMode, AllocationPayer, MAX_REPLAY_PATHS, note_skeleton_materialization};
        use $crate::cleanup_plan::replay::skeleton_work::{Observations, ObservationDagKind};
        use $crate::cleanup_plan::replay::skeleton_work::enumerated_prefix as sequence;
        use $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::{retained, validation, Emission};
        use $crate::cleanup_plan::replay::lexical_skeleton;
        use $crate::bounded_output::classifier_string::ClassifierLedger;
        use $crate::diagnostic::Diagnostic;
        // Reverse declaration order preserves Output's original field Drop
        // order on every error and unwind: paths, observations, then error.
        let mut error: Option<Diagnostic> = None;
        let mut observations = Observations::default();
        let mut paths = $prefixes;
        let suffixes: &[$crate::cleanup_plan::replay::ExprSkeletonPath] = $suffixes;
        let complete = 'retained: {
    retained::receive!(error, 'retained,
        $work.budget.reserve_bytes(
            $work.function,
            ClassifierLedger::capture_into_controls()
                + ClassifierLedger::current_receiving_controls()
                + retained::PAYMENT_RECEIVERS,
            "complete HIR fork exact ledger receivers",
        )
    );
    let mut ledger = ClassifierLedger::empty_receiving();
    ledger.capture_current_into();
    if $work.budget.admission != AdmissionMode::Ordinary
        || !matches!(
            $work.budget.payer,
            AllocationPayer::LegacyUnobserved | AllocationPayer::ActiveBuilder
        )
        || $work.budget.lent_skeleton
        || $work.budget.merge_paths
        || $work.function.cleanup_plan.lexical_control.is_some()
    {
        break 'retained false;
    }
    retained::receive!(error, 'retained,
        $work.budget.reserve_bytes(
            $work.function,
            sequence::SHARE_CONTROLS,
            "complete HIR renewal selection receivers"
        )
    );
    if !retained::receive!(error, 'retained, sequence::without_renewal($work)) {
        break 'retained false;
    }
    retained::receive!(error, 'retained,
        $work.budget.reserve_bytes(
            $work.function,
            validation::CALL,
            "complete HIR validation scope receivers"
        )
    );
    if !retained::receive!(error, 'retained,
        validation::inputs(&paths, suffixes, $work)
    ) {
        break 'retained false;
    }
    retained::pay!(error, 'retained, $work,
        retained::SELECT,
        "retained HIR fork cardinality receivers"
    );
    // This proof uses the real suffix cardinality, including stopped prefixes.
    // No allocation/capacity prediction, filtered output count or refunded bank.
    if suffixes.len() == 1 && paths.len() <= MAX_REPLAY_PATHS {
        let suffix = &suffixes[0];
    for path in &mut paths {
        // Split field loans keep the owned Vec in this invocation and never
        // expose a reusable selector or arbitrary output sink to another payer.
        retained::receive!(error, 'retained,
            $work.budget
                .reserve_bytes($work.function, retained::PREFIX, "complete HIR fork prefix receivers")
        );
        if path.failed || path.residual {
            retained::receive!(error, 'retained,
                $work.charge(1, "short-circuited skeleton path")
            );
            note_skeleton_materialization();
            continue;
        }
        retained::receive!(error, 'retained, $work.charge(1, "skeleton prefix clone"));
        if !ledger.current() {
            error = Some(Diagnostic::io("SPX-H006", String::new()));
            break 'retained false;
        }
        retained::receive!(error, 'retained,
            $work.budget.reserve_bytes(
                $work.function,
                retained::OUTPUT + ClassifierLedger::current_receiving_controls(),
                "complete HIR fork output receivers"
            )
        );
        note_skeleton_materialization();
        std::mem::swap(&mut path.observations, &mut observations);
        retained::extend!(observations, suffix, $work, error, 'retained);
        let owned_source = retained::receive!(error, 'retained,
            $work.clone_owned(
                &suffix.owned_source,
                "sequenced skeleton owned-source clone"
            )
        );
        let continuation =
            lexical_skeleton::Continuation::sequenced(&path.continuation, &suffix.continuation);
        retained::receive!(error, 'retained, $work.charge(1, "sequenced skeleton path"));
        note_skeleton_materialization();
        std::mem::swap(&mut path.observations, &mut observations);
        path.owned_source = owned_source;
        path.failed = suffix.failed;
        path.residual = suffix.residual;
        path.continuation = continuation;
    }
        break 'retained true;
    }
    let prefixes = std::mem::take(&mut paths);
    for mut prefix in prefixes {
        retained::pay!(error, 'retained, $work, retained::PREFIX, "complete HIR fork prefix receivers");
        if prefix.failed || prefix.residual {
            if paths.len() >= MAX_REPLAY_PATHS {
                error = Some($work.budget.budget_refusal(
                    $work.function,
                    "complete HIR fork exceeds finite path profile",
                    "",
                ));
                break 'retained false;
            }
            retained::receive!(error, 'retained,
                $work.push_expr_path(&mut paths, prefix, "short-circuited skeleton path")
            );
            continue;
        }
        let Some((last, earlier)) = suffixes.split_last() else {
            continue;
        };
        retained::receive!(error, 'retained, $work.charge(1, "skeleton prefix clone"));
        if !earlier.is_empty() && !prefix.observations.items.is_empty() {
            retained::receive!(error, 'retained,
                $work.budget.reserve_bytes(
                    $work.function,
                    sequence::SHARE_CONTROLS + retained::FREEZE_RECEIVERS,
                    "complete HIR immutable fork publication"
                )
            );
            let Observations {
                prefix: previous,
                items,
            } = prefix.observations;
            prefix.observations = Observations {
                prefix: Some(retained::receive!(error, 'retained,
                    sequence::node($work, ObservationDagKind::Seq(previous, items))
                )),
                items: Vec::new(),
            };
        }
        let mut first = true;
        for suffix in earlier {
            if !ledger.current() {
                error = Some(Diagnostic::io("SPX-H006", String::new()));
                break 'retained false;
            }
            retained::pay!(error, 'retained, $work,
                retained::OUTPUT + ClassifierLedger::current_receiving_controls(),
                "complete HIR fork output receivers"
            );
            if !first {
                retained::receive!(error, 'retained, $work.charge(1, "skeleton prefix clone"));
            }
            first = false;
            retained::receive!(error, 'retained,
                $work.budget.reserve_clone(
                    $work.function,
                    &prefix.observations,
                    "skeleton prefix clone"
                )
            );
            note_skeleton_materialization();
            observations = prefix.observations.clone();
            retained::append!(paths, observations, suffix, &prefix.continuation, $work, error, 'retained);
        }
        if !ledger.current() {
            error = Some(Diagnostic::io("SPX-H006", String::new()));
            break 'retained false;
        }
        retained::pay!(error, 'retained, $work,
            retained::OUTPUT + ClassifierLedger::current_receiving_controls(),
            "complete HIR fork output receivers"
        );
        if !first {
            retained::receive!(error, 'retained, $work.charge(1, "skeleton prefix clone"));
        }
        note_skeleton_materialization();
        // The final unique suffix directly receives the remaining observation
        // owner. No Some/take/expect or speculative cloned tail is constructed.
        observations = prefix.observations;
        retained::append!(paths, observations, last, &prefix.continuation, $work, error, 'retained);
    }
    true
        };
        match error {
            Some(error) => Err(error),
            None if complete => Ok(Emission::Complete(paths)),
            None => Ok(Emission::Unsupported(paths)),
        }
    }};
}
pub(in crate::cleanup_plan::replay) use emit;
// Full physical/compiled parent acquisition precedes its entire unchanged old
// source inverse. No truncated-body authentication or new expected endpoint.
#[cfg(test)]
pub(in crate::cleanup_plan::replay) fn before_emission(current: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = |s: &str| {
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(s.as_bytes()))
        )
    };
    assert_eq!(
        std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission.rs")
        )
        .unwrap(),
        current.as_bytes()
    );
    assert_eq!(current.len(), 22365);
    assert_eq!(digest(current), "9b32edd9e75c3199c9b1b3c01a5bcdc9689872795e3222b6f684f4e967c08834");
    let mut immediate = current.to_owned();
    assert_eq!(immediate.get(21476..21528), Some("pub(in crate::cleanup_plan::replay) mod validation;\n")); immediate.replace_range(21476..21528, "mod validation;\n");
    assert_eq!(immediate.get(6988..7096), Some("            $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::retained::emit!(\n")); immediate.replace_range(6988..7096, "            $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::retained::emit_prepaid(\n");
    assert_eq!(immediate.get(6417..6529), Some("                $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::retained::emit!(\n")); immediate.replace_range(6417..6529, "                $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::retained::emit_prepaid(\n");
    let current = immediate.as_str();
    assert_eq!(current.len(), 22343);
    assert_eq!(
        digest(current),
        "5554635b3b09edc365624503c11d4063c28f6e748ce8e732871ae3f5547cbcd9"
    );
    let mut restored = current.to_owned();
    let edits: &[(usize, &str, &str)] = &[
        (5507, "#[cfg(test)]\n        {\n            if $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::immediate_selected() {\n                ", ""),
        (5690, "", "\n            "),
        (5706, "        ", ""),
        (5824, "        ", ""),
        (5868, ")?;", ","),
        (5880, "", ")?;\n"),
        (5993, "        ", ""),
        (6038, "        ", ""),
        (6047, "\n            } else {\n                $work.budget.reserve_bytes($work.function,\n                    $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::ENTRY\n                        + $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::retained::ACQUIRE,\n                    \"complete HIR fork caller receivers\")?;\n                $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::retained::emit_prepaid(\n                    $prefixes, $suffixes, $work,\n                )\n            }\n        }\n        #[cfg(not(test))]\n        {\n            $work.budget.reserve_bytes($work.function,\n                $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::ENTRY\n                    + $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::retained::ACQUIRE,\n                \"complete HIR fork caller receivers\")?;\n            $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::retained::emit_prepaid(\n                $prefixes, $suffixes, $work,\n            )\n        }", ""),
        (7337, "\n#[cfg(test)]", ""),
        (21892, "\n// The complete immediate emitter, STEP/ENTRY invoices and dispatch above stay\n// independently executable. Only the selected shipping producer owns this frame.\npub(in crate::cleanup_plan::replay) mod retained;\n\n#[cfg(test)]\npub(in crate::cleanup_plan::replay) fn immediate_selected() -> bool {\n    SELECTED.with(|selected| selected.get() == 3)\n}\n#[cfg(test)]\nfn with_immediate<T>(operation: impl FnOnce() -> T) -> T {\n    with_route(3, operation)\n}\n", ""),
    ];
    for &(at, after, before) in edits.iter().rev() {
        assert_eq!(restored.get(at..at + after.len()), Some(after));
        restored.replace_range(at..at + after.len(), before);
    }
    assert_eq!(restored.len(), 20575);
    assert_eq!(
        digest(&restored),
        "d7db6c7fc49140f9dabec3e647bf64b50571ee71a8478aae1c03acd92bba6c5f"
    );
    restored
}
