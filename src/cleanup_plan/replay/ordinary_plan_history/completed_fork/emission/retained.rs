//! Invocation-owned fork output and error receivers (#925).
//! The input allocation is retained only for a true singleton Cartesian suffix.
use super::*;
use crate::cleanup_plan::replay::ordinary_receiving::Failure;

struct Output<'a> {
    paths: Vec<ExprSkeletonPath>,
    observations: Observations,
    failure: Failure<'a>,
}
// ENTRY still pays the complete existing public argument/result boundary. This
// bank is additional: inline owners, initializers, exclusive producer loans,
// status return/receive, publication/reset and terminal Diagnostic ownership.
pub(in crate::cleanup_plan::replay) const ACQUIRE: usize = size_of::<Output<'static>>()
    + size_of::<Observations>()
    + Failure::acquisition_bytes()
    + 2 * size_of::<(
        &mut Output<'static>,
        &[ExprSkeletonPath],
        &mut SkeletonWork<'static, 'static>,
    )>()
    + 2 * size_of::<bool>()
    + 2 * size_of::<&ClassifierLedger>()
    + 2 * size_of::<&mut Observations>()
    + 4 * size_of::<Vec<ExprSkeletonPath>>() // paths take: default/replacement and returned/received owner
    + 2 * size_of::<Option<Diagnostic>>()
    + PAYMENT;
// The prefix no longer constructs an output, enumerated suffix iterator,
// Option<Observations>, clone result or owned-source result. Freeze backing and
// its complete original call bank remain separately charged before formation.
const PREFIX: usize = 2 * size_of::<ExprSkeletonPath>()
    + 2 * size_of::<&ExprSkeletonPath>()
    + 2 * size_of::<&mut Output<'static>>()
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
const OUTPUT: usize = STEP
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
    + 2 * size_of::<&mut Output<'static>>()
    + 2 * size_of::<&mut Failure<'static>>()
    + 2 * size_of::<Observations>()
    + 2 * size_of::<(
        &mut Output<'static>,
        &ExprSkeletonPath,
        &lexical_skeleton::Continuation,
        &mut SkeletonWork<'static, 'static>,
    )>()
    + 2 * size_of::<(
        &mut Observations,
        &Observations,
        &mut SkeletonWork<'static, 'static>,
        &mut Failure<'static>,
    )>()
    + 4 * size_of::<bool>();
const SELECT: usize = 2 * size_of::<&[ExprSkeletonPath]>()
    + 2 * size_of::<&Vec<ExprSkeletonPath>>()
    + 2 * size_of::<Option<(&ExprSkeletonPath, &[ExprSkeletonPath])>>()
    + 2 * size_of::<(&ExprSkeletonPath, &[ExprSkeletonPath])>()
    + 2 * size_of::<(
        &mut Output<'static>,
        &ExprSkeletonPath,
        &mut SkeletonWork<'static, 'static>,
        &ClassifierLedger,
    )>()
    + 2 * size_of::<&mut Vec<ExprSkeletonPath>>()
    + 2 * size_of::<bool>()
    + 2 * size_of::<std::slice::IterMut<'static, ExprSkeletonPath>>()
    + 2 * size_of::<Option<&mut ExprSkeletonPath>>()
    + 4 * size_of::<usize>()
    + 4 * size_of::<bool>()
    + PAYMENT;
// The extension changes only the output ABI. No credit is taken from the old
// sequence EXTEND_CONTROLS: every node/backing/clone bank remains complete.
const EXTEND: usize = sequence::EXTEND_CONTROLS
    + 2 * size_of::<(
        &mut Observations,
        &Observations,
        &mut SkeletonWork<'static, 'static>,
        &mut Failure<'static>,
    )>()
    + 4 * size_of::<&mut Option<Rc<ObservationDag>>>()
    + 4 * size_of::<&mut Vec<Rc<SkeletonObservation>>>()
    + 4 * size_of::<Vec<Rc<SkeletonObservation>>>() // take: default, moved replacement and taken return/receive
    + 2 * size_of::<Observations>()
    + 2 * size_of::<bool>();

// A retained first error is never replaced. Existing returning callees still
// receive their complete original Result tariff; this is not an adapter credit.
macro_rules! receive {
    ($failure:expr, $operation:expr) => {{
        match $operation {
            Ok(value) => value,
            Err(error) => {
                $failure.error = Some(error);
                return false;
            }
        }
    }};
}
macro_rules! pay {
    ($output:ident, $work:ident, $bytes:expr, $phase:literal) => {{
        receive!(
            $output.failure,
            $work.budget.reserve_bytes($work.function, $bytes, $phase)
        );
    }};
}

pub(in crate::cleanup_plan::replay) fn emit_prepaid(
    prefixes: Vec<ExprSkeletonPath>,
    suffixes: &[ExprSkeletonPath],
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Emission, Diagnostic> {
    let mut output = Output {
        paths: prefixes,
        observations: Observations::default(),
        failure: Failure {
            error: None,
            function: work.function,
            budget: &*work.budget,
        },
    };
    let complete = emit_into(&mut output, suffixes, work);
    match output.failure.error {
        Some(error) => Err(error),
        None if complete => Ok(Emission::Complete(output.paths)),
        None => Ok(Emission::Unsupported(output.paths)),
    }
}

fn emit_into(
    output: &mut Output<'_>,
    suffixes: &[ExprSkeletonPath],
    work: &mut SkeletonWork<'_, '_>,
) -> bool {
    receive!(
        output.failure,
        work.budget.reserve_bytes(
            work.function,
            ClassifierLedger::capture_into_controls()
                + ClassifierLedger::current_receiving_controls()
                + PAYMENT,
            "complete HIR fork exact ledger receivers",
        )
    );
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
        return false;
    }
    receive!(
        output.failure,
        work.budget.reserve_bytes(
            work.function,
            sequence::SHARE_CONTROLS,
            "complete HIR renewal selection receivers"
        )
    );
    if !receive!(output.failure, sequence::without_renewal(work)) {
        return false;
    }
    receive!(
        output.failure,
        work.budget.reserve_bytes(
            work.function,
            validation::CALL,
            "complete HIR validation scope receivers"
        )
    );
    if !receive!(
        output.failure,
        validation::inputs(&output.paths, suffixes, work)
    ) {
        return false;
    }
    pay!(
        output,
        work,
        SELECT,
        "retained HIR fork cardinality receivers"
    );
    // This proof uses the real suffix cardinality, including stopped prefixes.
    // No allocation/capacity prediction, filtered output count or refunded bank.
    if suffixes.len() == 1 && output.paths.len() <= MAX_REPLAY_PATHS {
        return singleton_into(output, &suffixes[0], work, &ledger);
    }
    let prefixes = std::mem::take(&mut output.paths);
    for mut prefix in prefixes {
        pay!(output, work, PREFIX, "complete HIR fork prefix receivers");
        if prefix.failed || prefix.residual {
            if output.paths.len() >= MAX_REPLAY_PATHS {
                output.failure.error = Some(work.budget.budget_refusal(
                    work.function,
                    "complete HIR fork exceeds finite path profile",
                    "",
                ));
                return false;
            }
            receive!(
                output.failure,
                work.push_expr_path(&mut output.paths, prefix, "short-circuited skeleton path")
            );
            continue;
        }
        let Some((last, earlier)) = suffixes.split_last() else {
            continue;
        };
        receive!(output.failure, work.charge(1, "skeleton prefix clone"));
        if !earlier.is_empty() && !prefix.observations.items.is_empty() {
            receive!(
                output.failure,
                work.budget.reserve_bytes(
                    work.function,
                    sequence::SHARE_CONTROLS + FREEZE,
                    "complete HIR immutable fork publication"
                )
            );
            let Observations {
                prefix: previous,
                items,
            } = prefix.observations;
            prefix.observations = Observations {
                prefix: Some(receive!(
                    output.failure,
                    sequence::node(work, ObservationDagKind::Seq(previous, items))
                )),
                items: Vec::new(),
            };
        }
        let mut first = true;
        for suffix in earlier {
            if !ledger.current() {
                output.failure.error = Some(Diagnostic::io("SPX-H006", String::new()));
                return false;
            }
            pay!(
                output,
                work,
                OUTPUT + ClassifierLedger::current_receiving_controls(),
                "complete HIR fork output receivers"
            );
            if !first {
                receive!(output.failure, work.charge(1, "skeleton prefix clone"));
            }
            first = false;
            receive!(
                output.failure,
                work.budget.reserve_clone(
                    work.function,
                    &prefix.observations,
                    "skeleton prefix clone"
                )
            );
            note_skeleton_materialization();
            output.observations = prefix.observations.clone();
            if !append_into(output, suffix, &prefix.continuation, work) {
                return false;
            }
        }
        if !ledger.current() {
            output.failure.error = Some(Diagnostic::io("SPX-H006", String::new()));
            return false;
        }
        pay!(
            output,
            work,
            OUTPUT + ClassifierLedger::current_receiving_controls(),
            "complete HIR fork output receivers"
        );
        if !first {
            receive!(output.failure, work.charge(1, "skeleton prefix clone"));
        }
        note_skeleton_materialization();
        // The final unique suffix directly receives the remaining observation
        // owner. No Some/take/expect or speculative cloned tail is constructed.
        output.observations = prefix.observations;
        if !append_into(output, last, &prefix.continuation, work) {
            return false;
        }
    }
    true
}

fn extend_into(
    target: &mut Observations,
    suffix: &Observations,
    work: &mut SkeletonWork<'_, '_>,
    failure: &mut Failure<'_>,
) -> bool {
    if suffix.prefix.is_none() {
        receive!(failure, work.charge(1, "skeleton suffix clone"));
        note_skeleton_materialization();
        receive!(
            failure,
            work.budget.reserve_vec(
                work.function,
                &mut target.items,
                suffix.items.len(),
                "ordinary flat suffix"
            )
        );
        target.items.extend_from_slice(&suffix.items);
        return true;
    }
    receive!(failure, work.charge(1, "skeleton suffix clone"));
    note_skeleton_materialization();
    receive!(
        failure,
        work.budget.reserve_bytes(
            work.function,
            EXTEND,
            "complete HIR shared suffix call receivers"
        )
    );
    let prefix = suffix
        .prefix
        .as_ref()
        .expect("shared suffix observed by caller");
    let head = if target.items.is_empty() {
        target.prefix.take()
    } else {
        Some(receive!(
            failure,
            sequence::node(
                work,
                ObservationDagKind::Seq(target.prefix.take(), std::mem::take(&mut target.items))
            )
        ))
    };
    let joined = match head {
        None => prefix.clone(),
        Some(head) => {
            receive!(
                failure,
                work.budget.reserve_array::<Rc<ObservationDag>>(
                    work.function,
                    2,
                    "enumerated HIR concatenation backing"
                )
            );
            receive!(
                failure,
                sequence::node(work, ObservationDagKind::Cat(vec![head, prefix.clone()]))
            )
        }
    };
    receive!(
        failure,
        work.budget
            .reserve_clone(work.function, &suffix.items, "ordinary DAG suffix tail")
    );
    target.prefix = Some(joined);
    target.items = suffix.items.clone();
    true
}

// Called only after OUTPUT is acquired. Each output owns its independent source
// and continuation; error exit retains all partial owners for ordinary Drop.
fn append_into(
    output: &mut Output<'_>,
    suffix: &ExprSkeletonPath,
    continuation: &lexical_skeleton::Continuation,
    work: &mut SkeletonWork<'_, '_>,
) -> bool {
    if !extend_into(
        &mut output.observations,
        &suffix.observations,
        work,
        &mut output.failure,
    ) {
        return false;
    }
    let owned_source = receive!(
        output.failure,
        work.clone_owned(
            &suffix.owned_source,
            "sequenced skeleton owned-source clone"
        )
    );
    if output.paths.len() >= MAX_REPLAY_PATHS {
        output.failure.error = Some(work.budget.budget_refusal(
            work.function,
            "complete HIR fork exceeds finite path profile",
            "",
        ));
        return false;
    }
    receive!(
        output.failure,
        work.push_expr_path(
            &mut output.paths,
            ExprSkeletonPath {
                observations: std::mem::take(&mut output.observations),
                owned_source,
                failed: suffix.failed,
                residual: suffix.residual,
                continuation: lexical_skeleton::Continuation::sequenced(
                    continuation,
                    &suffix.continuation
                ),
            },
            "sequenced skeleton path"
        )
    );
    true
}

fn singleton_into(
    output: &mut Output<'_>,
    suffix: &ExprSkeletonPath,
    work: &mut SkeletonWork<'_, '_>,
    ledger: &ClassifierLedger,
) -> bool {
    for path in &mut output.paths {
        // Split field loans keep the owned Vec in this invocation and never
        // expose a reusable selector or arbitrary output sink to another payer.
        receive!(
            output.failure,
            work.budget
                .reserve_bytes(work.function, PREFIX, "complete HIR fork prefix receivers")
        );
        if path.failed || path.residual {
            receive!(
                output.failure,
                work.charge(1, "short-circuited skeleton path")
            );
            note_skeleton_materialization();
            continue;
        }
        receive!(output.failure, work.charge(1, "skeleton prefix clone"));
        if !ledger.current() {
            output.failure.error = Some(Diagnostic::io("SPX-H006", String::new()));
            return false;
        }
        receive!(
            output.failure,
            work.budget.reserve_bytes(
                work.function,
                OUTPUT + ClassifierLedger::current_receiving_controls(),
                "complete HIR fork output receivers"
            )
        );
        note_skeleton_materialization();
        std::mem::swap(&mut path.observations, &mut output.observations);
        if !extend_into(
            &mut output.observations,
            &suffix.observations,
            work,
            &mut output.failure,
        ) {
            return false;
        }
        let owned_source = receive!(
            output.failure,
            work.clone_owned(
                &suffix.owned_source,
                "sequenced skeleton owned-source clone"
            )
        );
        let continuation =
            lexical_skeleton::Continuation::sequenced(&path.continuation, &suffix.continuation);
        receive!(output.failure, work.charge(1, "sequenced skeleton path"));
        note_skeleton_materialization();
        std::mem::swap(&mut path.observations, &mut output.observations);
        path.owned_source = owned_source;
        path.failed = suffix.failed;
        path.residual = suffix.residual;
        path.continuation = continuation;
    }
    true
}

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
