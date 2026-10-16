use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;
use std::cell::Cell;

thread_local! { static ORIGINAL: Cell<bool> = const { Cell::new(false) }; }
pub(super) fn original_selected() -> bool {
    ORIGINAL.with(Cell::get)
}
fn with_original<T>(run: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            ORIGINAL.with(|flag| flag.set(self.0));
        }
    }
    let _restore = Restore(ORIGINAL.with(|flag| flag.replace(true)));
    run()
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
        assert_eq!(observations, old_observations);
        assert_eq!(owned_source, old_owned_source);
        assert_eq!(failed, old_failed);
        assert_eq!(residual, old_residual);
        assert_eq!(continuation, old_continuation);
    }
}

#[test]
fn complete_previous_receiver_keeps_every_field_capacity_work_and_paid_drop() {
    let program = path_array::fixture_program();
    let function = &program.functions[0];
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            for count in [0, 1, 8, 9, 16, 17] {
                for fanout in [0, 1, 2] {
                    for stopped in [0, 1, 2, 3] {
                        let mut old = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                        let mut current = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                        let mut old_work = SkeletonWork::new(&program, function, &mut old);
                        let mut work = SkeletonWork::new(&program, function, &mut current);
                        let old_inputs = seed(count, stopped, &mut old_work).unwrap();
                        let inputs = seed(count, stopped, &mut work).unwrap();
                        let pointer = inputs.as_ptr();
                        let suffix_stop = match stopped {
                            1 => 2,
                            3 => 1,
                            _ => 0,
                        };
                        let old_suffixes = seed(fanout, suffix_stop, &mut old_work).unwrap();
                        let suffixes = seed(fanout, suffix_stop, &mut work).unwrap();
                        let selected = can_reuse(Candidate {
                            prefixes: &inputs,
                            suffixes: &suffixes,
                            budget: work.budget,
                        });
                        let before = skeleton_materializations();
                        let expected = before::sequence_skeleton_paths(
                            old_inputs,
                            &old_suffixes,
                            &mut old_work,
                        )
                        .unwrap();
                        let old_materializations = skeleton_materializations() - before;
                        let before = skeleton_materializations();
                        let actual = sequence_skeleton_paths(inputs, &suffixes, &mut work).unwrap();
                        assert_eq!(skeleton_materializations() - before, old_materializations);
                        assert_expr_paths_equal(&actual, &expected);
                        assert_eq!(actual.capacity(), expected.capacity());
                        if selected {
                            assert_eq!(actual.as_ptr(), pointer);
                        }
                        drop(work);
                        drop(old_work);
                        assert_eq!(current.remaining, old.remaining);
                        assert_eq!(current.skeleton_remaining, old.skeleton_remaining);
                        assert_eq!(current.skeleton_spent, old.skeleton_spent);
                        drop(actual);
                        drop(expected);
                        drop(suffixes);
                        drop(old_suffixes);
                        drop(current);
                        drop(old);
                    }
                }
            }
        })
    });
    assert!(!refused && first.is_none());
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    assert!(allocation.allocated <= paid);
}

#[test]
fn complete_vector_backing_is_removed_with_zero_exact_short_and_future_growth() {
    let program = path_array::fixture_program();
    let function = &program.functions[0];
    for count in [16usize, 17, 64] {
        for stopped in [0, 1, 2] {
            let observe = |original, limit| {
                with_limit_usage_refusal(limit, || {
                    measure(|| {
                        let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                        let before = skeleton_materializations();
                        let result = (|| {
                            let mut work =
                                SkeletonWork::with_index(&program, function, &mut budget, None)?;
                            let inputs = seed(count, stopped, &mut work)?;
                            let pointer = inputs.as_ptr();
                            let capacity = inputs.capacity();
                            let suffixes = seed(1, 0, &mut work)?;
                            assert!(can_reuse(Candidate {
                                prefixes: &inputs,
                                suffixes: &suffixes,
                                budget: work.budget
                            }));
                            let mut output = if original {
                                before::sequence_skeleton_paths(inputs, &suffixes, &mut work)?
                            } else {
                                sequence_skeleton_paths(inputs, &suffixes, &mut work)?
                            };
                            if !original {
                                assert_eq!(output.as_ptr(), pointer);
                            }
                            assert_eq!(output.capacity(), capacity);
                            // Continue through the identical next growth frontier.
                            for _ in count..=capacity {
                                work.push_expr_path(
                                    &mut output,
                                    empty_expr_path(),
                                    "vector fixture future growth",
                                )?;
                            }
                            let result = (
                                output.len(),
                                output.capacity(),
                                output.iter().filter(|p| p.failed).count(),
                                output.iter().filter(|p| p.residual).count(),
                            );
                            drop(output);
                            drop(suffixes);
                            Ok::<_, Diagnostic>(result)
                        })();
                        let result = result.map_err(|diagnostic| diagnostic.code);
                        if let Err(code) = &result {
                            assert_eq!(
                                budget
                                    .reserve_array::<u8>(function, 0, "sticky vector refusal")
                                    .unwrap_err()
                                    .code,
                                *code
                            );
                            assert!(crate::bounded_output::active_refused());
                        }
                        let work = (
                            budget.remaining,
                            budget.skeleton_remaining,
                            budget.skeleton_spent,
                            skeleton_materializations() - before,
                        );
                        drop(budget);
                        (result, work)
                    })
                })
            };
            let ((expected, old_allocation), old_refused, old_paid, old_first) =
                observe(true, usize::MAX);
            let ((actual, allocation), refused, exact, first) = observe(false, usize::MAX);
            assert!(!old_refused && old_first.is_none());
            assert!(!refused && first.is_none());
            assert_eq!(actual, expected);
            assert_eq!(
                old_allocation.allocated - allocation.allocated,
                count.next_power_of_two() * size_of::<ExprSkeletonPath>()
            );
            assert!(exact < old_paid);
            for limit in [0, exact - 1, exact] {
                let ((outcome, allocation), refused, paid, first) = observe(false, limit);
                assert_eq!(outcome.0.is_ok(), limit == exact);
                assert_eq!(refused, limit != exact);
                assert_eq!(first.is_some(), limit != exact);
                assert!(!allocation.invalid && allocation.live == 0);
                assert_eq!(allocation.allocated, allocation.freed);
                assert!(allocation.allocated <= paid);
                if limit == 0 {
                    assert_eq!(allocation.allocated, 0);
                }
            }
        }
    }
}

#[test]
fn complete_hir_receiver_retains_original_fields_order_work_and_materializations() {
    let program = path_array::fixture_program();
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            for function in &program.functions {
                let mut old = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                let mut current = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                let before = skeleton_materializations();
                let expected =
                    with_original(|| hir_skeleton_paths(&program, function, &mut old)).unwrap();
                let old_materializations = skeleton_materializations() - before;
                let before = skeleton_materializations();
                let actual = hir_skeleton_paths(&program, function, &mut current).unwrap();
                assert_eq!(skeleton_materializations() - before, old_materializations);
                assert_eq!(actual, expected);
                assert_eq!(actual.capacity(), expected.capacity());
                assert_eq!(current.remaining, old.remaining);
                assert_eq!(current.skeleton_remaining, old.skeleton_remaining);
                assert_eq!(current.skeleton_spent, old.skeleton_spent);
                drop(actual);
                drop(expected);
                drop(current);
                drop(old);
            }
        })
    });
    assert!(!refused && first.is_none());
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    assert!(allocation.allocated <= paid);
}

#[test]
fn complete_predecessor_and_original_push_operations_stay_source_bound() {
    use sha2::{Digest, Sha256};
    let record: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/sequence-vector-reuse-v1/function.json"
    )))
    .unwrap();
    let body = record["body"].as_str().unwrap();
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(body.as_bytes()))
        ),
        "2381cbd3406937743d2fd70482267044bf99a376430ec3d5ed0ecb25360370f4"
    );
    assert_eq!(
        record["sha256"],
        "2381cbd3406937743d2fd70482267044bf99a376430ec3d5ed0ecb25360370f4"
    );
    let old = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main44-independent-custody-20261016/raw/src/cleanup_plan/replay/sequence_vector/before.rs.txt"));
    assert_eq!(old, format!("use super::super::*;\n\npub(super) {body}\n"));
    let root = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main44-independent-custody-20261016/raw/src/cleanup_plan/replay.rs.txt"));
    let hook = "    if sequence_vector::can_reuse(sequence_vector::Candidate {\n        prefixes: &prefixes,\n        suffixes: &suffixes,\n        budget: work.budget,\n    }) {\n        work.budget.reserve_array::<u8>(\n            work.function,\n            sequence_vector::receiving_bytes(),\n            \"single-successor receiving frame\",\n        )?;\n        return sequence_vector::single(prefixes, &suffixes, work, share_prefix);\n    }\n";
    let expected = body.replacen(
        "    let share_prefix = work.shares_prefix_tail(&suffixes)?;\n",
        &format!("    let share_prefix = work.shares_prefix_tail(&suffixes)?;\n{hook}"),
        1,
    );
    let expected = expected.replacen(
        "path_array::minimum(\n            &mut combined,\n            prefixes.len(),",
        "path_array::sequence::minimum(\n            &mut combined,\n            &prefixes,\n            &suffixes,",
        1,
    );
    assert_eq!(root.matches(&expected).count(), 1);
    let work = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main44-independent-custody-20261016/raw/src/cleanup_plan/replay/skeleton_work.rs.txt"));
    let original_push = "        self.charge(1, phase)?;\n        note_skeleton_materialization();\n        self.budget.reserve_vec(self.function, paths, 1, phase)?;\n        paths.push(path);";
    assert_eq!(work.matches(original_push).count(), 2);
    let live = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main44-independent-custody-20261016/raw/src/cleanup_plan/replay/sequence_vector.rs.txt"));
    for phase in ["short-circuited skeleton path", "sequenced skeleton path"] {
        let operations = format!("            work.charge(1, \"{phase}\")?;\n            note_skeleton_materialization();");
        assert!(
            live.contains(&operations)
                || live.contains(&operations.replace("            ", "        "))
        );
    }
}

#[test]
fn a_larger_input_backing_keeps_the_complete_original_output_growth_route() {
    let program = path_array::fixture_program();
    let function = &program.functions[0];
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut old = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
            let mut current = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
            let mut old_work = SkeletonWork::new(&program, function, &mut old);
            let mut work = SkeletonWork::new(&program, function, &mut current);
            let mut old_inputs = seed(16, 0, &mut old_work).unwrap();
            let mut inputs = seed(16, 0, &mut work).unwrap();
            old_work
                .budget
                .reserve_vec(function, &mut old_inputs, 1, "old extra input capacity")
                .unwrap();
            work.budget
                .reserve_vec(function, &mut inputs, 1, "extra input capacity")
                .unwrap();
            assert!(inputs.capacity() > 16);
            let old_suffixes = seed(1, 0, &mut old_work).unwrap();
            let suffixes = seed(1, 0, &mut work).unwrap();
            assert!(!can_reuse(Candidate {
                prefixes: &inputs,
                suffixes: &suffixes,
                budget: work.budget
            }));
            let unobserved = ReplayBudget::unobserved();
            assert!(!can_reuse(Candidate {
                prefixes: &inputs,
                suffixes: &suffixes,
                budget: &unobserved
            }));
            let before = skeleton_materializations();
            let expected =
                before::sequence_skeleton_paths(old_inputs, &old_suffixes, &mut old_work).unwrap();
            let old_materializations = skeleton_materializations() - before;
            let before = skeleton_materializations();
            let actual = sequence_skeleton_paths(inputs, &suffixes, &mut work).unwrap();
            assert_eq!(skeleton_materializations() - before, old_materializations);
            assert_expr_paths_equal(&actual, &expected);
            assert_eq!(actual.capacity(), 16);
            assert_eq!(actual.capacity(), expected.capacity());
            drop(work);
            drop(old_work);
            assert_eq!(current.remaining, old.remaining);
            assert_eq!(current.skeleton_remaining, old.skeleton_remaining);
            assert_eq!(current.skeleton_spent, old.skeleton_spent);
            drop(actual);
            drop(expected);
            drop(suffixes);
            drop(old_suffixes);
            drop(current);
            drop(old);
            drop(unobserved);
        })
    });
    assert!(!refused && first.is_none());
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    assert!(allocation.allocated <= paid);
}
