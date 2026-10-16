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
        let expression = work.clone_owned(&work.function.body.id, "minimum fixture identity")?;
        work.push_observation(
            &mut path,
            SkeletonObservation::Status {
                source: StatusSourceId {
                    expression,
                    lane: StatusLane::OperationFailure,
                },
                success: index % 2 == 0,
            },
            "minimum fixture observation",
        )?;
        path.continuation =
            lexical_skeleton::Continuation::Advancing(path.observations.items[0].clone());
        work.push_expr_path(&mut paths, path, "minimum fixture path")?;
    }
    Ok(paths)
}

fn same(actual: &[ExprSkeletonPath], expected: &[ExprSkeletonPath]) {
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
fn complete_receiver_preserves_every_field_order_capacity_and_work() {
    let program = path_array::fixture_program();
    let function = &program.functions[0];
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            for count in [0, 1, 7, 8, 9, 16, 17] {
                for fanout in [0, 1, 2, 3] {
                    for stopped in [0, 1, 2] {
                        let mut old = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                        let mut current = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                        let mut old_work = SkeletonWork::new(&program, function, &mut old);
                        let mut work = SkeletonWork::new(&program, function, &mut current);
                        let old_inputs = seed(count, stopped, &mut old_work).unwrap();
                        let inputs = seed(count, stopped, &mut work).unwrap();
                        let old_suffixes = seed(fanout, 1, &mut old_work).unwrap();
                        let suffixes = seed(fanout, 1, &mut work).unwrap();
                        let before = skeleton_materializations();
                        let expected = before::sequence_skeleton_paths(
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
                        same(&actual, &expected);
                        assert_eq!(actual.capacity(), expected.capacity());
                        // A later growth must still meet the identical frontier.
                        let capacity = actual.capacity();
                        let mut expected = expected;
                        for _ in actual.len()..=capacity {
                            work.push_expr_path(&mut actual, empty_expr_path(), "later growth")
                                .unwrap();
                            old_work
                                .push_expr_path(&mut expected, empty_expr_path(), "later growth")
                                .unwrap();
                        }
                        same(&actual, &expected);
                        assert_eq!(actual.capacity(), expected.capacity());
                        assert_eq!(work.budget.remaining, old_work.budget.remaining);
                        assert_eq!(
                            work.budget.skeleton_remaining,
                            old_work.budget.skeleton_remaining
                        );
                        assert_eq!(work.budget.skeleton_spent, old_work.budget.skeleton_spent);
                        drop(actual);
                        drop(expected);
                        drop(suffixes);
                        drop(old_suffixes);
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
fn complete_paid_drop_and_exact_short_zero_sticky_controls() {
    let program = path_array::fixture_program();
    let function = &program.functions[0];
    for count in [8usize, 16, 256, 1024] {
        let observe = |original, limit| {
            with_limit_usage_refusal(limit, || {
                measure(|| {
                    let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                    let before = skeleton_materializations();
                    let result = (|| {
                        let mut work =
                            SkeletonWork::with_index(&program, function, &mut budget, None)?;
                        let inputs = seed(count, 0, &mut work)?;
                        let suffixes = seed(2, 0, &mut work)?;
                        assert_eq!(
                            lower_bound(Candidate {
                                prefixes: &inputs,
                                suffixes: &suffixes,
                                budget: work.budget,
                            }),
                            Some(count + 1)
                        );
                        let output = if original {
                            before::sequence_skeleton_paths(inputs, &suffixes, &mut work)?
                        } else {
                            sequence_skeleton_paths(inputs, &suffixes, &mut work)?
                        };
                        let shape = (output.len(), output.capacity());
                        drop(output);
                        drop(suffixes);
                        Ok::<_, Diagnostic>(shape)
                    })();
                    let result = result.map_err(|diagnostic| diagnostic.code);
                    if let Err(code) = &result {
                        assert_eq!(
                            budget
                                .reserve_array::<u8>(function, 0, "sticky minimum refusal")
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
        let ((old_result, old_allocation), old_refused, old_paid, old_first) =
            observe(true, usize::MAX);
        let ((result, allocation), refused, exact, first) = observe(false, usize::MAX);
        assert!(!old_refused && old_first.is_none());
        assert!(!refused && first.is_none());
        assert_eq!(result, old_result);
        assert_eq!(
            old_allocation.allocated - allocation.allocated,
            count * size_of::<ExprSkeletonPath>()
        );
        assert_eq!(
            old_paid - exact,
            count * size_of::<ExprSkeletonPath>() - receiving_bytes()
        );
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

#[test]
fn complete_previous_receiver_is_source_bound_except_for_the_new_minimum() {
    use sha2::{Digest, Sha256};
    let previous = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_array/sequence/before.rs"))
        .split_once("pub(super) fn sequence_skeleton_paths(")
        .unwrap()
        .1;
    let previous = format!("fn sequence_skeleton_paths({previous}");
    let previous = previous.trim_end();
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(previous.as_bytes()))
        ),
        "fd54e2c120585d772809e8d0cc8f1c9ea4fda1aff898239c27cd05a9556ccae7"
    );
    let source = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main44-independent-custody-20261016/raw/src/cleanup_plan/replay.rs.txt"));
    let current = source
        .split_once("fn sequence_skeleton_paths(")
        .unwrap()
        .1
        .split_once("\nuse path_summary::has_active_paths;")
        .unwrap()
        .0;
    let current = format!("fn sequence_skeleton_paths({current}");
    let restored = current.replace(
        "path_array::sequence::minimum(\n            &mut combined,\n            &prefixes,\n            &suffixes,",
        "path_array::minimum(\n            &mut combined,\n            prefixes.len(),",
    );
    assert_eq!(restored.trim_end(), previous);
}

#[test]
fn header_probe_keeps_small_stopped_selected_and_unpaid_routes_closed() {
    let mut prefixes = (0..16).map(|_| empty_expr_path()).collect::<Vec<_>>();
    let suffixes = (0..3).map(|_| empty_expr_path()).collect::<Vec<_>>();
    let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
    budget.payer = AllocationPayer::ActiveBuilder;
    let probe =
        |prefixes: &[ExprSkeletonPath], suffixes: &[ExprSkeletonPath], budget: &ReplayBudget| {
            lower_bound(Candidate {
                prefixes,
                suffixes,
                budget,
            })
        };
    assert_eq!(probe(&prefixes, &suffixes, &budget), Some(18));
    assert_eq!(with_original(|| probe(&prefixes, &suffixes, &budget)), None);
    assert_eq!(probe(&prefixes[..7], &suffixes, &budget), None);
    assert_eq!(probe(&prefixes, &suffixes[..1], &budget), None);
    assert_eq!(probe(&prefixes[1..], &suffixes[..2], &budget), None);
    prefixes[0].failed = true;
    assert_eq!(probe(&prefixes, &suffixes, &budget), None);
    prefixes[0].failed = false;
    prefixes[0].residual = true;
    assert_eq!(probe(&prefixes, &suffixes, &budget), None);
    prefixes[0].residual = false;
    budget.admission = AdmissionMode::SelectedRetained;
    assert_eq!(probe(&prefixes, &suffixes, &budget), None);
    budget.admission = AdmissionMode::Ordinary;
    budget.payer = AllocationPayer::LegacyUnobserved;
    assert_eq!(probe(&prefixes, &suffixes, &budget), None);
    assert!(!crate::bounded_output::active_refused());
}

#[test]
fn real_hir_paths_keep_original_order_work_and_every_complete_drop() {
    let program = path_array::fixture_program();
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            for function in &program.functions {
                let mut old = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                let mut current = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                let before = skeleton_materializations();
                let expected = with_original(|| {
                    hir_skeleton_paths_with_index_storage(&program, function, &mut old, None, None)
                })
                .unwrap();
                let materializations = skeleton_materializations() - before;
                let before = skeleton_materializations();
                let actual = hir_skeleton_paths_with_index_storage(
                    &program,
                    function,
                    &mut current,
                    None,
                    None,
                )
                .unwrap();
                assert_eq!(skeleton_materializations() - before, materializations);
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
fn both_prior_source_controls_keep_their_complete_original_subjects_and_assertions() {
    use sha2::{Digest, Sha256};
    let records: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/sequence-minimum-v1/source-controls.json"
    )))
    .unwrap();
    let records = records.as_array().unwrap();
    assert_eq!(records.len(), 2);
    let sources = [
        (
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main44-independent-custody-20261016/raw/src/cleanup_plan/replay/path_array/tests.rs.txt")),
            "9e7c649b2ad6db2df8418c4ad710f1cdd7bf40814d8f7ca4f47ec7ab35df9ed4",
            "42087969be0964e2dbffe07e9c01e07a02f17e3ee416a9c2629e7ee41b94b2cc",
        ),
        (
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main44-independent-custody-20261016/raw/src/cleanup_plan/replay/sequence_vector/tests.rs.txt")),
            "2f1bee2e7e1812c0c8524d1a775992a7808a40d1f32420788b549617899b76da",
            "b6324b3c6c35ab53b4a2a19b27acaca38a9256464865ecbabcb2e8f90d649feb",
        ),
    ];
    let digest = |text: &str| {
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
        )
    };
    for (record, (source, before_hash, after_hash)) in records.iter().zip(sources) {
        let name = record["name"].as_str().unwrap();
        let start = source.find(&format!("fn {name}(")).unwrap();
        let tail = &source[start..];
        let current = tail
            .split_once("\n#[test]\n")
            .map_or(tail, |(body, _)| body)
            .trim_end();
        assert_eq!(digest(current), after_hash);
        assert_eq!(record["after_sha256"], after_hash);
        assert_eq!(record["before_sha256"], before_hash);
        assert_eq!(digest(record["body"].as_str().unwrap()), before_hash);
        let mut restored = current.to_owned();
        for edit in record["inverse"].as_array().unwrap() {
            let added = edit["after"].as_str().unwrap();
            assert_eq!(edit["before"], "");
            assert_eq!(restored.matches(added).count(), 1);
            restored = restored.replacen(added, "", 1);
        }
        assert_eq!(restored, record["body"].as_str().unwrap());
        assert_eq!(digest(&restored), before_hash);
    }
}
