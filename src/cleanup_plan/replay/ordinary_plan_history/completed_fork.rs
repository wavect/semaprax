//! Closed flat-input fork composition; no shipping selector or semantic cache.
//! Each actual prefix tail moves once into an append-only immutable arena.
//! Separate producers never share indices as comparison authority. Callers must
//! retain the original producer on unsupported DAG/lexical/renewal inputs.
use super::super::skeleton_work::SkeletonWork;
use super::*;
use crate::bounded_output::classifier_string::ClassifierLedger;

struct Chunk {
    previous: Option<usize>,
    items: Vec<Rc<SkeletonObservation>>,
    length: usize,
}
struct Path {
    previous: Option<usize>,
    tail: ExprSkeletonPath,
}
pub(super) struct Histories {
    chunks: Vec<Chunk>,
    paths: Vec<Path>,
}
pub(super) enum Outcome {
    Unsupported(Vec<ExprSkeletonPath>),
    Complete(Histories),
}
// The entry frame covers distinct actual/formal owners and returned receivers,
// iterators, selectors, arithmetic Options, failure and complete Drop paths.
// Nested backing is charged separately before every allocation/replacement.
pub(super) const ENTRY: usize = 2 * size_of::<Vec<ExprSkeletonPath>>()
    + 2 * size_of::<&[&[ExprSkeletonPath]]>()
    + 2 * size_of::<&mut SkeletonWork<'static, 'static>>()
    + size_of::<Result<Outcome, Diagnostic>>()
    + size_of::<Outcome>()
    + 2 * size_of::<Histories>()
    + 2 * size_of::<ClassifierLedger>()
    + 2 * size_of::<Vec<Chunk>>()
    + 2 * size_of::<Vec<Path>>()
    + size_of::<std::vec::IntoIter<ExprSkeletonPath>>()
    + size_of::<std::slice::Iter<'static, &'static [ExprSkeletonPath]>>()
    + 2 * size_of::<std::slice::Iter<'static, ExprSkeletonPath>>()
    + 2 * size_of::<std::vec::IntoIter<Path>>()
    + 2 * size_of::<Option<Path>>()
    + size_of::<Option<ExprSkeletonPath>>()
    + 2 * size_of::<Option<&'static ExprSkeletonPath>>()
    + size_of::<Option<&'static &'static [ExprSkeletonPath]>>()
    + 8 * size_of::<bool>()
    + 6 * size_of::<usize>()
    + 2 * size_of::<std::slice::Iter<'static, CleanupBlock>>()
    + 2 * size_of::<std::slice::Iter<'static, CleanupTransition>>()
    + 4 * size_of::<Option<&ExprSkeletonPath>>()
    + 2 * size_of::<Option<&CleanupBlock>>()
    + 2 * size_of::<Option<&CleanupTransition>>();
pub(super) const PAYMENT: usize = 2 * size_of::<&mut ReplayBudget>()
    + 2 * size_of::<&ResolvedFunction>()
    + 2 * size_of::<&str>()
    + 2 * size_of::<usize>()
    + 2 * size_of::<Result<(), Diagnostic>>();
// Renewed before constructing each owned publication, including the partial
// owner which is dropped if any original clone/work/backing operation fails.
const PATH: usize = 2 * size_of::<Path>()
    + 2 * size_of::<ExprSkeletonPath>()
    + 2 * size_of::<Observations>()
    + 2 * size_of::<Option<usize>>()
    + 2 * size_of::<Option<CleanupPlace>>()
    + 2 * size_of::<lexical_skeleton::Continuation>()
    + 4 * size_of::<bool>()
    + 4 * size_of::<&ExprSkeletonPath>()
    + 4 * size_of::<&Observations>()
    + 2 * size_of::<&Option<CleanupPlace>>()
    + 2 * size_of::<&mut Vec<Path>>()
    + 2 * size_of::<Result<Observations, Diagnostic>>()
    + 2 * size_of::<Result<Option<CleanupPlace>, Diagnostic>>()
    + 4 * size_of::<&mut SkeletonWork<'static, 'static>>()
    + 4 * size_of::<&str>()
    + 2 * PAYMENT;
const CHUNK: usize = 2 * size_of::<Chunk>()
    + 2 * size_of::<Vec<Rc<SkeletonObservation>>>()
    + 3 * size_of::<Option<usize>>()
    + 4 * size_of::<usize>()
    + 2 * size_of::<&mut Vec<Chunk>>()
    + 2 * PAYMENT;

// The caller acquires new actual/formal/Result owners before their formation.
// A private macro keeps input evaluation and owned moves after that payment.
#[allow(
    unused_macros,
    reason = "closed producer awaits shipping expression plumbing"
)]
macro_rules! produce {
    ($prefixes:expr, $forks:expr, $work:ident) => {{
        $work.budget.reserve_bytes(
            $work.function,
            $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::ENTRY
                + $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::PAYMENT,
            "completed history entry",
        )?;
        $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::produce_prepaid(
            $prefixes, $forks, $work,
        )
    }};
}
#[allow(
    unused_imports,
    reason = "closed producer awaits shipping expression plumbing"
)]
pub(super) use produce;

pub(super) fn produce_prepaid(
    prefixes: Vec<ExprSkeletonPath>,
    forks: &[&[ExprSkeletonPath]],
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Outcome, Diagnostic> {
    work.budget.reserve_bytes(
        work.function,
        ClassifierLedger::capture_into_controls()
            + ClassifierLedger::current_receiving_controls()
            + PAYMENT,
        "completed history exact ledger receivers",
    )?;
    let mut ledger = ClassifierLedger::empty_receiving();
    ledger.capture_current_into();
    // No name, size threshold, node marker or observation identity grants scope.
    // Every input sequence must be flat; no Alt or existing DAG is interpreted.
    if work.budget.selected()
        || work.budget.merge_paths
        || work.budget.lent_skeleton
        || work.function.cleanup_plan.lexical_control.is_some()
    {
        return Ok(Outcome::Unsupported(prefixes));
    }
    #[cfg(test)]
    if matches!(work.budget.payer, AllocationPayer::Oracle(_)) {
        return Ok(Outcome::Unsupported(prefixes));
    }
    for block in &work.function.cleanup_plan.blocks {
        work.budget
            .charge(work.function, 1, "completed history renewal exclusion")?;
        for transition in &block.transitions {
            work.budget
                .charge(work.function, 1, "completed history renewal exclusion")?;
            if matches!(
                transition,
                CleanupTransition::ReserveRenewal { .. } | CleanupTransition::Renew { .. }
            ) {
                return Ok(Outcome::Unsupported(prefixes));
            }
        }
    }
    for path in &prefixes {
        work.budget
            .charge(work.function, 1, "completed history input selection")?;
        if path.observations.prefix.is_some()
            || !matches!(path.continuation, lexical_skeleton::Continuation::Normal)
        {
            return Ok(Outcome::Unsupported(prefixes));
        }
    }
    for fork in forks {
        for path in *fork {
            work.budget
                .charge(work.function, 1, "completed history input selection")?;
            if path.observations.prefix.is_some()
                || !matches!(path.continuation, lexical_skeleton::Continuation::Normal)
            {
                return Ok(Outcome::Unsupported(prefixes));
            }
        }
    }
    let mut result = Histories {
        chunks: Vec::new(),
        paths: Vec::new(),
    };
    for prefix in prefixes {
        work.budget
            .reserve_bytes(work.function, PATH, "completed history initial path")?;
        work.budget.reserve_vec(
            work.function,
            &mut result.paths,
            1,
            "completed history initial backing",
        )?;
        result.paths.push(Path {
            previous: None,
            tail: prefix,
        });
    }
    for fork in forks {
        if !ledger.current() {
            return Err(Diagnostic::io("SPX-H006", String::new()));
        }
        work.budget.reserve_bytes(
            work.function,
            PAYMENT + ClassifierLedger::current_receiving_controls(),
            "completed history renewed ledger receivers",
        )?;
        let mut next = Vec::new();
        for mut prefix in result.paths {
            work.budget
                .reserve_bytes(work.function, PATH, "completed history path state")?;
            if prefix.tail.failed || prefix.tail.residual {
                work.charge(1, "short-circuited skeleton path")?;
                note_skeleton_materialization();
                work.budget.reserve_vec(
                    work.function,
                    &mut next,
                    1,
                    "completed history stopped backing",
                )?;
                if next.len() >= MAX_REPLAY_PATHS {
                    return Err(work.budget.budget_refusal(
                        work.function,
                        "completed history exceeds the finite path profile",
                        "",
                    ));
                }
                next.push(prefix);
                continue;
            }
            // Empty suffixes destroy active prefixes, exactly like the old
            // Cartesian product. They never create an empty success path.
            if fork.is_empty() {
                continue;
            }
            // Preserve the original first prefix-clone work frontier BEFORE
            // acquiring or publishing this new immutable chunk.
            work.charge(1, "skeleton prefix clone")?;
            note_skeleton_materialization();
            work.budget
                .reserve_bytes(work.function, CHUNK, "completed history immutable chunk")?;
            let previous_length = match prefix.previous {
                Some(index) => result.chunks[index].length,
                None => 0,
            };
            let length = match previous_length.checked_add(prefix.tail.observations.items.len()) {
                Some(length) => length,
                None => {
                    return Err(work.budget.budget_refusal(
                        work.function,
                        "completed history length overflowed",
                        "",
                    ));
                }
            };
            work.budget.reserve_vec(
                work.function,
                &mut result.chunks,
                1,
                "completed history chunk backing",
            )?;
            let index = result.chunks.len();
            result.chunks.push(Chunk {
                previous: prefix.previous,
                items: std::mem::take(&mut prefix.tail.observations.items),
                length,
            });
            let mut first = true;
            for suffix in *fork {
                work.budget.reserve_bytes(
                    work.function,
                    PATH,
                    "completed history fork publication",
                )?;
                if first {
                    first = false;
                } else {
                    work.charge(1, "skeleton prefix clone")?;
                    note_skeleton_materialization();
                }
                let observations =
                    work.clone_owned(&suffix.observations, "skeleton suffix clone")?;
                let owned_source = work.clone_owned(
                    &suffix.owned_source,
                    "sequenced skeleton owned-source clone",
                )?;
                work.charge(1, "sequenced skeleton path")?;
                note_skeleton_materialization();
                work.budget.reserve_vec(
                    work.function,
                    &mut next,
                    1,
                    "completed history result backing",
                )?;
                if next.len() >= MAX_REPLAY_PATHS {
                    return Err(work.budget.budget_refusal(
                        work.function,
                        "completed history exceeds the finite path profile",
                        "",
                    ));
                }
                next.push(Path {
                    previous: Some(index),
                    tail: ExprSkeletonPath {
                        observations,
                        owned_source,
                        failed: suffix.failed,
                        residual: suffix.residual,
                        continuation: lexical_skeleton::Continuation::sequenced(
                            &prefix.tail.continuation,
                            &suffix.continuation,
                        ),
                    },
                });
            }
        }
        result.paths = next;
    }
    Ok(Outcome::Complete(result))
}

#[cfg(test)]
#[rustfmt::skip]
mod before;
mod content;
#[cfg(test)]
mod tests;
