//! Direct production-slot controls. Single-request compatibility wrappers are
//! deliberately excluded from the receiving-cost observation below.
use super::super::{before_closure_receiver, Canonicalizer};
use super::*;
use crate::bounded_output::{active_remaining, with_limit_usage_refusal};
use crate::test_allocation_observer::{current_metrics, measure};

fn fixture() -> ResolvedFunction {
    super::super::super::lexical_plan_tests::fixture(super::super::super::lexical_plan_tests::WHILE)
        .1
}

#[test]
fn held_closure_full_old_ids_six_fields_work_order_overflow_and_cycles() {
    let function = fixture();
    let (automaton, sink, roots) = super::super::tests::automaton(&function);
    let mut probe = ReplayBudget::unobserved();
    let start = probe.remaining;
    before_closure_receiver::Canonicalizer::new(&automaton, sink)
        .id(&function, vec![(roots[4], 1)], &mut probe)
        .unwrap();
    for units in 0..=start - probe.remaining {
        for (root, weight) in [(roots[4], 1), (roots[3], u64::MAX), (roots[6], 1)] {
            let mut old = before_closure_receiver::Canonicalizer::new(&automaton, sink);
            let mut current = Canonicalizer::new(&automaton, sink);
            let mut a = ReplayBudget::unobserved();
            let mut b = ReplayBudget::unobserved();
            let mut receiving = ClosureReceiving::acquire(&function, &mut b).unwrap();
            a.remaining = units;
            b.remaining = units;
            // Diagnostic Eq includes code, severity, message, path, span and help.
            assert_eq!(
                old.id(&function, vec![(root, weight)], &mut a),
                current.id_with_receiving(&function, vec![(root, weight)], &mut b, &mut receiving)
            );
            assert_eq!(
                (a.remaining, a.skeleton_remaining),
                (b.remaining, b.skeleton_remaining)
            );
            assert_eq!(old.signatures(), &current.ids);
            assert_eq!(old.states().len(), current.states.len());
            for ((a, id_a), (b, id_b)) in old.states().iter().zip(current.states.iter()) {
                assert_eq!(a.as_slice(), b.as_slice());
                assert_eq!(id_a, id_b);
            }
            assert!(!receiving.ready && receiving.value.is_none() && receiving.refusal.is_none());
            assert!(
                current.cursor.is_empty() && current.frames.is_empty() && current.open.is_empty()
            );
        }
    }
}

#[test]
fn held_closure_two_roots_reuse_one_owner_and_count_actual_inner_misses() {
    let function = fixture();
    let (automaton, sink, roots) = super::super::tests::automaton(&function);
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(1 << 24, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            let mut current = Canonicalizer::new(&automaton, sink);
            let mut receiving = ClosureReceiving::acquire(&function, &mut budget).unwrap();
            let owner = std::ptr::from_ref(&receiving);
            let before = producer_count();
            drop(
                current
                    .closure_with_receiving(&function, roots[3], &mut budget, &mut receiving)
                    .unwrap(),
            );
            assert_eq!(producer_count() - before, 1);
            // One unit invocation filled multiple memo entries: frequency is the
            // producer boundary, never the number of inserted entries.
            assert!(!current.closures.contains_key(&sink));
            assert!(current.closures.contains_key(&roots[0]));
            assert!(current.closures.contains_key(&roots[3]));
            // The initial closure follows epsilon edges, not roots[0]'s move
            // to sink. Query that still-cold sink explicitly, then prove reuse.
            let count = producer_count();
            let value = current
                .closure_with_receiving(&function, sink, &mut budget, &mut receiving)
                .unwrap()
                .unwrap();
            assert_eq!(producer_count(), count + 1);
            assert_eq!(value.len(), 1);
            assert_eq!(value.get(&sink), Some(&1));
            drop(value);
            let work = budget.remaining;
            let allocated = current_metrics().allocated;
            drop(
                current
                    .closure_with_receiving(&function, sink, &mut budget, &mut receiving)
                    .unwrap(),
            );
            assert_eq!(producer_count(), count + 1);
            assert_eq!(budget.remaining, work - 1);
            assert_eq!(current_metrics().allocated, allocated);
            for root in [roots[4], roots[5]] {
                budget
                    .reserve_array::<(usize, u64)>(&function, 1, "held closure test root")
                    .unwrap();
                assert!(current
                    .id_with_receiving(&function, vec![(root, 1)], &mut budget, &mut receiving)
                    .unwrap()
                    .is_some());
                assert_eq!(owner, std::ptr::from_ref(&receiving));
            }
            // LoopLatch is disconnected from both comparison roots. A fresh
            // query must invoke the producer once; its retry must invoke none.
            assert!(!current.closures.contains_key(&roots[2]));
            let count = producer_count();
            let value = current
                .closure_with_receiving(&function, roots[2], &mut budget, &mut receiving)
                .unwrap()
                .unwrap();
            assert_eq!(producer_count(), count + 1);
            assert_eq!(value.len(), 1);
            assert_eq!(value.get(&roots[2]), Some(&1));
            drop(value);
            let work = budget.remaining;
            let allocated = current_metrics().allocated;
            drop(
                current
                    .closure_with_receiving(&function, roots[2], &mut budget, &mut receiving)
                    .unwrap(),
            );
            assert_eq!(producer_count(), count + 1);
            assert_eq!(budget.remaining, work - 1);
            assert_eq!(current_metrics().allocated, allocated);
            let count = producer_count();
            let work = budget.remaining;
            let allocated = current_metrics().allocated;
            for _ in 0..8 {
                drop(
                    current
                        .closure_with_receiving(&function, roots[3], &mut budget, &mut receiving)
                        .unwrap(),
                );
            }
            assert_eq!(producer_count(), count);
            assert_eq!(budget.remaining, work - 8);
            assert_eq!(current_metrics().allocated, allocated);
            drop(current);
            drop(receiving);
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
fn held_closure_bootstrap_pays_actual_controls_at_exact_and_one_short() {
    let function = fixture();
    let expected = 2 * size_of::<ClosureReceiving>()
        + size_of::<Result<ClosureReceiving, Diagnostic>>()
        + size_of::<Option<Diagnostic>>()
        + size_of::<Diagnostic>()
        + size_of::<Value>()
        + size_of::<Result<(), Diagnostic>>();
    assert_eq!(ClosureReceiving::CONTROLS, expected);
    let baseline = ReplayBudget::with_physical(&function, 1 << 20).unwrap();
    let admission = baseline.physical.as_ref().unwrap().charged();
    drop(baseline);
    for available in [0, expected - 1, expected] {
        let mut budget = ReplayBudget::with_physical(&function, admission + available).unwrap();
        let (success, allocation) = measure(|| {
            let result = ClosureReceiving::acquire(&function, &mut budget);
            let success = result.is_ok();
            drop(result);
            drop(budget);
            success
        });
        assert_eq!(success, available == expected);
        assert!(
            !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
        );
    }
}

#[test]
fn held_closure_production_sequence_exact_short_and_whole_drop_have_separate_old_domain() {
    let function = fixture();
    let (automaton, sink, roots) = super::super::tests::automaton(&function);
    let observe = |old: bool, limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let success = if old {
                    let mut canonical =
                        before_closure_receiver::Canonicalizer::new(&automaton, sink);
                    let result = (|| {
                        for root in [roots[4], roots[5]] {
                            budget.reserve_array::<(usize, u64)>(
                                &function,
                                1,
                                "held sequence root",
                            )?;
                            canonical.id(&function, vec![(root, 1)], &mut budget)?;
                        }
                        Ok::<_, Diagnostic>(())
                    })();
                    let ok = result.is_ok();
                    drop(result);
                    drop(canonical);
                    ok
                } else {
                    let mut canonical = Canonicalizer::new(&automaton, sink);
                    let result = (|| {
                        let mut receiving = ClosureReceiving::acquire(&function, &mut budget)?;
                        for root in [roots[4], roots[5]] {
                            budget.reserve_array::<(usize, u64)>(
                                &function,
                                1,
                                "held sequence root",
                            )?;
                            canonical.id_with_receiving(
                                &function,
                                vec![(root, 1)],
                                &mut budget,
                                &mut receiving,
                            )?;
                        }
                        Ok::<_, Diagnostic>(())
                    })();
                    let ok = result.is_ok();
                    drop(result);
                    drop(canonical);
                    ok
                };
                drop(budget);
                success
            })
        })
    };
    // Each real receiver establishes its own exact charged frontier. There is
    // no allowance, padding, equality-of-fees or claimed reduction here.
    for old in [true, false] {
        let ((success, allocation), refused, exact, first) = observe(old, 1 << 24);
        assert!(success && !refused && first.is_none());
        assert!(
            !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
        );
        assert!(allocation.allocated <= exact && allocation.highwater <= exact);
        for limit in [0, exact - 1, exact] {
            let ((success, allocation), refused, _, _) = observe(old, limit);
            assert_eq!(success, limit == exact);
            assert_eq!(refused, limit != exact);
            assert!(
                !allocation.invalid
                    && allocation.live == 0
                    && allocation.allocated == allocation.freed
            );
        }
    }
}

#[test]
fn held_closure_failure_publication_moves_all_six_fields_without_success_transport() {
    let function = fixture();
    let mut budget = ReplayBudget::unobserved();
    let mut receiving = ClosureReceiving::acquire(&function, &mut budget).unwrap();
    let error = Diagnostic {
        code: "SPX-H006",
        severity: crate::diagnostic::Severity::Warning,
        message: std::borrow::Cow::Owned("selected first failure".to_owned()),
        path: Some("held.spx".to_owned()),
        span: Some(crate::ast::Span {
            start: 3,
            end: 9,
            line: 2,
            column: 4,
        }),
        help: Some("retained help".to_owned()),
    };
    let expected = error.clone();
    let pointer = error.message.as_ptr();
    let ((), allocation) = measure(|| {
        receiving.begin();
        receiving.refuse(error);
        assert!(receiving.ready);
        receiving.ready = false;
        assert!(receiving.refusal.is_some());
        let published = receiving.refusal.take().unwrap();
        assert_eq!(published, expected);
        assert_eq!(published.message.as_ptr(), pointer);
        assert!(receiving.value.is_none());
        drop(published);
        drop(receiving);
    });
    assert!(!allocation.invalid && allocation.allocated == 0 && allocation.live == 0);
}

#[test]
fn held_closure_foreign_function_budget_nested_and_expired_slot_are_sticky_without_query() {
    let function = fixture();
    let foreign = fixture();
    for kind in 0..3 {
        let (((), allocation), refused, _, first) = with_limit_usage_refusal(1 << 20, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let mut other = ReplayBudget::new();
                let mut receiving = ClosureReceiving::acquire(&function, &mut budget).unwrap();
                if kind == 2 {
                    let ((), refused, used, first) = with_limit_usage_refusal(0, || {
                        let at = current_metrics().allocated;
                        assert!(!receiving.valid(&function, &budget));
                        assert_eq!(current_metrics().allocated, at);
                    });
                    assert!(!refused && used == 0 && first.is_none());
                } else {
                    let left = active_remaining();
                    if kind == 0 {
                        assert!(!receiving.valid(&foreign, &budget));
                    } else {
                        assert!(!receiving.valid(&function, &other));
                    }
                    assert_eq!(active_remaining(), left);
                }
                let left = active_remaining();
                assert!(!receiving.valid(&function, &budget));
                assert_eq!(active_remaining(), left);
                // Keep the second budget at a stable distinct address throughout.
                assert_eq!(other.remaining, budget.remaining);
                let _ = &mut other;
            })
        });
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
        assert_eq!(refused, kind == 2);
        assert!(first.is_none() || kind == 2);
    }
    let ((), allocation) = measure(|| {
        let mut budget = ReplayBudget::new();
        let (mut expired, _, _, _) = with_limit_usage_refusal(1 << 20, || {
            ClosureReceiving::acquire(&function, &mut budget).unwrap()
        });
        assert!(!expired.valid(&function, &budget));
        assert!(!expired.valid(&function, &budget));
        drop(expired);
        drop(budget);
    });
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
}

#[test]
fn held_closure_arithmetic_none_is_not_a_refusal_and_slot_can_receive_again() {
    let function = fixture();
    let mut budget = ReplayBudget::unobserved();
    let mut automaton = super::super::super::Automaton::default();
    let sink = automaton.node(&function, &mut budget).unwrap();
    let mut root = sink;
    for _ in 0..64 {
        let next = automaton.node(&function, &mut budget).unwrap();
        automaton
            .epsilon(&function, &mut budget, next, root)
            .unwrap();
        automaton
            .epsilon(&function, &mut budget, next, root)
            .unwrap();
        root = next;
    }
    let mut old = before_closure_receiver::Canonicalizer::new(&automaton, sink);
    let mut current = Canonicalizer::new(&automaton, sink);
    let mut a = ReplayBudget::unobserved();
    let mut b = ReplayBudget::unobserved();
    let mut receiving = ClosureReceiving::acquire(&function, &mut b).unwrap();
    assert!(old.closure(&function, root, &mut a).unwrap().is_none());
    assert!(current
        .closure_with_receiving(&function, root, &mut b, &mut receiving)
        .unwrap()
        .is_none());
    assert_eq!(a.remaining, b.remaining);
    assert!(!receiving.failed && !receiving.ready && receiving.refusal.is_none());
    assert!(current
        .closure_with_receiving(&function, sink, &mut b, &mut receiving)
        .unwrap()
        .is_some());
}

#[test]
fn held_closure_complete_old_invocation_and_current_caller_keep_checked_verdict_and_work() {
    let (program, function) = super::super::super::lexical_plan_tests::fixture(
        super::super::super::lexical_plan_tests::WHILE,
    );
    let mut a = ReplayBudget::with_skeleton_limit(super::super::super::MAX_REPLAY_WORK_UNITS);
    let mut b = ReplayBudget::with_skeleton_limit(super::super::super::MAX_REPLAY_WORK_UNITS);
    assert_eq!(
        super::super::super::before_closure_invocation::compare_with_index_storage(
            &program, &function, &mut a, None, None
        ),
        super::super::super::compare_with_index_storage(&program, &function, &mut b, None, None)
    );
    assert_eq!(
        (a.remaining, a.skeleton_remaining),
        (b.remaining, b.skeleton_remaining)
    );
}

#[test]
fn held_closure_actual_entries_hide_cached_views_after_foreign_and_restored_slots() {
    let function = fixture();
    let foreign_function = fixture();
    let (automaton, sink, roots) = super::super::tests::automaton(&function);
    for id_entry in [false, true] {
        // Foreign function, foreign budget, nested current ledger, expired slot.
        for kind in 0..4 {
            let ((), allocation) = measure(|| {
                let ((), refused, _, first) = with_limit_usage_refusal(1 << 24, || {
                    let mut budget = ReplayBudget::new();
                    let mut foreign_budget = ReplayBudget::new();
                    let mut current = Canonicalizer::new(&automaton, sink);
                    let mut receiving = ClosureReceiving::acquire(&function, &mut budget).unwrap();
                    // This unused replacement already belongs to the original
                    // live ledger; the attack must invalidate the Memo itself.
                    let mut fresh = ClosureReceiving::acquire(&function, &mut budget).unwrap();
                    if id_entry {
                        budget
                            .reserve_array::<(usize, u64)>(&function, 1, "sticky id prime")
                            .unwrap();
                        assert!(current
                            .id_with_receiving(
                                &function,
                                vec![(roots[0], 1)],
                                &mut budget,
                                &mut receiving
                            )
                            .unwrap()
                            .is_some());
                        assert!(!current.states.is_empty());
                    } else {
                        assert!(current
                            .closure_with_receiving(
                                &function,
                                roots[0],
                                &mut budget,
                                &mut receiving
                            )
                            .unwrap()
                            .is_some());
                    }
                    assert!(current.closures.get(&roots[0]).is_some());
                    if kind == 3 {
                        let (stale, refused, _, first) = with_limit_usage_refusal(1 << 20, || {
                            ClosureReceiving::acquire(&function, &mut budget).unwrap()
                        });
                        assert!(!refused && first.is_none());
                        receiving = stale;
                    }
                    // Every owned test argument exists before the no-allocation
                    // boundary, including arguments rejected before id_inner.
                    let mut inputs = [
                        vec![(roots[0], 1)],
                        vec![(roots[0], 1)],
                        vec![(roots[0], 1)],
                    ]
                    .into_iter();
                    macro_rules! invoke {
                        ($function:expr,$budget:expr,$slot:expr) => {
                            if id_entry {
                                current
                                    .id_with_receiving(
                                        $function,
                                        inputs.next().unwrap(),
                                        $budget,
                                        $slot,
                                    )
                                    .map(|_| ())
                            } else {
                                current
                                    .closure_with_receiving($function, roots[0], $budget, $slot)
                                    .map(|_| ())
                            }
                        };
                    }
                    let expected = Diagnostic::io("SPX-H006", String::new());
                    let work = (budget.remaining, budget.skeleton_remaining);
                    let foreign_work =
                        (foreign_budget.remaining, foreign_budget.skeleton_remaining);
                    let at = current_metrics().allocated;
                    if kind == 2 {
                        let ((), refused, used, first) = with_limit_usage_refusal(0, || {
                            let inner_at = current_metrics().allocated;
                            assert_eq!(
                                invoke!(&function, &mut budget, &mut receiving).unwrap_err(),
                                expected
                            );
                            assert_eq!(current_metrics().allocated, inner_at);
                        });
                        assert!(!refused && used == 0 && first.is_none());
                    } else {
                        let left = active_remaining();
                        if kind == 0 {
                            assert_eq!(
                                invoke!(&foreign_function, &mut budget, &mut receiving)
                                    .unwrap_err(),
                                expected
                            );
                        } else if kind == 1 {
                            assert_eq!(
                                invoke!(&function, &mut foreign_budget, &mut receiving)
                                    .unwrap_err(),
                                expected
                            );
                        } else {
                            assert_eq!(
                                invoke!(&function, &mut budget, &mut receiving).unwrap_err(),
                                expected
                            );
                        }
                        assert_eq!(active_remaining(), left);
                        assert_eq!(current_metrics().allocated, at);
                    }
                    assert!(current.closures.failed());
                    assert!(current.closures.get(&roots[0]).is_none());
                    assert_eq!(
                        invoke!(&function, &mut budget, &mut receiving).unwrap_err(),
                        expected
                    );
                    // Even a distinct, correctly bound destination cannot cure
                    // the original canonical receiver after the rejected call.
                    assert!(fresh.valid(&function, &budget));
                    let left = active_remaining();
                    let retry_at = current_metrics().allocated;
                    assert_eq!(
                        invoke!(&function, &mut budget, &mut fresh).unwrap_err(),
                        expected
                    );
                    assert!(current.closures.get(&roots[0]).is_none());
                    assert_eq!((budget.remaining, budget.skeleton_remaining), work);
                    assert_eq!(
                        (foreign_budget.remaining, foreign_budget.skeleton_remaining),
                        foreign_work
                    );
                    assert_eq!(active_remaining(), left);
                    assert_eq!(current_metrics().allocated, retry_at);
                    drop(inputs);
                    drop(current);
                    drop(receiving);
                    drop(fresh);
                    drop(budget);
                    drop(foreign_budget);
                });
                // A foreign nested ledger selects sticky refusal on its held
                // outer owner, while the nested payer itself is never queried.
                assert_eq!(refused, kind == 2);
                assert!(first.is_none() || kind == 2);
            });
            assert!(
                !allocation.invalid
                    && allocation.live == 0
                    && allocation.allocated == allocation.freed
            );
        }
    }
}
