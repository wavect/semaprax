//! Exact immediate-old receivers and physical scratch lifetime controls.
use super::{before_scratch, Canonicalizer, Diagnostic, ReplayBudget};
use super::before_held::memo;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::{current_metrics, measure};

fn fixture() -> super::ResolvedFunction {
    super::super::lexical_plan_tests::fixture(super::super::lexical_plan_tests::WHILE).1
}

#[test]
fn retained_scratch_complete_old_ids_weights_signatures_work_and_diagnostics() {
    let function = fixture();
    let (automaton, sink, roots) = super::tests::automaton(&function);
    let mut old = before_scratch::Canonicalizer::new(&automaton, sink);
    let mut current = Canonicalizer::new(&automaton, sink);
    let mut old_budget = ReplayBudget::new();
    let mut budget = ReplayBudget::new();
    for root in roots[..6].iter().chain(roots[..6].iter()) {
        assert_eq!(
            old.id(&function, vec![(*root, 1)], &mut old_budget),
            current.id(&function, vec![(*root, 1)], &mut budget),
        );
        assert_eq!(budget.remaining, old_budget.remaining);
        assert_eq!(budget.skeleton_remaining, old_budget.skeleton_remaining);
        assert_eq!(&current.ids, old.signatures());
        assert_eq!(&current.states, old.states());
        assert!(current.frames.is_empty() && current.open.is_empty() && current.cursor.is_empty());
        super::ordered_weights::assert_closure_result(
            &current.closure(&function, *root, &mut budget),
            &old.closure(&function, *root, &mut old_budget),
        );
        assert_eq!(budget.remaining, old_budget.remaining);
    }
    // Overflow and cycle compare the complete Result/Diagnostic, not its code.
    for state in [vec![(roots[3], u64::MAX)], vec![(roots[6], 1)]] {
        assert_eq!(
            old.id(&function, state.clone(), &mut old_budget),
            current.id(&function, state, &mut budget),
        );
        assert_eq!(budget.remaining, old_budget.remaining);
        assert!(current.frames.is_empty() && current.open.is_empty() && current.cursor.is_empty());
    }
}

#[test]
fn retained_scratch_complete_old_every_work_frontier_and_failure_cleanup() {
    let function = fixture();
    let (automaton, sink, roots) = super::tests::automaton(&function);
    let mut old = before_scratch::Canonicalizer::new(&automaton, sink);
    let mut probe = ReplayBudget::new();
    let initial = probe.remaining;
    old.id(&function, vec![(roots[3], 1)], &mut probe).unwrap();
    let spent = initial - probe.remaining;
    for units in 0..=spent {
        let mut old = before_scratch::Canonicalizer::new(&automaton, sink);
        let mut current = Canonicalizer::new(&automaton, sink);
        let mut a = ReplayBudget::new();
        let mut b = ReplayBudget::new();
        a.remaining = units;
        b.remaining = units;
        assert_eq!(
            old.id(&function, vec![(roots[3], 1)], &mut a),
            current.id(&function, vec![(roots[3], 1)], &mut b),
        );
        assert_eq!(a.remaining, b.remaining);
        assert_eq!(a.skeleton_remaining, b.skeleton_remaining);
        assert!(current.frames.is_empty() && current.open.is_empty() && current.cursor.is_empty());
    }
}

#[test]
fn retained_closure_cached_route_constructs_no_cursor_and_rejects_foreign_authentication() {
    let function = fixture();
    let (automaton, sink, roots) = super::tests::automaton(&function);
    let mut current = Canonicalizer::new(&automaton, sink);
    let mut budget = ReplayBudget::new();
    let ((), refused, _, _) = with_limit_usage_refusal(usize::MAX, || {
        current.closure(&function, roots[3], &mut budget).unwrap();
    });
    assert!(!refused);
    // The same held ledger must stay active across both operations. This
    // separate scope is deliberately foreign and must refuse without payment.
    let (((), allocation), refused, paid, _) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let before = current_metrics().allocated;
            assert_eq!(
                current.closure(&function, roots[3], &mut budget),
                Err(Diagnostic::io("SPX-H006", String::new())),
            );
            assert_eq!(current_metrics().allocated, before);
        })
    });
    assert!(!refused && paid == 0 && allocation.allocated == 0);

    let ((), refused, _, _) = with_limit_usage_refusal(usize::MAX, || {
        let mut current = Canonicalizer::new(&automaton, sink);
        let mut budget = ReplayBudget::new();
        current.closure(&function, roots[3], &mut budget).unwrap();
        let pointer = current.cursor.as_ptr();
        let capacity = current.cursor.capacity();
        let before = budget.remaining;
        let (((), allocation), refused, paid, first) =
            crate::bounded_output::with_limit_usage_refusal(usize::MAX, || {
                // Nested scopes are foreign too; successful reuse is measured
                // below inside the original scope without replacing its ledger.
                measure(|| {
                    assert!(current.closure(&function, roots[3], &mut budget).is_err());
                })
            });
        assert!(!refused && paid == 0 && first.is_none() && allocation.allocated == 0);
        assert_eq!(pointer, current.cursor.as_ptr());
        assert_eq!(capacity, current.cursor.capacity());
        assert_eq!(before, budget.remaining);
    });
    assert!(refused); // Restoring a bound scope cannot cure a foreign attempt.
}

#[test]
fn retained_closure_same_scope_cache_hit_has_zero_allocation_and_one_work_unit() {
    let function = fixture();
    let (automaton, sink, roots) = super::tests::automaton(&function);
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut current = Canonicalizer::new(&automaton, sink);
            let mut budget = ReplayBudget::new();
            current.closure(&function, roots[3], &mut budget).unwrap();
            let at = current_metrics().allocated;
            let before = budget.remaining;
            let capacity = current.cursor.capacity();
            let pointer = current.cursor.as_ptr();
            for _ in 0..16 {
                assert!(current
                    .closure(&function, roots[3], &mut budget)
                    .unwrap()
                    .is_some());
            }
            assert_eq!(budget.remaining, before - 16);
            assert_eq!(current_metrics().allocated, at);
            assert_eq!(current.cursor.capacity(), capacity);
            assert_eq!(current.cursor.as_ptr(), pointer);
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

#[test]
fn retained_canonical_physical_zero_exact_short_growth_and_all_exit_drops() {
    let function = fixture();
    let (automaton, sink, roots) = super::tests::automaton(&function);
    let observe = |limit, count| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut current = Canonicalizer::new(&automaton, sink);
                let mut budget = ReplayBudget::new();
                let mut success = true;
                for root in roots[..count].iter() {
                    // The test's input Vec has its own real prepaid backing.
                    if budget
                        .reserve_array::<(usize, u64)>(&function, 1, "scratch test input")
                        .is_err()
                    {
                        success = false;
                        break;
                    }
                    match current.id(&function, vec![(*root, 1)], &mut budget) {
                        Ok(Some(_)) => {}
                        other => {
                            drop(other);
                            success = false;
                            assert!(
                                current.frames.is_empty()
                                    && current.open.is_empty()
                                    && current.cursor.is_empty()
                            );
                            break;
                        }
                    }
                }
                drop(current);
                drop(budget);
                success
            })
        })
    };
    for count in 1..=6 {
        let ((success, allocation), refused, exact, first) = observe(usize::MAX, count);
        assert!(success && !refused && first.is_none());
        assert!(
            !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
        );
        assert!(allocation.allocated <= exact && allocation.highwater <= exact);
        for limit in [0, exact - 1, exact] {
            let ((success, allocation), refused, _, first) = observe(limit, count);
            assert_eq!(success, limit == exact);
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
fn retained_memo_foreign_function_budget_and_restoration_never_touch_foreign_payer() {
    let function = fixture();
    let wrong = function.clone();
    for foreign_function in [true, false] {
        let (((), allocation), refused, paid, _) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let mut foreign = ReplayBudget::new();
                let mut memo = memo::RetainedMemo::<usize>::new();
                memo.insert_retained(&function, 0, &mut budget, || 3)
                    .unwrap();
                let at = current_metrics().allocated;
                if foreign_function {
                    memo.authenticate_retained(&wrong, &mut budget);
                } else {
                    memo.authenticate_retained(&function, &mut foreign);
                }
                assert!(memo.failed());
                assert_eq!(memo.failure(), Diagnostic::io("SPX-H006", String::new()));
                memo.authenticate_retained(&function, &mut budget);
                assert!(memo.failed() && memo.get(&0).is_none());
                assert!(foreign.refusal_text.is_none() && !foreign.refusal_spent);
                assert_eq!(current_metrics().allocated, at);
                drop(memo);
                drop(foreign);
                drop(budget);
            })
        });
        assert!(!refused && paid > 0);
        assert!(
            !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
        );
    }
}

#[test]
fn retained_scratch_immediate_complete_originals_and_adaptation_are_exact() {
    let original = include_str!(
        "../../../../../docs/evidence/canonical-scratch-839/canonical_shared.rs.before.txt"
    );
    let start = original.find("use super::{").unwrap();
    let end = original.find("type SharedWeighted").unwrap();
    let mut expected = original[..start].to_owned();
    expected.push_str("use super::super::{\n    physical_clone, replay_error, Automaton, BTreeMap, BTreeSet, Diagnostic, Label, Rc,\n    ReplayBudget, ResolvedFunction, Weighted,\n};\nuse super::{app_census, epsilon_alias, successor_targets, tests};\n#[path = \"before_scratch_memo.rs\"]\nmod memo;\n\n");
    let body = &original[end..];
    let body = body.split("\n#[cfg(test)]\nmod tests;").next().unwrap();
    expected.push_str(&body.replacen("    fn closure(", "    pub(super) fn closure(", 1));
    expected.push_str("\n\nimpl Canonicalizer<'_> {\n    pub(super) fn signatures(&self) -> &BTreeMap<(u64, Vec<(Label, usize)>), usize> { &self.ids }\n    pub(super) fn states(&self) -> &BTreeMap<SharedWeighted, usize> { &self.states }\n}\n");
    assert_eq!(expected, include_str!("before_scratch.rs"));
    let memo =
        include_str!("../../../../../docs/evidence/canonical-scratch-839/memo.rs.before.txt");
    assert_eq!(
        memo.replace("\n#[cfg(test)]\nmod tests;\n", ""),
        include_str!("before_scratch_memo.rs")
    );
}

#[test]
fn retained_scratch_no_owner_route_never_bootstraps_active_zero_payer() {
    let function = fixture();
    let (automaton, sink, roots) = super::tests::automaton(&function);
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(0, || {
        measure(|| {
            let mut current = Canonicalizer::new(&automaton, sink);
            let mut budget = ReplayBudget::unobserved();
            assert!(current
                .id(&function, vec![(roots[0], 1)], &mut budget)
                .unwrap()
                .is_some());
            assert!(budget.refusal_text.is_none() && !budget.refusal_spent);
            assert!(
                current.frames.is_empty() && current.open.is_empty() && current.cursor.is_empty()
            );
            drop(current);
            drop(budget);
        })
    });
    assert!(!refused && paid == 0 && first.is_none());
    assert!(
        !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
    );
}

#[test]
fn retained_scratch_every_actual_byte_cut_clears_owners_before_sticky_retry() {
    let function = fixture();
    let (automaton, sink, roots) = super::tests::automaton(&function);
    let observe = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut current = Canonicalizer::new(&automaton, sink);
                let mut budget = ReplayBudget::new();
                // Input construction is outside the changed receiver's funding
                // claim, and remains included in observed actual allocations.
                let input = vec![(roots[3], 1)];
                let success = current.id(&function, input, &mut budget).is_ok();
                assert!(
                    current.frames.is_empty()
                        && current.open.is_empty()
                        && current.cursor.is_empty()
                );
                if !success {
                    let at = current_metrics().allocated;
                    assert!(current.closure(&function, roots[3], &mut budget).is_err());
                    assert_eq!(current_metrics().allocated, at);
                }
                drop(current);
                drop(budget);
                success
            })
        })
    };
    let ((success, _), refused, exact, first) = observe(usize::MAX);
    assert!(success && !refused && first.is_none());
    // This traverses every real control, cursor, frame, open backing and memo
    // growth refusal frontier, including its exact and one-byte-short edge.
    for limit in 0..=exact {
        let ((success, allocation), refused, _, first) = observe(limit);
        assert_eq!(success, limit == exact);
        assert_eq!(refused, limit != exact);
        assert_eq!(first.is_none(), limit == exact);
        assert!(
            !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
        );
    }
}

#[test]
fn retained_scratch_complete_old_current_actual_and_reserved_costs_are_separate() {
    let function = fixture();
    let (automaton, sink, roots) = super::tests::automaton(&function);
    let observe = |old| {
        with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                if old {
                    let mut receiver = before_scratch::Canonicalizer::new(&automaton, sink);
                    for root in roots[..6].iter().chain(roots[..6].iter()) {
                        budget
                            .reserve_array::<(usize, u64)>(&function, 1, "scratch comparison input")
                            .unwrap();
                        assert!(receiver
                            .id(&function, vec![(*root, 1)], &mut budget)
                            .unwrap()
                            .is_some());
                    }
                    drop(receiver);
                } else {
                    let mut receiver = Canonicalizer::new(&automaton, sink);
                    for root in roots[..6].iter().chain(roots[..6].iter()) {
                        budget
                            .reserve_array::<(usize, u64)>(&function, 1, "scratch comparison input")
                            .unwrap();
                        assert!(receiver
                            .id(&function, vec![(*root, 1)], &mut budget)
                            .unwrap()
                            .is_some());
                    }
                    drop(receiver);
                }
                budget.remaining
            })
        })
    };
    let old = observe(true);
    let current = observe(false);
    assert_eq!(old.0 .0, current.0 .0);
    for observation in [&old, &current] {
        assert!(!observation.1 && observation.3.is_none());
        let metrics = observation.0 .1;
        assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
        assert!(metrics.allocated <= observation.2 && metrics.highwater <= observation.2);
    }
    eprintln!(
        "scratch full-old reservation={} actual={:?}; current reservation={} actual={:?}",
        old.2, old.0 .1, current.2, current.0 .1
    );
}

#[test]
fn retained_cache_receiving_fee_preserves_floor_and_sticky_refusal_without_allocation() {
    immediate_old_floor::run();
}

mod immediate_old_floor {
pub(super) fn run() {
    retained_cache_receiving_fee_preserves_floor_and_sticky_refusal_without_allocation();
}
use super::*;
use super::super::before_held::Canonicalizer;
#[test]
fn retained_cache_receiving_fee_preserves_floor_and_sticky_refusal_without_allocation() {
    use crate::bounded_output::{
        active_remaining, clear_active_floor, reserve_active_required, set_active_floor,
    };
    let function = fixture();
    let (automaton, sink, roots) = super::tests::automaton(&function);
    for floor in [false, true] {
        let (((), allocation), refused, _, first) = with_limit_usage_refusal(1 << 20, || {
            measure(|| {
                let mut current = Canonicalizer::new(&automaton, sink);
                let mut budget = ReplayBudget::new();
                current.closure(&function, roots[3], &mut budget).unwrap();
                let remaining = active_remaining().unwrap();
                let receipt = std::mem::size_of::<Result<(), Diagnostic>>();
                if floor {
                    assert!(set_active_floor(remaining - receipt));
                } else {
                    assert!(reserve_active_required(remaining - receipt));
                }
                let at = current_metrics().allocated;
                assert!(current
                    .closure(&function, roots[3], &mut budget)
                    .unwrap()
                    .is_some());
                assert_eq!(current_metrics().allocated, at);
                assert!(!reserve_active_required(1));
                clear_active_floor();
                assert!(current.closure(&function, roots[3], &mut budget).is_err());
                assert_eq!(current_metrics().allocated, at);
                assert!(current.closures.failed());
                drop(current);
                drop(budget);
            })
        });
        assert!(refused);
        let first = first.unwrap();
        assert_eq!(first.requested, 1);
        assert_eq!(first.floor > 0, floor);
        assert!(
            !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
        );
    }
}

}

#[test]
fn retained_warm_authentication_pays_the_real_result_at_exact_and_one_short_frontiers() {
    use crate::bounded_output::{active_remaining, reserve_active_required};
    let function = fixture();
    let receipt = std::mem::size_of::<Result<(), Diagnostic>>();
    for available in [0, receipt - 1, receipt] {
        let (((), allocation), refused, _, first) = with_limit_usage_refusal(1 << 20, || {
            measure(|| {
                let mut memo = memo::RetainedMemo::<usize>::new();
                let mut budget = ReplayBudget::new();
                memo.authenticate_retained(&function, &mut budget);
                assert!(!memo.failed());
                let remaining = active_remaining().unwrap();
                assert!(reserve_active_required(remaining - available));
                let at = current_metrics().allocated;
                memo.authenticate_retained(&function, &mut budget);
                assert_eq!(memo.failed(), available != receipt);
                assert_eq!(current_metrics().allocated, at);
                if available == receipt {
                    assert_eq!(active_remaining(), Some(0));
                } else {
                    assert_eq!(memo.failure().code, "SPX-H006");
                    memo.authenticate_retained(&function, &mut budget);
                    assert!(memo.failed());
                    assert_eq!(current_metrics().allocated, at);
                }
                drop(memo);
                drop(budget);
            })
        });
        assert_eq!(refused, available != receipt);
        if available == receipt {
            assert!(first.is_none());
        } else {
            let first = first.unwrap();
            assert_eq!(first.requested, receipt);
            assert_eq!(first.remaining, available);
        }
        assert!(
            !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
        );
    }
    // Calling the ordinary authenticator above retains its exact source fee;
    // its complete implementation has no hidden replacement or weakened debit.
    let ordinary = include_str!("memo.rs").replace(
        "    #[cfg(test)]\n    pub(super) fn insert(",
        "    pub(super) fn insert(",
    );
    let ordinary = ordinary
        .strip_suffix("\nmod retained;\npub(super) use retained::RetainedMemo;\n")
        .unwrap();
    assert_eq!(
        ordinary,
        include_str!("../../../../../docs/evidence/canonical-scratch-839/memo.rs.before.txt")
    );
}
