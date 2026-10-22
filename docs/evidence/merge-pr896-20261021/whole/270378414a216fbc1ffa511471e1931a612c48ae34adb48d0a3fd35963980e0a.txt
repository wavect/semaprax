#[test]
fn one_pass_labels_preserve_original_subset_work_and_all_paid_cuts_through_drop() {
    use crate::bounded_output::with_limit_usage_refusal;
    use crate::test_allocation_observer::measure;
    for width in 0..=3 {
        let (function, mut graph, closed) = direct_fixture(width, 1, 7);
        if width > 1 {
            let mut setup = ReplayBudget::unobserved();
            graph
                .movement(
                    &function,
                    &mut setup,
                    1,
                    Label::End(super::super::SkeletonTerminal::Failure),
                    0,
                )
                .unwrap();
        }
        let original_work: usize = closed
            .iter()
            .map(|(node, _)| graph.nodes[*node].moves().len().saturating_add(1))
            .sum();
        let expected = {
            let mut old = ReplayBudget::unobserved();
            successors(&function, &mut old, &graph, &closed, 0)
        };
        let observe = |limit| {
            with_limit_usage_refusal(limit, || {
                measure(|| {
                    let mut budget = ReplayBudget::new();
                    let before = budget.remaining;
                    let result = budget
                        .reserve_bytes(
                            &function,
                            one_pass_receiving(),
                            "factored one-pass test call receivers",
                        )
                        .and_then(|()| {
                            one_pass_successors(&function, &mut budget, &graph, &closed, 0)
                        });
                    match result {
                        Ok(actual) => {
                            same_successors(&Ok(actual), &expected);
                            assert_eq!(before - budget.remaining, original_work);
                            drop(budget);
                            true
                        }
                        Err(error) => {
                            assert_eq!(error.code, "SPX-H006");
                            let selected = (
                                &error.code,
                                &error.severity,
                                &error.message,
                                &error.span,
                                &error.path,
                                &error.help,
                            );
                            let remaining = budget.remaining;
                            let physical_remaining = crate::bounded_output::active_remaining();
                            let allocated =
                                crate::test_allocation_observer::current_metrics().allocated;
                            let ledger =
                                crate::bounded_output::classifier_string::ClassifierLedger::capture(
                                );
                            let first = ledger.first_refusal();
                            let retry = budget
                                .reserve_bytes(&function, 0, "one-pass sticky retry")
                                .unwrap_err();
                            assert_eq!(
                                selected,
                                (
                                    &error.code,
                                    &error.severity,
                                    &error.message,
                                    &error.span,
                                    &error.path,
                                    &error.help
                                )
                            );
                            let empty = Diagnostic::io("SPX-H006", String::new());
                            assert_eq!(
                                (
                                    &retry.code,
                                    &retry.severity,
                                    &retry.message,
                                    &retry.span,
                                    &retry.path,
                                    &retry.help
                                ),
                                (
                                    &empty.code,
                                    &empty.severity,
                                    &empty.message,
                                    &empty.span,
                                    &empty.path,
                                    &empty.help
                                )
                            );
                            assert_eq!(ledger.first_refusal(), first);
                            assert_eq!(
                                crate::bounded_output::active_remaining(),
                                physical_remaining
                            );
                            assert_eq!(
                                crate::test_allocation_observer::current_metrics().allocated,
                                allocated
                            );
                            assert_eq!(budget.remaining, remaining);
                            drop(empty);
                            drop(ledger);
                            drop(retry);
                            drop(error);
                            drop(budget);
                            false
                        }
                    }
                })
            })
        };
        let ((ok, metrics), refused, exact, first) = observe(usize::MAX);
        assert!(ok && !refused && first.is_none());
        assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
        for limit in 0..=exact {
            let ((ok, metrics), refused, _, first) = observe(limit);
            assert_eq!(ok, limit == exact);
            assert_eq!(refused, limit != exact);
            assert_eq!(first.is_none(), limit == exact);
            assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
        }
    }
}

#[test]
fn one_pass_real_composition_repetition_promotion_content_labels_and_all_cuts_keep_full_results() {
    use crate::bounded_output::{with_limit, with_limit_usage_refusal};
    use crate::test_allocation_observer::measure;
    for case in 0..7 {
        let function =
            super::super::lexical_plan_tests::fixture(super::super::lexical_plan_tests::WHILE).1;
        let mut setup = ReplayBudget::unobserved();
        let mut graph = Automaton::default();
        for _ in 0..5 {
            graph.node(&function, &mut setup).unwrap();
        }
        let a = Label::End(super::super::SkeletonTerminal::Success);
        let b = Label::End(super::super::SkeletonTerminal::Failure);
        let c = Label::End(super::super::SkeletonTerminal::LoopLatch);
        let mut edges = match case {
            0 => Vec::new(),
            1 | 2 => vec![(a.clone(), 2), (a.clone(), 2)],
            3 => vec![
                (a.clone(), 2),
                (a.clone(), 3),
                (a.clone(), 3),
                (a.clone(), 4),
            ],
            4 => vec![(b.clone(), 3), (a.clone(), 2)],
            5 => vec![(c, 4), (b, 3), (a, 2)],
            _ => {
                let first = Rc::new(super::super::super::SkeletonObservation::Boolean {
                    expression: function.body.id.clone(),
                    value: true,
                });
                let second = Rc::new((*first).clone());
                assert!(!Rc::ptr_eq(&first, &second));
                assert_eq!(first, second);
                vec![
                    (Label::Observation(first), 2),
                    (Label::Observation(second), 2),
                ]
            }
        };
        for (label, child) in edges.drain(..) {
            graph
                .movement(&function, &mut setup, 1, label, child)
                .unwrap();
        }
        let mut closed = ordered_weights::OrderedWeights::new();
        closed.insert_prepaid(1, if case == 2 { u64::MAX } else { 7 });
        let original_subset = graph.nodes[1].moves().len().saturating_add(1);
        // Measure real delegated helper work independently, rather than assume
        // a constant or subtract a guessed allowance from the new producer.
        let (delegated, delegated_refused) = if case == 3 {
            with_limit(usize::MAX, || {
                let mut budget = ReplayBudget::new();
                let before = budget.remaining;
                budget
                    .reserve_bytes(
                        &function,
                        paid_many::PROMOTE_RECEIVING,
                        "one-pass delegation witness",
                    )
                    .unwrap();
                let paid_many::Targets::PaidMany(mut targets) =
                    paid_many::promote(&function, &mut budget, 2, 7, 3, 7).unwrap()
                else {
                    panic!("real paid promotion")
                };
                for child in [3, 4] {
                    budget
                        .reserve_bytes(
                            &function,
                            paid_many::ADD_RECEIVING,
                            "one-pass delegation witness",
                        )
                        .unwrap();
                    assert!(
                        paid_many::add(&function, &mut budget, &mut targets, child, 7).unwrap()
                    );
                }
                let spent = before - budget.remaining;
                drop(targets);
                drop(budget);
                spent
            })
        } else {
            (0, false)
        };
        assert!(!delegated_refused);
        let (expected, reference_refused) = with_limit(usize::MAX, || {
            let mut budget = ReplayBudget::new();
            successors(&function, &mut budget, &graph, &closed, 11)
        });
        assert!(!reference_refused);
        let observe = |limit| {
            with_limit_usage_refusal(limit, || {
                measure(|| {
                    let mut budget = ReplayBudget::new();
                    let before = budget.remaining;
                    let result = budget
                        .reserve_bytes(
                            &function,
                            one_pass_receiving(),
                            "one-pass composition receiving",
                        )
                        .and_then(|()| {
                            one_pass_successors(&function, &mut budget, &graph, &closed, 11)
                        });
                    match result {
                        Ok(actual) => {
                            same_successors(&Ok(actual), &expected);
                            assert_eq!(before - budget.remaining, original_subset + delegated);
                            drop(budget);
                            true
                        }
                        Err(error) => {
                            assert_eq!(error.code, "SPX-H006");
                            let selected = (
                                &error.code,
                                &error.severity,
                                &error.message,
                                &error.span,
                                &error.path,
                                &error.help,
                            );
                            let remaining = budget.remaining;
                            let physical_remaining = crate::bounded_output::active_remaining();
                            let allocated =
                                crate::test_allocation_observer::current_metrics().allocated;
                            let ledger =
                                crate::bounded_output::classifier_string::ClassifierLedger::capture(
                                );
                            let first = ledger.first_refusal();
                            let retry = budget
                                .reserve_bytes(&function, 0, "one-pass composition retry")
                                .unwrap_err();
                            assert_eq!(
                                selected,
                                (
                                    &error.code,
                                    &error.severity,
                                    &error.message,
                                    &error.span,
                                    &error.path,
                                    &error.help
                                )
                            );
                            let empty = Diagnostic::io("SPX-H006", String::new());
                            assert_eq!(
                                (
                                    &retry.code,
                                    &retry.severity,
                                    &retry.message,
                                    &retry.span,
                                    &retry.path,
                                    &retry.help
                                ),
                                (
                                    &empty.code,
                                    &empty.severity,
                                    &empty.message,
                                    &empty.span,
                                    &empty.path,
                                    &empty.help
                                )
                            );
                            assert_eq!(ledger.first_refusal(), first);
                            assert_eq!(
                                crate::bounded_output::active_remaining(),
                                physical_remaining
                            );
                            assert_eq!(
                                crate::test_allocation_observer::current_metrics().allocated,
                                allocated
                            );
                            assert_eq!(budget.remaining, remaining);
                            drop(empty);
                            drop(ledger);
                            drop(retry);
                            drop(error);
                            drop(budget);
                            false
                        }
                    }
                })
            })
        };
        let ((ok, metrics), refused, exact, first) = observe(usize::MAX);
        assert!(ok && !refused && first.is_none());
        assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
        for limit in 0..=exact {
            let ((ok, metrics), refused, _, first) = observe(limit);
            assert_eq!(ok, limit == exact);
            assert_eq!(refused, limit != exact);
            assert_eq!(first.is_none(), limit == exact);
            assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
        }
    }
}
