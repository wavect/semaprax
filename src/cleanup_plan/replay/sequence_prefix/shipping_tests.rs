//! Complete real Main-f1 versus shipping lexical sequence corpus.
use super::tests::{program, same, seed};
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::cleanup_plan::replay::*;
use crate::test_allocation_observer::measure;

#[test]
fn shipping_lexical_single_successor_matches_complete_main_f1_corpus() {
    let program = program();
    let function = &program.functions[0];
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            for count in [
                0,
                1,
                minimum_items() - 1,
                minimum_items(),
                minimum_items() + 9,
            ] {
                for fanout in [0, 1, 2] {
                    for stopped in [0, 1, 2] {
                        let mut old_budget =
                            ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                        let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                        let mut old_work = SkeletonWork::new(&program, function, &mut old_budget);
                        let mut work = SkeletonWork::new(&program, function, &mut budget);
                        let capacity = (count + 8).next_power_of_two();
                        let mut old_prefix = seed(count, capacity, &mut old_work).unwrap();
                        let mut prefix = seed(count, capacity, &mut work).unwrap();
                        old_prefix.failed = stopped == 1;
                        prefix.failed = stopped == 1;
                        old_prefix.residual = stopped == 2;
                        prefix.residual = stopped == 2;
                        let mut old_inputs = Vec::new();
                        let mut inputs = Vec::new();
                        old_work
                            .push_expr_path(&mut old_inputs, old_prefix, "prefix fixture path")
                            .unwrap();
                        work.push_expr_path(&mut inputs, prefix, "prefix fixture path")
                            .unwrap();
                        let mut old_suffix = Vec::new();
                        let mut suffix = Vec::new();
                        for n in 0..fanout {
                            let mut a = seed(n + 1, n + 1, &mut old_work).unwrap();
                            let mut b = seed(n + 1, n + 1, &mut work).unwrap();
                            a.owned_source =
                                Some(CleanupPlace::whole(StorageId::ProvisionalResult));
                            b.owned_source =
                                Some(CleanupPlace::whole(StorageId::ProvisionalResult));
                            old_work
                                .push_expr_path(&mut old_suffix, a, "suffix fixture path")
                                .unwrap();
                            work.push_expr_path(&mut suffix, b, "suffix fixture path")
                                .unwrap();
                        }
                        let before = skeleton_materializations();
                        let expected = super::main_current::sequence_skeleton_paths(
                            old_inputs,
                            &old_suffix,
                            &mut old_work,
                        )
                        .unwrap();
                        let old_materializations = skeleton_materializations() - before;
                        let before = skeleton_materializations();
                        let actual = crate::cleanup_plan::replay::sequence_skeleton_paths(
                            inputs, &suffix, &mut work,
                        )
                        .unwrap();
                        assert_eq!(skeleton_materializations() - before, old_materializations);
                        same(&actual, &expected);
                        assert_eq!(work.budget.remaining, old_work.budget.remaining);
                        assert_eq!(
                            work.budget.skeleton_remaining,
                            old_work.budget.skeleton_remaining
                        );
                    }
                }
            }
        })
    });
    assert!(!refused && first.is_none());
    assert!(
        !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
    );
    assert!(allocation.allocated <= paid);
}
