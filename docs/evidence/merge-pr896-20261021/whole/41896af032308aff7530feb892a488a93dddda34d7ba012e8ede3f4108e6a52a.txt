use super::super::{Diagnostic, ReplayBudget, ResolvedFunction};
use super::{Memo, WIDTH};
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::{current_metrics, measure};
fn fixture() -> ResolvedFunction {
    super::super::super::lexical_plan_tests::fixture(super::super::super::lexical_plan_tests::WHILE)
        .1
}
#[test]
fn numeric_memo_sparse_overwrite_and_full_old_map_values_keep_exact_work() {
    let function = fixture();
    let (((), allocation), refused, paid, _) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            let before = budget.remaining;
            let mut memo = Memo::<usize>::new();
            let mut expected = std::collections::BTreeMap::new();
            for key in [127, 0, 7, 8, 63, 1, 7] {
                let required = key / WIDTH + 1;
                let expected_capacity = if required > memo.chunks.capacity() {
                    crate::cleanup_plan::replay::physical_storage::replay_growth_capacity(
                        memo.chunks.capacity(),
                        required,
                    )
                    .unwrap()
                } else {
                    memo.chunks.capacity()
                };
                memo.insert(&function, key, &mut budget, || key + 3)
                    .unwrap();
                assert_eq!(memo.chunks.capacity(), expected_capacity);
                expected.insert(key, key + 3);
            }
            for key in 0..=128 {
                assert_eq!(memo.get(&key).copied(), expected.get(&key).copied());
            }
            assert_eq!(budget.remaining, before);
            drop(expected);
            drop(memo);
            drop(budget);
        })
    });
    assert!(!refused && !allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    // The independent old map is observed but has no new-memo funding claim.
    assert!(paid > 0);
}
#[test]
fn numeric_memo_actual_exact_short_zero_and_partial_drop_remain_paid() {
    let function = fixture();
    let observe = |limit, count| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let mut memo = Memo::<usize>::new();
                let mut success = match memo.authenticate(&function, &mut budget) {
                    Ok(()) => true,
                    Err(error) => {
                        drop(error);
                        false
                    }
                };
                if success {
                    for key in [0, 7, 8, 63, 127].into_iter().take(count) {
                        if let Err(error) = memo.insert(&function, key, &mut budget, || key) {
                            assert_eq!(error.code, "SPX-H006");
                            drop(error);
                            success = false;
                            break;
                        }
                    }
                }
                if !success {
                    let at = current_metrics().allocated;
                    assert!(memo
                        .insert(&function, 0, &mut budget, || panic!("refused publication"))
                        .is_err());
                    assert_eq!(current_metrics().allocated, at);
                }
                drop(memo);
                drop(budget);
                success
            })
        })
    };
    // Include control-only activation and each actual directory/chunk frontier.
    // Sparse high-key-first is independently exercised by the full map owner.
    for count in 0..=5 {
        let ((ok, allocation), refused, exact, first) = observe(usize::MAX, count);
        assert!(ok && !refused && first.is_none());
        assert!(
            !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
        );
        assert!(allocation.allocated <= exact);
        for limit in [0, exact - 1, exact] {
            let ((ok, allocation), refused, _, first) = observe(limit, count);
            assert_eq!(ok, limit == exact);
            assert_eq!(refused, limit != exact);
            assert_eq!(first.is_none(), limit == exact);
            assert!(
                !allocation.invalid
                    && allocation.live == 0
                    && allocation.allocated == allocation.freed
            );
            if limit == 0 {
                assert_eq!(allocation.allocated, 0);
            }
        }
    }
}
#[test]
fn numeric_memo_foreign_then_restored_budget_never_reacquires() {
    let function = fixture();
    let (((), allocation), refused, _, _) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            let mut memo = Memo::<usize>::new();
            memo.insert(&function, 0, &mut budget, || 3).unwrap();
            let (_, foreign_refused, foreign_paid, _) =
                with_limit_usage_refusal(usize::MAX, || {
                    let before = current_metrics().allocated;
                    let error = memo.insert(&function, 8, &mut budget, || 9).unwrap_err();
                    drop(error);
                    assert_eq!(current_metrics().allocated, before);
                });
            assert!(!foreign_refused && foreign_paid == 0);
            let before = current_metrics().allocated;
            let error = memo.insert(&function, 8, &mut budget, || 9).unwrap_err();
            drop(error);
            assert_eq!(current_metrics().allocated, before);
            assert!(memo.failed);
            assert_eq!(memo.chunks[0].as_ref().unwrap()[0], Some(3));
            assert_eq!(memo.get(&0), None);
            assert_eq!(memo.get(&8), None);
            drop(memo);
            drop(budget);
        })
    });
    assert!(refused);
    assert!(
        !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
    );
}

#[test]
fn chunked_complete_canonical_receiver_preserves_old_work_ids_and_last_drop() {
    let function = fixture();
    let (automaton, sink, roots) = super::super::tests::automaton(&function);
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut old = super::super::before_index::Canonicalizer::new(&automaton, sink);
            let mut current = super::super::Canonicalizer::new(&automaton, sink);
            let mut old_budget = ReplayBudget::new();
            let mut budget = ReplayBudget::new();
            for root in &roots[..6] {
                old_budget
                    .reserve_array::<(usize, u64)>(&function, 1, "old canonical test root")
                    .unwrap();
                budget
                    .reserve_array::<(usize, u64)>(&function, 1, "current canonical test root")
                    .unwrap();
                let a = old.id(&function, vec![(*root, 1)], &mut old_budget);
                let b = current.id(&function, vec![(*root, 1)], &mut budget);
                assert_eq!(a, b);
                assert_eq!(old_budget.remaining, budget.remaining);
                assert_eq!(old_budget.skeleton_remaining, budget.skeleton_remaining);
            }
            drop(old);
            drop(current);
            drop(old_budget);
            drop(budget);
        })
    });
    assert!(!refused && first.is_none());
    assert!(
        !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
    );
    assert!(allocation.allocated <= paid);
}

#[test]
fn numeric_memo_foreign_function_budget_and_stale_ledger_fail_before_cached_reuse() {
    let function = fixture();
    let wrong = function.clone();
    let empty = Diagnostic::io("SPX-H006", String::new());
    for foreign_function in [false, true] {
        let (((), allocation), refused, paid, _) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let mut foreign = ReplayBudget::new();
                let mut memo = Memo::<usize>::new();
                memo.insert(&function, 0, &mut budget, || 3).unwrap();
                let foreign_remaining = foreign.remaining;
                let at = current_metrics().allocated;
                let error = if foreign_function {
                    memo.authenticate(&wrong, &mut budget)
                } else {
                    memo.authenticate(&function, &mut foreign)
                }
                .unwrap_err();
                assert_eq!(error, empty);
                drop(error);
                assert!(memo.failed);
                assert!(memo.get(&0).is_none() && !memo.contains_key(&0));
                assert_eq!(memo.chunks[0].as_ref().unwrap()[0], Some(3));
                assert_eq!(foreign.remaining, foreign_remaining);
                assert!(foreign.refusal_text.is_none() && !foreign.refusal_spent);
                assert_eq!(
                    memo.insert(&function, 1, &mut budget, || panic!(
                        "no publication after refusal"
                    )),
                    Err(Diagnostic::io("SPX-H006", String::new()))
                );
                assert_eq!(current_metrics().allocated, at);
                drop(memo);
                drop(foreign);
                drop(budget);
            })
        });
        assert!(!refused);
        assert!(
            !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
        );
        assert!(allocation.allocated <= paid && allocation.highwater <= paid);
    }
    // The complete ledger setup, retained scope and last Drop are observed.
    // Its test wrapper is not claimed as compiler-funded payload.
    let ((), allocation) = measure(|| {
        let ((mut memo, mut budget), refused, _, _) = with_limit_usage_refusal(usize::MAX, || {
            let mut budget = ReplayBudget::new();
            let mut memo = Memo::<usize>::new();
            memo.insert(&function, 0, &mut budget, || 3).unwrap();
            (memo, budget)
        });
        assert!(!refused);
        let at = current_metrics().allocated;
        assert_eq!(
            memo.authenticate(&function, &mut budget),
            Err(Diagnostic::io("SPX-H006", String::new()))
        );
        assert!(memo.failed && memo.get(&0).is_none());
        assert_eq!(
            memo.authenticate(&function, &mut budget),
            Err(Diagnostic::io("SPX-H006", String::new()))
        );
        assert_eq!(current_metrics().allocated, at);
        drop(memo);
        drop(budget);
    });
    assert!(
        !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
    );
}

#[test]
fn numeric_memo_legacy_unobserved_payer_has_no_active_debit_or_refusal_bootstrap() {
    let function = fixture();
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(0, || {
        measure(|| {
            let mut budget = ReplayBudget::unobserved();
            let mut memo = Memo::<usize>::new();
            let before = budget.remaining;
            for key in [127, 0, 8, 7] {
                memo.insert(&function, key, &mut budget, || key).unwrap();
            }
            assert_eq!(memo.get(&127), Some(&127));
            assert_eq!(budget.remaining, before);
            assert!(budget.refusal_text.is_none() && !budget.refusal_spent);
            drop(memo);
            drop(budget);
        })
    });
    assert!(!refused && paid == 0 && first.is_none());
    assert!(
        !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
    );
    // This explicit legacy control makes no selected-byte payment claim.
}

#[test]
fn chunked_complete_cached_id_authenticates_before_any_new_backing() {
    let function = fixture();
    let wrong = function.clone();
    let (automaton, sink, roots) = super::super::tests::automaton(&function);
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut current = super::super::Canonicalizer::new(&automaton, sink);
            let mut budget = ReplayBudget::new();
            budget
                .reserve_array::<(usize, u64)>(&function, 1, "memo cached initial input")
                .unwrap();
            let id = current
                .id(&function, vec![(roots[0], 1)], &mut budget)
                .unwrap();
            assert!(id.is_some());
            budget
                .reserve_array::<(usize, u64)>(&function, 1, "memo cached hostile input")
                .unwrap();
            let input = vec![(roots[0], 1)];
            let at = current_metrics().allocated;
            let work = budget.remaining;
            assert_eq!(
                current.id(&wrong, input, &mut budget),
                Err(Diagnostic::io("SPX-H006", String::new()))
            );
            assert_eq!(budget.remaining, work);
            assert_eq!(current_metrics().allocated, at);
            assert!(current.closures.get(&roots[0]).is_none());
            drop(current);
            drop(budget);
        })
    });
    assert!(!refused && first.is_none());
    assert!(
        !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
    );
    assert!(allocation.allocated <= paid && allocation.highwater <= paid);
}
