//! Complete real Main-f1 versus the shipping sequence, including ordinary forks.
use super::tests::{program, same, seed};
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::cleanup_plan::replay::skeleton_work::ObservationDagKind;
use crate::cleanup_plan::replay::*;
use crate::test_allocation_observer::measure;
use std::rc::Rc;

#[test]
fn shipping_lexical_single_successor_matches_complete_main_f1_corpus() {
    let program = program();
    let function = &program.functions[0];
    assert!(function.cleanup_plan.lexical_control.is_none());
    let renewal_scan: usize = function
        .cleanup_plan
        .blocks
        .iter()
        .map(|block| {
            assert!(!block.transitions.iter().any(|transition| matches!(
                transition,
                CleanupTransition::ReserveRenewal { .. } | CleanupTransition::Renew { .. }
            )));
            1 + block.transitions.len()
        })
        .sum();
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
                        let ordinary_fork = fanout == 2 && count > 8;
                        let shared = ordinary_fork && stopped == 0;
                        if shared {
                            // Two outputs retain one leaf. Compare the complete
                            // ordered observations against Main-f1's flat word
                            // without adding heap traffic inside the measurement.
                            assert_eq!(actual.len(), 2);
                            assert_eq!(actual.len(), expected.len());
                            let root = actual[0].observations.prefix.as_ref().unwrap();
                            let ObservationDagKind::Seq(None, prefix) = &root.kind else {
                                panic!("the original flat prefix should form one shared leaf");
                            };
                            assert_eq!(prefix.len(), count);
                            for (current, old) in actual.iter().zip(&expected) {
                                assert!(Rc::ptr_eq(
                                    current.observations.prefix.as_ref().unwrap(),
                                    root
                                ));
                                assert!(old.observations.prefix.is_none());
                                assert!(prefix
                                    .iter()
                                    .chain(&current.observations.items)
                                    .eq(&old.observations.items));
                                assert_eq!(current.owned_source, old.owned_source);
                                assert_eq!(current.failed, old.failed);
                                assert_eq!(current.residual, old.residual);
                                assert_eq!(current.continuation, old.continuation);
                            }
                        } else {
                            same(&actual, &expected);
                        }
                        // Fork admission scans the blocks/transitions and the
                        // three flat inputs. The one leaf adds one release unit.
                        let fork_work = if ordinary_fork { renewal_scan + 3 } else { 0 };
                        assert_eq!(
                            work.budget.remaining + fork_work + usize::from(shared),
                            old_work.budget.remaining
                        );
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
