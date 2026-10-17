//! Shipping whole-owner controls against the complete immediate predecessor.
use super::super::*;
use crate::bounded_output::{
    active_remaining, set_active_floor, with_limit_usage_refusal, with_limit_usage_refusal_site,
};
use crate::test_allocation_observer::measure;
#[path = "before_funded.rs"]
mod before;
#[path = "observation_parity.rs"]
mod observation_parity;
const OLD_HIR_SELECTION_CONTROLS: usize = std::mem::size_of::<Result<(), Diagnostic>>()
    + std::mem::size_of::<&mut ReplayBudget>() + std::mem::size_of::<&ResolvedFunction>() + std::mem::size_of::<&str>()
    + 2 * std::mem::size_of::<usize>() // price and length
    + 2 * std::mem::size_of::<bool>() // predicate initializer/receiving slot
    + std::mem::size_of::<&std::borrow::Cow<'_, [ExprSkeletonPath]>>()
    + std::mem::size_of::<&[ExprSkeletonPath]>();

#[derive(Debug, Eq, PartialEq)]
struct Failure {
    code: &'static str,
    severity: crate::diagnostic::Severity,
    message: ([u8; 1024], usize),
    path: Option<([u8; 1024], usize)>,
    span: Option<crate::ast::Span>,
    help: Option<([u8; 1024], usize)>,
}
fn text(value: &str) -> ([u8; 1024], usize) {
    let mut bytes = [0; 1024];
    assert!(value.len() <= bytes.len());
    bytes[..value.len()].copy_from_slice(value.as_bytes());
    (bytes, value.len())
}
fn failure(value: Diagnostic) -> Failure {
    let result = Failure {
        code: value.code,
        severity: value.severity,
        message: text(&value.message),
        path: value.path.as_deref().map(text),
        span: value.span,
        help: value.help.as_deref().map(text),
    };
    drop(value);
    result
}

fn seed(
    count: usize,
    stopped: usize,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    let mut paths = Vec::new();
    for index in 0..count {
        let mut path = empty_expr_path();
        path.failed = stopped == 1 && index % 3 == 0;
        path.residual = stopped == 2 || (stopped == 1 && index % 3 == 1);
        path.owned_source = Some(CleanupPlace::whole(StorageId::ProvisionalResult));
        for bit in 0..10 {
            let expression = work.clone_owned(&work.function.body.id, "vector fixture identity")?;
            work.push_observation(
                &mut path,
                SkeletonObservation::Status {
                    source: StatusSourceId {
                        expression,
                        lane: StatusLane::OperationFailure,
                    },
                    success: bit % 2 == 0,
                },
                "vector fixture observation",
            )?;
        }
        path.continuation =
            lexical_skeleton::Continuation::Advancing(path.observations.items[0].clone());
        work.push_expr_path(&mut paths, path, "vector fixture path")?;
    }
    Ok(paths)
}

fn assert_expr_paths_equal(actual: &[ExprSkeletonPath], expected: &[ExprSkeletonPath]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        let ExprSkeletonPath {
            observations,
            owned_source,
            failed,
            residual,
            continuation,
        } = actual;
        let ExprSkeletonPath {
            observations: old_observations,
            owned_source: old_owned_source,
            failed: old_failed,
            residual: old_residual,
            continuation: old_continuation,
        } = expected;
        if observations.prefix.is_none() && old_observations.prefix.is_none() {
            assert_eq!(observations, old_observations);
        } else {
            // Both complete drivers independently synthesize a new Cat root;
            // root address equality is not a cross-driver semantic oracle.
            observation_parity::assert_equal(observations, old_observations);
        }
        assert_eq!(owned_source, old_owned_source);
        assert_eq!(failed, old_failed);
        assert_eq!(residual, old_residual);
        assert_eq!(continuation, old_continuation);
    }
}

// Real same-ledger ordinary DAG producer, restored to the original caller
// mode before sequencing. Each resulting path retains a nonempty flat tail.
fn dag_seed(
    count: usize,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    let mut members = seed(2, 0, work)?;
    for member in &mut members {
        member.owned_source = None;
        member.continuation = lexical_skeleton::Continuation::Normal;
    }
    let old_merge = work.budget.merge_paths;
    let old_region = work.active_copy_region.take();
    work.active_copy_region =
        Some(work.clone_owned(&work.function.body.id, "DAG fixture region identity")?);
    work.budget.merge_paths = true;
    let merged = work.merged_paths(members);
    work.budget.merge_paths = old_merge;
    work.active_copy_region = old_region;
    let mut merged = merged?;
    assert_eq!(merged.len(), 1);
    let dag = merged.pop().unwrap().observations;
    assert!(dag.prefix.is_some());
    let mut inputs = seed(count, 0, work)?;
    for input in &mut inputs {
        let copied = work.clone_observations(&dag, "DAG fixture prefix")?;
        input.observations.prefix = copied.prefix;
        assert!(!input.observations.items.is_empty());
    }
    Ok(inputs)
}

#[test]
fn lexical_vector_transfer_keeps_complete_fields_work_and_reports_all_shapes() {
    let program = path_array_main8a::fixture_program();
    let function = &program.functions[0];
    let mut transfers = [false; 6 * 3 * 4];
    let mut transfer_index = 0;
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            for count in [0, 1, 8, 17, 64, 128] {
                for fanout in [0, 1, 2] {
                    for stopped in [0, 1, 2, 3] {
                        let mut old = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                        let mut current = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                        let mut old_work = SkeletonWork::new(&program, function, &mut old);
                        let mut work = SkeletonWork::new(&program, function, &mut current);
                        let expected_input = seed(count, stopped, &mut old_work).unwrap();
                        let input = seed(count, stopped, &mut work).unwrap();
                        let old_suffixes = seed(fanout, stopped, &mut old_work).unwrap();
                        let suffixes = seed(fanout, stopped, &mut work).unwrap();
                        let pointer = input.as_ptr();
                        let before = skeleton_materializations();
                        let expected = before::sequence_skeleton_paths(
                            expected_input,
                            &old_suffixes,
                            &mut old_work,
                        )
                        .unwrap();
                        let old_materializations = skeleton_materializations() - before;
                        let before = skeleton_materializations();
                        let actual = sequence_skeleton_paths(input, &suffixes, &mut work).unwrap();
                        assert_eq!(skeleton_materializations() - before, old_materializations);
                        assert_expr_paths_equal(&actual, &expected);
                        assert_eq!(actual.capacity(), expected.capacity());
                        assert_eq!(work.budget.skeleton_spent, old_work.budget.skeleton_spent);
                        assert_eq!(
                            work.budget.skeleton_remaining,
                            old_work.budget.skeleton_remaining
                        );
                        transfers[transfer_index] = actual.as_ptr() == pointer;
                        transfer_index += 1;
                        drop((actual, expected, suffixes, old_suffixes));
                        drop((work, old_work));
                        drop((current, old));
                    }
                }
            }
        })
    });
    assert_eq!(transfer_index, transfers.len());
    let mut transfer_index = 0;
    for count in [0, 1, 8, 17, 64, 128] {
        for fanout in [0, 1, 2] {
            for stopped in [0, 1, 2, 3] {
                eprintln!(
                    "vector count={count} fanout={fanout} stopped={stopped} moved={}",
                    transfers[transfer_index]
                );
                transfer_index += 1;
            }
        }
    }
    eprintln!("whole vector parity reserved={paid} heap={allocation:?}");
    assert!(!refused && first.is_none());
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);

    // Genuine DAG suffix and target prefixes, both with live tails. This is
    // an admitted general transfer witness, not parity through the fallback.
    let (((), dag_allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut old = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
            let mut current = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
            let mut old_work = SkeletonWork::new(&program, function, &mut old);
            let mut work = SkeletonWork::new(&program, function, &mut current);
            let old_inputs = dag_seed(128, &mut old_work).unwrap();
            let inputs = dag_seed(128, &mut work).unwrap();
            let old_suffixes = dag_seed(1, &mut old_work).unwrap();
            let suffixes = dag_seed(1, &mut work).unwrap();
            assert!(
                inputs[0].observations.prefix.is_some()
                    && suffixes[0].observations.prefix.is_some()
            );
            assert!(
                !inputs[0].observations.items.is_empty()
                    && !suffixes[0].observations.items.is_empty()
            );
            let pointer = inputs.as_ptr();
            let capacity = inputs.capacity();
            let before = skeleton_materializations();
            let expected =
                before::sequence_skeleton_paths(old_inputs, &old_suffixes, &mut old_work).unwrap();
            let old_materializations = skeleton_materializations() - before;
            let before = skeleton_materializations();
            let actual = sequence_skeleton_paths(inputs, &suffixes, &mut work).unwrap();
            assert_eq!(
                actual.as_ptr(),
                pointer,
                "general DAG route retains the actual input vector"
            );
            assert_eq!(actual.capacity(), capacity);
            assert_eq!(skeleton_materializations() - before, old_materializations);
            assert_eq!(actual.len(), expected.len());
            assert_eq!(work.budget.skeleton_spent, old_work.budget.skeleton_spent);
            assert_eq!(
                work.budget.skeleton_remaining,
                old_work.budget.skeleton_remaining
            );
            drop((actual, expected, suffixes, old_suffixes));
            drop((work, old_work));
            drop((current, old));
        })
    });
    assert!(!refused && first.is_none());
    assert!(!dag_allocation.invalid && dag_allocation.live == 0);
    assert_eq!(dag_allocation.allocated, dag_allocation.freed);
    eprintln!("general DAG vector reserved={paid} heap={dag_allocation:?}");

    // Full DAG values are compared outside the allocation observer: both
    // independent drivers allocate their own new concatenation roots. Keep the
    // measured construction-through-Drop/pointer/work controls above intact.
    observation_parity::verify_contract(&function.body.id, &program.functions[1].body.id);
    let ((), refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
        let mut old = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
        let mut current = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
        let mut old_work = SkeletonWork::new(&program, function, &mut old);
        let mut work = SkeletonWork::new(&program, function, &mut current);
        let old_inputs = dag_seed(128, &mut old_work).unwrap();
        let inputs = dag_seed(128, &mut work).unwrap();
        let old_suffixes = dag_seed(1, &mut old_work).unwrap();
        let suffixes = dag_seed(1, &mut work).unwrap();
        assert!(
            inputs[0].observations.prefix.is_some() && suffixes[0].observations.prefix.is_some()
        );
        assert!(
            !inputs[0].observations.items.is_empty() && !suffixes[0].observations.items.is_empty()
        );
        let pointer = inputs.as_ptr();
        let capacity = inputs.capacity();
        let before = skeleton_materializations();
        let expected =
            before::sequence_skeleton_paths(old_inputs, &old_suffixes, &mut old_work).unwrap();
        let old_materializations = skeleton_materializations() - before;
        let before = skeleton_materializations();
        let actual = sequence_skeleton_paths(inputs, &suffixes, &mut work).unwrap();
        assert_eq!(
            actual.as_ptr(),
            pointer,
            "general DAG route retains the actual input vector"
        );
        assert_eq!(actual.capacity(), capacity);
        assert_eq!(skeleton_materializations() - before, old_materializations);
        assert_expr_paths_equal(&actual, &expected);
        assert_eq!(work.budget.skeleton_spent, old_work.budget.skeleton_spent);
        assert_eq!(
            work.budget.skeleton_remaining,
            old_work.budget.skeleton_remaining
        );
        drop((actual, expected, suffixes, old_suffixes));
        drop((work, old_work));
        drop((current, old));
    });
    assert!(!refused && first.is_none());

    // Separate actual whole owners include their input construction and budget
    // Drop. Small, stopped and wide cases are reported even when unfavorable.
    for count in [1, 8, 64, 128] {
        for stopped in [0, 1, 2] {
            for old in [true, false] {
                let mut caller = ReplayBudget::unobserved();
                let mut work = SkeletonWork::new(&program, function, &mut caller);
                let ((outcome, allocation), refused, paid, first) =
                    with_limit_usage_refusal(usize::MAX, || {
                        measure(|| {
                            *work.budget = ReplayBudget::new();
                            let outcome = (|| {
                                work.budget.prepare_builder(function)?;
                                work.budget
                                    .reserve_skeleton(function, MAX_REPLAY_WORK_UNITS)?;
                                let input = seed(count, stopped, &mut work)?;
                                let suffixes = seed(1, 0, &mut work)?;
                                let result = if old {
                                    before::sequence_skeleton_paths(input, &suffixes, &mut work)
                                } else {
                                    sequence_skeleton_paths(input, &suffixes, &mut work)
                                };
                                drop(suffixes);
                                result.map(|paths| {
                                    let shape = (paths.len(), paths.capacity());
                                    drop(paths);
                                    shape
                                })
                            })()
                            .map_err(failure);
                            *work.budget = ReplayBudget::unobserved();
                            outcome
                        })
                    });
                assert!(outcome.is_ok() && !refused && first.is_none());
                assert!(!allocation.invalid && allocation.live == 0);
                assert_eq!(allocation.allocated, allocation.freed);
                eprintln!("vector old={old} count={count} stopped={stopped} reserved={paid} heap={allocation:?}");
                drop(work);
                drop(caller);
            }
        }
    }
}

#[test]
fn lexical_vector_first_work_keeps_six_fields_before_selection_floor_and_real_limits() {
    let program = path_array_main8a::fixture_program();
    let function = &program.functions[0];
    for (stopped, dag) in [(0, false), (2, false), (0, true), (2, true)] {
        let run = |old, limit, work_limit, floor_offset: Option<usize>| {
            let mut caller = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
            let mut work = SkeletonWork::new(&program, function, &mut caller);
            // Immutable inputs are independently made outside this protocol
            // observer; the separate whole-owner gate above includes all seeds.
            let mut input = if dag {
                dag_seed(128, &mut work)
            } else {
                seed(128, stopped, &mut work)
            }
            .unwrap();
            if dag && stopped == 2 {
                for path in &mut input {
                    path.residual = true;
                }
            }
            let suffixes = if dag {
                dag_seed(1, &mut work)
            } else {
                seed(1, 0, &mut work)
            }
            .unwrap();
            let observed = with_limit_usage_refusal_site(limit, || {
                measure(|| {
                    *work.budget = ReplayBudget::new();
                    let outcome = (|| {
                        work.budget.prepare_builder(function)?;
                        work.budget.reserve_skeleton(function, work_limit)?;
                        FIRST_WORK.with(|slot| slot.set(None));
                        if let Some(offset) = floor_offset {
                            assert!(set_active_floor(limit - offset));
                        }
                        let result = if old {
                            before::sequence_skeleton_paths(input, &suffixes, &mut work)
                        } else {
                            sequence_skeleton_paths(input, &suffixes, &mut work)
                        };
                        result.map(|paths| {
                            let shape = (paths.len(), paths.capacity());
                            drop(paths);
                            shape
                        })
                    })()
                    .map_err(failure);
                    let frontier = FIRST_WORK.with(|slot| slot.get());
                    let remaining = work.budget.skeleton_remaining;
                    let retry = if outcome.is_err() {
                        Some(
                            work.budget
                                .reserve_bytes(function, 0, "vector sticky retry")
                                .map_err(failure),
                        )
                    } else {
                        None
                    };
                    *work.budget = ReplayBudget::unobserved();
                    // DAG memo/backing can be retained by SkeletonWork. Keep
                    // its final Drop inside the original allocation window.
                    drop(suffixes);
                    drop(work);
                    // The assignment above already dropped the actual funded
                    // budget here. Only its resource-free unobserved replacement
                    // remains in caller, outside the observer and borrow.
                    (outcome, frontier, remaining, retry)
                })
            });
            observed
        };
        let (((ample, frontier, _, _), allocation), refused, exact, first, site) =
            run(false, usize::MAX, MAX_REPLAY_WORK_UNITS, None);
        assert!(ample.is_ok() && !refused && first.is_none() && site.is_none());
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
        for limit in [0, exact - 1, exact] {
            let (((outcome, _, _, retry), allocation), refused, paid, first, site) =
                run(false, limit, MAX_REPLAY_WORK_UNITS, None);
            assert_eq!(outcome.is_ok(), limit == exact);
            assert_eq!(refused, limit != exact);
            if refused {
                assert!(first.is_some() && site.is_some());
                assert!(retry.unwrap().is_err());
            }
            assert!(!allocation.invalid && allocation.live == 0);
            assert_eq!(allocation.allocated, allocation.freed);
            eprintln!("vector stopped={stopped} dag={dag} limit={limit} reserved={paid} heap={allocation:?}");
        }
        let (remaining, setup_paid) = frontier.expect("actual first original work frontier");
        // This floor admits all preceding original/new held selection state but
        // refuses any positive optional query. Zero original work must win.
        let offset = usize::MAX - remaining;
        let (((actual, _, left, retry), allocation), _, _, _, _) =
            run(false, usize::MAX, setup_paid, Some(offset));
        let (((expected, _, _, _), old_allocation), _, _, _, _) =
            run(true, usize::MAX, setup_paid, None);
        assert_eq!(
            actual.unwrap_err(),
            expected.unwrap_err(),
            "all six Diagnostic fields"
        );
        assert_eq!(left, 0);
        assert!(retry.unwrap().is_err());
        assert!(!allocation.invalid && allocation.live == 0);
        assert!(!old_allocation.invalid && old_allocation.live == 0);
    }
}

thread_local! { static FIRST_WORK: std::cell::Cell<Option<(usize,usize)>> = const { std::cell::Cell::new(None) }; }
pub(in crate::cleanup_plan::replay) fn record_first(work: &SkeletonWork<'_, '_>) {
    if let Some(remaining) = active_remaining() {
        FIRST_WORK.with(|slot| slot.set(Some((remaining, work.budget.skeleton_spent))));
    }
}
