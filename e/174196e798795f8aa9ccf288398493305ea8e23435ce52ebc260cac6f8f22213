//! Whole shipping receiver controls; the full predecessor keeps its old macros.
use super::super::super::*;
use super::immediate as before;
use super::optimized::{capacity, HEADER, TOTAL as RECEIVING};
use crate::bounded_output::{
    active_remaining, set_active_floor, with_limit_usage_refusal, with_limit_usage_refusal_site,
};
use crate::test_allocation_observer::measure;
#[path = "../../sequence_vector/observation_parity.rs"]
mod observation_parity;
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
        if observations.prefix.is_some() || old_observations.prefix.is_some() {
            observation_parity::assert_equal(observations, old_observations);
        } else {
            assert_eq!(observations, old_observations);
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
fn staged_header_preserves_complete_fields_work_growth_and_drop() {
    let program = path_array_main8a::fixture_program();
    let function = &program.functions[0];
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            for count in [0, 1, 7, 8, 9, 16, 17, 64] {
                for fanout in [0, 1, 2, 3] {
                    for stopped in [0, 1, 2] {
                        let mut old = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                        let mut current = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                        let mut old_work = SkeletonWork::new(&program, function, &mut old);
                        let mut work = SkeletonWork::new(&program, function, &mut current);
                        let old_inputs = seed(count, stopped, &mut old_work).unwrap();
                        let inputs = seed(count, stopped, &mut work).unwrap();
                        let old_suffixes = seed(fanout, 0, &mut old_work).unwrap();
                        let suffixes = seed(fanout, 0, &mut work).unwrap();
                        let before = skeleton_materializations();
                        let mut expected = before::sequence_skeleton_paths(
                            old_inputs,
                            &old_suffixes,
                            &mut old_work,
                        )
                        .unwrap();
                        let materializations = skeleton_materializations() - before;
                        let before = skeleton_materializations();
                        let mut actual =
                            sequence_skeleton_paths(inputs, &suffixes, &mut work).unwrap();
                        assert_eq!(skeleton_materializations() - before, materializations);
                        assert_expr_paths_equal(&actual, &expected);
                        assert_eq!(actual.capacity(), expected.capacity());
                        let old_capacity = actual.capacity();
                        for _ in actual.len()..=old_capacity {
                            work.push_expr_path(
                                &mut actual,
                                empty_expr_path(),
                                "minimum later growth",
                            )
                            .unwrap();
                            old_work
                                .push_expr_path(
                                    &mut expected,
                                    empty_expr_path(),
                                    "minimum later growth",
                                )
                                .unwrap();
                        }
                        assert_expr_paths_equal(&actual, &expected);
                        assert_eq!(actual.capacity(), expected.capacity());
                        assert_eq!(current.skeleton_remaining, old.skeleton_remaining);
                        assert_eq!(current.skeleton_spent, old.skeleton_spent);
                        assert_eq!(current.remaining, old.remaining);
                        drop((actual, expected, suffixes, old_suffixes));
                    }
                }
            }
            // Real DAG observations with retained tails on both sides exercise
            // the unchanged final-suffix move and complete general extension.
            let mut old = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
            let mut current = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
            let mut old_work = SkeletonWork::new(&program, function, &mut old);
            let mut work = SkeletonWork::new(&program, function, &mut current);
            let old_inputs = dag_seed(64, &mut old_work).unwrap();
            let inputs = dag_seed(64, &mut work).unwrap();
            let old_suffixes = dag_seed(3, &mut old_work).unwrap();
            let suffixes = dag_seed(3, &mut work).unwrap();
            let before = skeleton_materializations();
            let expected =
                before::sequence_skeleton_paths(old_inputs, &old_suffixes, &mut old_work).unwrap();
            let materializations = skeleton_materializations() - before;
            let before = skeleton_materializations();
            let actual = sequence_skeleton_paths(inputs, &suffixes, &mut work).unwrap();
            assert_eq!(skeleton_materializations() - before, materializations);
            assert_eq!(actual.len(), expected.len());
            assert_eq!(actual.capacity(), expected.capacity());
            assert_eq!(current.skeleton_spent, old.skeleton_spent);
            assert_eq!(current.skeleton_remaining, old.skeleton_remaining);
            drop((actual, expected, suffixes, old_suffixes));
        })
    });
    assert!(!refused && first.is_none());
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    assert!(allocation.allocated <= paid);

    // Full DAG values are compared outside the allocation observer: both
    // independent drivers allocate their own new concatenation roots. Keep the
    // measured construction-through-Drop/pointer/work controls above intact.
    observation_parity::verify_contract(&function.body.id, &program.functions[1].body.id);
    let ((), refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
        let mut old = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
        let mut current = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
        let mut old_work = SkeletonWork::new(&program, function, &mut old);
        let mut work = SkeletonWork::new(&program, function, &mut current);
        let old_inputs = dag_seed(64, &mut old_work).unwrap();
        let inputs = dag_seed(64, &mut work).unwrap();
        let old_suffixes = dag_seed(3, &mut old_work).unwrap();
        let suffixes = dag_seed(3, &mut work).unwrap();
        let before = skeleton_materializations();
        let expected =
            before::sequence_skeleton_paths(old_inputs, &old_suffixes, &mut old_work).unwrap();
        let materializations = skeleton_materializations() - before;
        let before = skeleton_materializations();
        let actual = sequence_skeleton_paths(inputs, &suffixes, &mut work).unwrap();
        assert_eq!(skeleton_materializations() - before, materializations);
        assert_expr_paths_equal(&actual, &expected);
        assert_eq!(actual.capacity(), expected.capacity());
        assert_eq!(current.skeleton_spent, old.skeleton_spent);
        assert_eq!(current.skeleton_remaining, old.skeleton_remaining);
        drop((actual, expected, suffixes, old_suffixes));
    });
    assert!(!refused && first.is_none());
}

#[test]
fn staged_probe_reports_every_paid_and_heap_delta_with_exact_sticky_accounts() {
    let program = path_array_main8a::fixture_program();
    let function = &program.functions[0];
    for count in [8, 9, 16, 64] {
        let run = |old, limit| {
            with_limit_usage_refusal_site(limit, || {
                measure(|| {
                    let mut budget = ReplayBudget::new();
                    let result = (|| {
                        budget.prepare_builder(function)?;
                        budget.reserve_skeleton(function, MAX_REPLAY_WORK_UNITS)?;
                        let mut work = SkeletonWork::new(&program, function, &mut budget);
                        let inputs = seed(count, 0, &mut work)?;
                        let suffixes = seed(2, 0, &mut work)?;
                        let result = if old {
                            before::sequence_skeleton_paths(inputs, &suffixes, &mut work)
                        } else {
                            sequence_skeleton_paths(inputs, &suffixes, &mut work)
                        };
                        result.map(|paths| {
                            let shape = (paths.len(), paths.capacity());
                            drop(paths);
                            shape
                        })
                    })()
                    .map_err(failure);
                    let retry = if result.is_err() {
                        Some(
                            budget
                                .reserve_bytes(function, 0, "minimum sticky retry")
                                .map_err(failure),
                        )
                    } else {
                        None
                    };
                    drop(budget);
                    (result, retry)
                })
            })
        };
        let (((actual, _), allocation), refused, exact, first, site) = run(false, usize::MAX);
        let (((expected, _), old_allocation), old_refused, old_paid, old_first, old_site) =
            run(true, usize::MAX);
        assert_eq!(actual.unwrap(), expected.unwrap());
        assert!(!refused && first.is_none() && site.is_none());
        assert!(!old_refused && old_first.is_none() && old_site.is_none());
        assert!(!allocation.invalid && allocation.live == 0);
        assert!(!old_allocation.invalid && old_allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
        assert_eq!(old_allocation.allocated, old_allocation.freed);
        assert!(allocation.allocated <= exact && old_allocation.allocated <= old_paid);
        let old_capacity = count.next_power_of_two();
        let selected = capacity(count, 1, old_capacity);
        let old_selected = super::capacity(count, 1, old_capacity);
        let size = std::mem::size_of::<ExprSkeletonPath>();
        let saved = (selected - old_capacity) * size;
        let old_saved = (old_selected - old_capacity) * size;
        let added = if count + 1 <= old_capacity {
            HEADER
        } else {
            RECEIVING
        };
        assert_eq!(
            old_allocation.allocated + old_saved,
            allocation.allocated + saved
        );
        assert_eq!(
            old_paid + old_saved + added,
            exact + saved + super::RECEIVING
        );
        if count == 64 {
            assert!(selected > old_capacity);
        }
        if count == 9 {
            assert_eq!(capacity(9, 1, 16), 16);
            assert!(HEADER < super::RECEIVING);
            assert_eq!(exact + super::RECEIVING, old_paid + HEADER);
            assert!(exact < old_paid, "real no-growth paid reduction");
            assert_eq!(allocation.allocated, old_allocation.allocated);
        }
        eprintln!("sequence minimum count={count} current_paid={exact} old_paid={old_paid} current_heap={allocation:?} old_heap={old_allocation:?}");
        for limit in [0, exact - 1, exact] {
            let (((outcome, retry), allocation), refused, paid, first, site) = run(false, limit);
            assert_eq!(outcome.is_ok(), limit == exact);
            assert_eq!(refused, limit != exact);
            if refused {
                assert!(first.is_some() && site.is_some());
                assert!(retry.unwrap().is_err());
            }
            assert!(!allocation.invalid && allocation.live == 0);
            assert_eq!(allocation.allocated, allocation.freed);
            assert!(allocation.allocated <= paid);
        }
    }
}

#[test]
fn total_capacity_covers_both_frames_and_preserves_overflow_fallbacks() {
    assert_eq!(capacity(9, 1, 16), 16);
    assert_eq!(capacity(usize::MAX, 1, 16), 16);
    assert_eq!(
        capacity(MAX_REPLAY_PATHS, 1, MAX_REPLAY_PATHS),
        MAX_REPLAY_PATHS
    );
    for prefixes in [8usize, 16, 64, 256] {
        let old = prefixes.next_power_of_two();
        let selected = capacity(prefixes, 1, old);
        if selected != old {
            assert_eq!(selected, (prefixes + 1).next_power_of_two());
            assert!((selected - old) * std::mem::size_of::<ExprSkeletonPath>() >= RECEIVING);
        }
    }
}

thread_local! { static FIRST_WORK: std::cell::Cell<Option<(usize,usize)>> = const { std::cell::Cell::new(None) }; }
pub(in crate::cleanup_plan::replay) fn record_first(work: &SkeletonWork<'_, '_>) {
    if let Some(remaining) = active_remaining() {
        FIRST_WORK.with(|slot| slot.set(Some((remaining, work.budget.skeleton_spent))));
    }
}

#[test]
fn original_first_work_still_wins_before_paid_header_at_real_floor() {
    let program = path_array_main8a::fixture_program();
    let function = &program.functions[0];
    let run = |old, work_limit, floor_offset: Option<usize>| {
        let mut budget = ReplayBudget::unobserved();
        budget
            .reserve_skeleton(function, MAX_REPLAY_WORK_UNITS)
            .unwrap();
        let mut work = SkeletonWork::new(&program, function, &mut budget);
        let inputs = seed(64, 0, &mut work).unwrap();
        let suffixes = seed(2, 0, &mut work).unwrap();
        let observed = with_limit_usage_refusal_site(usize::MAX, || {
            measure(|| {
                *work.budget = ReplayBudget::new();
                let outcome = (|| {
                    work.budget.prepare_builder(function)?;
                    work.budget.reserve_skeleton(function, work_limit)?;
                    FIRST_WORK.with(|slot| slot.set(None));
                    if let Some(offset) = floor_offset {
                        assert!(set_active_floor(usize::MAX - offset));
                    }
                    let result = if old {
                        before::sequence_skeleton_paths(inputs, &suffixes, &mut work)
                    } else {
                        sequence_skeleton_paths(inputs, &suffixes, &mut work)
                    };
                    result.map(|paths| {
                        let count = paths.len();
                        drop(paths);
                        count
                    })
                })()
                .map_err(failure);
                let frontier = FIRST_WORK.with(|slot| slot.get());
                let remaining = work.budget.skeleton_remaining;
                *work.budget = ReplayBudget::unobserved();
                (outcome, frontier, remaining)
            })
        });
        drop(suffixes);
        drop(work);
        drop(budget);
        observed
    };
    let (((result, frontier, _), allocation), refused, _, first, site) =
        run(false, MAX_REPLAY_WORK_UNITS, None);
    assert!(result.is_ok() && !refused && first.is_none() && site.is_none());
    assert!(!allocation.invalid && allocation.live == 0);
    let (remaining, spent) = frontier.expect("actual first sequenced push work frontier");
    let (((actual, _, left), allocation), _, _, _, _) =
        run(false, spent, Some(usize::MAX - remaining));
    let (((expected, _, _), old_allocation), _, _, _, _) = run(true, spent, None);
    assert_eq!(
        actual.unwrap_err(),
        expected.unwrap_err(),
        "all six Diagnostic fields"
    );
    assert_eq!(left, 0);
    assert!(!allocation.invalid && allocation.live == 0);
    assert!(!old_allocation.invalid && old_allocation.live == 0);
}

fn isolated_first(
    old: bool,
    suffixes: &[ExprSkeletonPath],
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    let mut result = Vec::new();
    let minimum: usize = path_array::count!(work, true, 9, "isolated sequencing count");
    path_array::select!(first, minimum, &result, work);
    let pending = suffixes[1..].iter();
    if old {
        super::push!(
            work,
            first,
            minimum,
            pending,
            &mut result,
            empty_expr_path(),
            "sequenced skeleton path"
        );
    } else {
        super::optimized::push!(
            work,
            first,
            minimum,
            pending,
            &mut result,
            empty_expr_path(),
            "sequenced skeleton path"
        );
    }
    assert!(!first);
    Ok(result)
}

#[test]
fn selected_unpaid_and_warm_receivers_keep_complete_drop_and_real_fee_deltas() {
    let program = path_array_main8a::fixture_program();
    let function = &program.functions[0];
    for selected in [false, true] {
        let run = |old| {
            let mut budget = if selected {
                ReplayBudget::with_physical(function, 1 << 20).unwrap()
            } else {
                ReplayBudget::unobserved()
            };
            budget
                .reserve_skeleton(function, MAX_REPLAY_WORK_UNITS)
                .unwrap();
            let suffixes = vec![empty_expr_path(), empty_expr_path()];
            let (work, allocation) = measure(|| {
                let mut spent = 0;
                let mut shape = (0, 0);
                for _ in 0..2 {
                    let mut work = SkeletonWork::new(&program, function, &mut budget);
                    let result = isolated_first(old, &suffixes, &mut work).unwrap();
                    assert_eq!(result.len(), 1);
                    if selected {
                        assert_eq!(result.capacity(), 16);
                    }
                    shape = (result.len(), result.capacity());
                    spent = work.budget.skeleton_spent;
                    drop(result);
                }
                (spent, shape)
            });
            let paid = budget
                .physical
                .as_deref()
                .map_or(0, |ledger| ledger.charged());
            drop((budget, suffixes));
            (work, allocation, paid)
        };
        let (actual_work, actual_heap, actual_paid) = run(false);
        let (old_work, old_heap, old_paid) = run(true);
        assert_eq!(actual_work, old_work);
        assert!(!actual_heap.invalid && !old_heap.invalid);
        assert_eq!(actual_heap.live, 0);
        assert_eq!(old_heap.live, 0);
        assert_eq!(actual_heap.allocated, actual_heap.freed);
        assert_eq!(old_heap.allocated, old_heap.freed);
        assert_eq!(actual_heap.allocated, old_heap.allocated);
        if selected {
            assert_eq!(actual_paid + 2 * super::RECEIVING, old_paid + 2 * HEADER);
            assert!(actual_paid < old_paid);
        } else {
            assert_eq!(actual_paid, old_paid);
        }
        eprintln!("isolated sequence selected={selected} new_paid={actual_paid} old_paid={old_paid} new_heap={actual_heap:?} old_heap={old_heap:?}");
    }
}

#[test]
fn expired_and_foreign_account_keep_the_original_failure_before_probe() {
    let program = path_array_main8a::fixture_program();
    let function = &program.functions[0];
    for foreign in [false, true] {
        let run = |old| {
            let scope =
                allocation::OracleScope::open_with_work(1 << 20, MAX_REPLAY_WORK_UNITS).unwrap();
            let lease = scope.lease();
            // The real constructor requires the same live subject's explicit
            // invocation authority. An account alone grants no source authority.
            let invocation = allocation::enter(&program, function, lease).unwrap();
            let mut budget = ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS, lease);
            let suffixes = vec![empty_expr_path(), empty_expr_path()];
            // Authenticate the actual source inventory before retiring its owner.
            let mut work = SkeletonWork::new(&program, function, &mut budget);
            let setup = scope.receipt();
            assert!(setup.charged > 0 && setup.charged < 1 << 20);
            assert!(setup.work > 0 && setup.work < MAX_REPLAY_WORK_UNITS);
            assert!(setup.first_refusal.is_none() && setup.first_work_refusal.is_none());
            assert!(setup.physical_failure.is_none());
            drop(scope);
            let live = foreign.then(|| {
                allocation::OracleScope::open_with_work(1 << 20, MAX_REPLAY_WORK_UNITS).unwrap()
            });
            let before = live.as_ref().map(|scope| scope.receipt().charged);
            let result = isolated_first(old, &suffixes, &mut work).map_err(failure);
            assert!(result.is_err());
            let after = live.as_ref().map(|scope| scope.receipt().charged);
            assert_eq!(
                after, before,
                "foreign account cannot pay an expired source owner"
            );
            drop(work);
            drop((budget, suffixes, live));
            drop(invocation);
            result
        };
        let actual = run(false).err().expect("optimized receiver must refuse");
        let expected = run(true).err().expect("original receiver must refuse");
        assert_eq!(actual, expected, "all six original Diagnostic fields");
    }
}

#[test]
fn positive_work_probe_header_and_full_query_floor_keep_original_six_fields() {
    let program = path_array_main8a::fixture_program();
    let function = &program.functions[0];
    for full_query in [false, true] {
        let run = |old| {
            with_limit_usage_refusal_site(1 << 20, || {
                measure(|| {
                    let mut budget = ReplayBudget::new();
                    let mut work = SkeletonWork::new(&program, function, &mut budget);
                    work.budget.reserve_skeleton(function, 1).unwrap();
                    let suffixes = [empty_expr_path(), empty_expr_path()];
                    let mut target = Vec::new();
                    let materializations = skeleton_materializations();
                    let mut expected_floor = 0;
                    let mut initial_remaining = 0;
                    let result = (|| {
                        // P9/S2 declines after HEADER; P8/S2 reaches the full
                        // query. Both begin at the actual paid empty target.
                        let prefixes: usize = if full_query { 8 } else { 9 };
                        let minimum =
                            path_array::count!(work, true, prefixes, "floor sequencing count");
                        path_array::select!(first, minimum, &target, work);
                        assert!(first && !work.budget.selected());
                        assert_eq!(work.budget.skeleton_remaining, 1);
                        let pending = suffixes[1..].iter();
                        initial_remaining = active_remaining().unwrap();
                        // Derive only the exact earlier payments from their
                        // production formulas. This is a real reserved floor,
                        // not an artificially small absolute byte allowance.
                        let before_probe =
                            path_array::capacity_controls() + if full_query { HEADER } else { 0 };
                        expected_floor = initial_remaining.checked_sub(before_probe).unwrap();
                        assert!(expected_floor > super::RECEIVING && HEADER < super::RECEIVING);
                        assert!(set_active_floor(expected_floor));
                        let outcome: Result<(), Diagnostic> = (|| {
                            if old {
                                super::push!(
                                    work,
                                    first,
                                    minimum,
                                    pending,
                                    &mut target,
                                    empty_expr_path(),
                                    "sequenced skeleton path"
                                );
                            } else {
                                super::optimized::push!(
                                    work,
                                    first,
                                    minimum,
                                    pending,
                                    &mut target,
                                    empty_expr_path(),
                                    "sequenced skeleton path"
                                );
                            }
                            Ok(())
                        })();
                        assert!(first, "failed bank cannot publish the first output");
                        outcome
                    })()
                    .map_err(failure);
                    assert!(result.is_err());
                    assert_eq!(work.budget.skeleton_spent, 1, "positive original work ran");
                    assert_eq!(work.budget.skeleton_remaining, 0);
                    assert_eq!(work.budget.remaining, 0);
                    assert_eq!(skeleton_materializations(), materializations + 1);
                    assert!(target.is_empty() && target.capacity() == 0);
                    let retry = work
                        .budget
                        .reserve_bytes(function, 0, "floor probe retry")
                        .map_err(failure);
                    assert_eq!(
                        retry,
                        Err(failure(Diagnostic::io("SPX-H006", String::new())))
                    );
                    drop((retry, target, suffixes));
                    drop(work);
                    drop(budget);
                    (result, expected_floor, initial_remaining)
                })
            })
        };
        let (((actual, floor, initial), allocation), refused, _, first, site) = run(false);
        let (
            ((expected, old_floor, old_initial), old_allocation),
            old_refused,
            _,
            old_first,
            old_site,
        ) = run(true);
        let actual = actual.unwrap_err();
        assert_eq!(
            actual,
            expected.unwrap_err(),
            "all six original Diagnostic fields"
        );
        assert_eq!(
            actual,
            failure(replay_error(function, format_args!(
                "cleanup replay physical reservation exhausted during sequenced minimum receiving frame"
            ))),
            "physical bank refusal, not the original work failure"
        );
        assert!(refused && old_refused);
        let first = first.expect("actual HEADER or full-query floor refusal");
        let old_first = old_first.expect("complete original receiving-bank floor refusal");
        assert_eq!(
            first.requested,
            if full_query { super::RECEIVING } else { HEADER }
        );
        assert_eq!(old_first.requested, super::RECEIVING);
        assert_eq!((first.remaining, first.floor), (floor, floor));
        assert_eq!(old_first.floor, old_floor);
        assert_eq!(
            old_first.remaining,
            old_floor + if full_query { HEADER } else { 0 }
        );
        assert!(first.requested > 0 && first.requested <= first.remaining);
        assert!(old_first.requested > 0 && old_first.requested <= old_first.remaining);
        assert_eq!((initial, floor), (old_initial, old_floor));
        assert!(
            site.is_some() && old_site.is_some(),
            "both actual first sites survive retry"
        );
        assert!(!allocation.invalid && allocation.live == 0);
        assert!(!old_allocation.invalid && old_allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
        assert_eq!(old_allocation.allocated, old_allocation.freed);
        eprintln!("sequence probe floor full_query={full_query} current={first:?} old={old_first:?} current_site={site:?} old_site={old_site:?}");
    }
}

#[test]
fn oracle_path_fixture_without_invocation_keeps_original_entry_refusal() {
    // An account must never substitute for invocation authority. Keep the
    // formerly accidental setup failure as a deliberate, fallible negative.
    let program = path_array_main8a::fixture_program();
    let function = &program.functions[0];
    let scope = allocation::OracleScope::open_with_work(1 << 20, MAX_REPLAY_WORK_UNITS).unwrap();
    let mut budget = ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS, scope.lease());
    let error = match SkeletonWork::with_index(&program, function, &mut budget, None) {
        Err(error) => error,
        Ok(_) => panic!("account without invocation granted source authority"),
    };
    assert_eq!(error.code, "SPX-H006");
    assert!(error.message.is_empty());
    let receipt = scope.receipt();
    assert_eq!(receipt.charged, 0);
    assert_eq!(receipt.work, 0);
    assert_eq!(receipt.first_refusal, None);
    assert_eq!(receipt.first_work_refusal, None);
    assert_eq!(
        receipt.physical_failure,
        Some("oracle replay budget refused physical construction")
    );
    drop((error, budget, scope));
}
