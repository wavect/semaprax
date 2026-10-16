use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;

#[test]
fn complete_finished_receiver_preserves_paths_capacity_work_and_legacy() {
    let program = tests::program();
    for paid in [false, true] {
        let (((), allocation), refused, reserved, first) =
            with_limit_usage_refusal(usize::MAX, || {
                measure(|| {
                    for function in &program.functions {
                        let mut old = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                        let mut current = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                        if !paid {
                            old.payer = AllocationPayer::LegacyUnobserved;
                            current.payer = AllocationPayer::LegacyUnobserved;
                        }
                        let before = skeleton_materializations();
                        let expected = final_before::hir_skeleton_paths_with_index_storage(
                            &program, function, &mut old, None, None,
                        )
                        .unwrap();
                        let old_materializations = skeleton_materializations() - before;
                        let before = skeleton_materializations();
                        let actual = super::current::hir_skeleton_paths_with_index_storage(
                            &program,
                            function,
                            &mut current,
                            None,
                            None,
                        )
                        .unwrap();
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
        if paid {
            assert!(allocation.allocated <= reserved);
        }
    }
}

#[test]
fn finished_backing_and_future_growth_pay_at_zero_exact_and_short_limits() {
    let program = tests::program();
    let function = &program.functions[0];
    for count in [0usize, 1, 7, 8, 9, 16, 17, 512] {
        let observe = |original: bool, limit| {
            with_limit_usage_refusal(limit, || {
                measure(|| {
                    let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                    let before = skeleton_materializations();
                    let result = (|| {
                        let mut work =
                            SkeletonWork::with_index(&program, function, &mut budget, None)?;
                        let mut output = Vec::new();
                        if !original {
                            completed_minimum(
                                &mut output,
                                count,
                                &mut work,
                                "finished fixture backing",
                            )?;
                        }
                        // The old producer's original push loop and following growth.
                        for n in 0..count + 3 {
                            work.push_skeleton_path(
                                &mut output,
                                SkeletonPath {
                                    observations: Observations::default(),
                                    terminal: match n % 3 {
                                        0 => SkeletonTerminal::Success,
                                        1 => SkeletonTerminal::Failure,
                                        _ => SkeletonTerminal::LoopLatch,
                                    },
                                },
                                "finished fixture push",
                            )?;
                        }
                        for (n, path) in output.iter().enumerate() {
                            assert!(
                                path.observations.items.is_empty()
                                    && path.observations.prefix.is_none()
                            );
                            assert_eq!(
                                path.terminal,
                                match n % 3 {
                                    0 => SkeletonTerminal::Success,
                                    1 => SkeletonTerminal::Failure,
                                    _ => SkeletonTerminal::LoopLatch,
                                }
                            );
                        }
                        let result = (output.len(), output.capacity());
                        drop(output);
                        Ok::<_, Diagnostic>(result)
                    })();
                    let output = result.as_ref().ok().copied();
                    let error = result.as_ref().err().map(|diagnostic| diagnostic.code);
                    drop(result);
                    if let Some(code) = error {
                        assert_eq!(
                            budget
                                .reserve_array::<u8>(function, 0, "sticky finished refusal")
                                .unwrap_err()
                                .code,
                            code
                        );
                        assert!(crate::bounded_output::active_refused());
                    }
                    let logical = (
                        budget.remaining,
                        budget.skeleton_remaining,
                        budget.skeleton_spent,
                        skeleton_materializations() - before,
                    );
                    drop(budget);
                    (output, error, logical)
                })
            })
        };
        let ((expected, old_allocation), old_refused, old_paid, first) = observe(true, usize::MAX);
        assert!(!old_refused && first.is_none());
        let ((actual, allocation), refused, exact, first) = observe(false, usize::MAX);
        assert!(!refused && first.is_none());
        assert_eq!(actual, expected);
        let capacity = count.next_power_of_two();
        let removed = (capacity - 1) * std::mem::size_of::<SkeletonPath>();
        let eligible = count >= 8 && removed >= completed_receiving_bytes();
        if eligible {
            assert_eq!(old_allocation.allocated - allocation.allocated, removed);
            assert_eq!(old_paid - exact, removed - completed_receiving_bytes());
        } else {
            assert_eq!(old_allocation.allocated, allocation.allocated);
            assert_eq!(old_paid, exact);
        }
        let ((baseline_zero, _), old_zero_refused, _, old_first) = observe(true, 0);
        assert!(old_zero_refused && old_first.is_some());
        for limit in [0, exact - 1, exact] {
            let ((outcome, allocation), refused, paid, first) = observe(false, limit);
            assert_eq!(outcome.0.is_some(), limit == exact);
            assert_eq!(refused, limit != exact);
            assert_eq!(first.is_some(), limit != exact);
            assert!(!allocation.invalid && allocation.live == 0);
            assert_eq!(allocation.allocated, allocation.freed);
            assert!(allocation.allocated <= paid);
            if limit != exact {
                assert_eq!(outcome.1, baseline_zero.1);
            }
            if limit == 0 {
                assert_eq!(allocation.allocated, 0);
            }
        }
    }
}

#[test]
fn complete_finished_predecessor_is_compiled_and_source_bound() {
    use sha2::{Digest, Sha256};
    let record: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/completed-skeleton-array-v1/function.json"
    )))
    .unwrap();
    let body = record["body"].as_str().unwrap();
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(body.as_bytes()))
        ),
        record["sha256"]
    );
    let before = include_str!("final_before.rs");
    assert_eq!(before.split_once("pub(super) ").unwrap().1, body);
    let old = "    let mut completed = Vec::new();\n    for path in paths {";
    let new = "    let mut completed = Vec::new();\n    path_array::completed_minimum(\n        &mut completed,\n        paths.len(),\n        &mut work,\n        \"completed HIR skeleton capacity\",\n    )?;\n    for path in paths {";
    assert_eq!(body.matches(old).count(), 1);
    assert_eq!(
        crate::source_reflows_merge::main8a_input("src/cleanup_plan/replay.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay.rs")))
            .matches(&body.replacen(old, new, 1))
            .count(),
        1
    );
}
