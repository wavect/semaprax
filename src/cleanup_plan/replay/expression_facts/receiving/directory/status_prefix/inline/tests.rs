use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;
fn function(keys: &[(&str, StatusLane)]) -> ResolvedFunction {
    let mut program = crate::hir::resolve(&crate::check(
        "module prefix.status; @id(\"status.f\") fn f(x:i64)->i64{x+1} @id(\"status.main\") fn main()->i64{0}",
        "status-prefix.spx").unwrap()).unwrap();
    let index = program
        .functions
        .iter()
        .position(|f| f.id.as_str() == "status.f")
        .unwrap();
    let mut function = program.functions.swap_remove(index);
    function.cleanup_plan.status_sources = keys
        .iter()
        .map(|(key, lane)| StatusSource {
            id: StatusSourceId {
                expression: ExpressionId::from_owned((*key).to_owned()),
                lane: *lane,
            },
            producer: StatusProducer::ContractFalse {
                phase: crate::cleanup_plan::ContractPhase::Requires,
                ordinal: 0,
            },
        })
        .collect();
    function
}
#[test]
fn inline_receipts_keep_every_old_link_key_duplicate_and_original_work() {
    let names = (0..512)
        .map(|i| format!("opaque.λ\0.{:04}", i * 197 % 512))
        .collect::<Vec<_>>();
    let mut keys = names
        .iter()
        .map(|n| (n.as_str(), StatusLane::OperationFailure))
        .collect::<Vec<_>>();
    keys.extend_from_within(..8);
    let function = function(&keys);
    let (_, overflow, _, first) = with_limit_usage_refusal(usize::MAX, || {
        let mut old_budget = ReplayBudget::new();
        let mut budget = ReplayBudget::new();
        let mut old = Directory::new(&function, &mut old_budget, "inline parity").unwrap();
        let mut current = Directory::new(&function, &mut budget, "inline parity").unwrap();
        let mut old = old.held_prefix(&function, &mut old_budget).unwrap();
        let mut current = current.held_prefix(&function, &mut budget).unwrap();
        current.fund_failure_cell(&function, &mut budget).unwrap();
        let mut failure = None;
        for source in &function.cleanup_plan.status_sources {
            let expected = old.insert(&function, &source.id, &mut old_budget).unwrap();
            let inserted = current.insert_inline(&function, &source.id, &mut budget, &mut failure);
            assert!(failure.is_none());
            assert_eq!(inserted, expected);
            assert_eq!(
                (old.directory.root, old.directory.first),
                (current.directory.root, current.directory.first)
            );
            assert!(old.directory.balanced() && current.directory.balanced());
            assert!(old.directory.keys().eq(current.directory.keys()));
            for (a, b) in old.directory.nodes.iter().zip(&current.directory.nodes) {
                assert!(std::ptr::eq(a.key, b.key));
                assert_eq!(
                    (a.left, a.right, a.parent, a.next, a.height),
                    (b.left, b.right, b.parent, b.next, b.height)
                );
            }
        }
        assert_eq!(old_budget.remaining - budget.remaining, 3 * keys.len() + 6);
    });
    assert!(!overflow && first.is_none());
}
#[test]
fn inline_receipts_pay_the_complete_cell_and_reduce_actual_cumulative_reservations() {
    let names = (0..512)
        .map(|i| format!("r.{i:04}.opaque"))
        .collect::<Vec<_>>();
    let keys = names
        .iter()
        .map(|n| (n.as_str(), StatusLane::ContractFalse))
        .collect::<Vec<_>>();
    let function = function(&keys);
    let run = |inline: bool| {
        with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let mut ids = Directory::new(&function, &mut budget, "inline accounting").unwrap();
                let mut held = ids.held_prefix(&function, &mut budget).unwrap();
                if inline {
                    let before = crate::bounded_output::active_remaining().unwrap();
                    held.fund_failure_cell(&function, &mut budget).unwrap();
                    let paid = before - crate::bounded_output::active_remaining().unwrap();
                    assert!(
                        paid >= 2 * size_of::<Option<Diagnostic>>()
                            + 2 * size_of::<Result<(), Diagnostic>>()
                    );
                }
                let mut failure = None;
                for source in &function.cleanup_plan.status_sources {
                    if inline {
                        assert!(held.insert_inline(
                            &function,
                            &source.id,
                            &mut budget,
                            &mut failure
                        ));
                        assert!(failure.is_none());
                    } else {
                        assert!(held.insert(&function, &source.id, &mut budget).unwrap());
                    }
                }
                drop(held);
                assert!(ids.balanced());
                assert_eq!(ids.len(), 512);
            })
        })
    };
    let (((), old), old_refused, old_paid, old_first) = run(false);
    let (((), new), new_refused, new_paid, new_first) = run(true);
    assert!(!old_refused && !new_refused && old_first.is_none() && new_first.is_none());
    assert!(old_paid - new_paid > 512 * 200);
    for (metrics, paid) in [(old, old_paid), (new, new_paid)] {
        assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
        assert!(metrics.allocated <= paid && metrics.highwater <= paid);
    }
    eprintln!(
        "inline-status complete512 old={old_paid} new={new_paid} saved={}",
        old_paid - new_paid
    );
}
#[test]
fn inline_receipts_keep_exact_short_zero_floor_and_sticky_drop_frontiers() {
    let names = (0..256).map(|i| format!("k.{i:04}")).collect::<Vec<_>>();
    let keys = names
        .iter()
        .map(|n| (n.as_str(), StatusLane::OperationFailure))
        .collect::<Vec<_>>();
    let function = function(&keys);
    let run = |available, floor| {
        with_limit_usage_refusal(available + floor, || {
            assert!(crate::bounded_output::set_active_floor(floor));
            measure(|| {
                let mut budget = ReplayBudget::new();
                let result = (|| {
                    let mut ids = Directory::new(&function, &mut budget, "inline frontier")?;
                    let mut held = ids.held_prefix(&function, &mut budget)?;
                    held.fund_failure_cell(&function, &mut budget)?;
                    let mut failure = None;
                    for source in &function.cleanup_plan.status_sources {
                        let inserted =
                            held.insert_inline(&function, &source.id, &mut budget, &mut failure);
                        if let Some(error) = failure.take() {
                            assert!(!inserted);
                            let before = crate::bounded_output::active_remaining();
                            assert!(!held.insert_inline(
                                &function,
                                &source.id,
                                &mut budget,
                                &mut failure
                            ));
                            assert!(failure.is_some());
                            assert_eq!(crate::bounded_output::active_remaining(), before);
                            return Err(error);
                        }
                        assert!(inserted);
                    }
                    drop(held);
                    assert_eq!(ids.len(), 256);
                    assert!(ids.balanced());
                    Ok::<_, Diagnostic>(())
                })();
                let ok = result.is_ok();
                drop(result);
                ok
            })
        })
    };
    let ((ok, _), refused, exact, first) = run(usize::MAX, 0);
    assert!(ok && !refused && first.is_none());
    for floor in [0, 37] {
        for available in [0, exact - 1, exact] {
            let ((ok, metrics), refused, spent, first) = run(available, floor);
            assert_eq!(ok, available == exact);
            assert_eq!(refused, !ok);
            assert_eq!(first.is_some(), !ok);
            if let Some(first) = first {
                assert_eq!(first.floor, floor);
            }
            assert!(spent <= available && metrics.allocated <= spent);
            assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
            if available == 0 {
                assert_eq!(metrics.allocated, 0);
            }
        }
    }
}

#[test]
fn inline_receipts_refuse_nested_and_restored_invocations_before_any_foreign_debit() {
    let names = (0..256)
        .map(|i| format!("prefix.{i:04}"))
        .collect::<Vec<_>>();
    let keys = names
        .iter()
        .map(|n| (n.as_str(), StatusLane::OperationFailure))
        .collect::<Vec<_>>();
    let function = function(&keys);
    let expected = Diagnostic::io("SPX-H006", format!("cleanup plan for function `{}` failed independent replay: status prefix insertion belongs to another invocation", function.id));
    let (((), metrics), refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            let mut ids = Directory::new(&function, &mut budget, "inline invocation").unwrap();
            let mut held = ids.held_prefix(&function, &mut budget).unwrap();
            held.fund_failure_cell(&function, &mut budget).unwrap();
            let mut failure = None;
            for source in &function.cleanup_plan.status_sources[..2] {
                assert!(held.insert_inline(&function, &source.id, &mut budget, &mut failure));
                assert!(failure.is_none());
            }
            let snapshot = (
                held.directory.len(),
                held.directory.nodes.as_ptr(),
                held.prefix,
                held.index.as_ref().unwrap().nodes.len(),
                held.index.as_ref().unwrap().nodes.as_ptr(),
            );
            let (_, child_refused, child_paid, child_first) =
                with_limit_usage_refusal(usize::MAX, || {
                    let before = crate::test_allocation_observer::current_metrics().allocated;
                    assert!(!held.insert_inline(
                        &function,
                        &function.cleanup_plan.status_sources[2].id,
                        &mut budget,
                        &mut failure
                    ));
                    assert_eq!(failure.take().unwrap(), expected);
                    assert_eq!(
                        crate::test_allocation_observer::current_metrics().allocated,
                        before
                    );
                });
            assert!(!child_refused && child_paid == 0 && child_first.is_none());
            assert!(matches!(held.owner.first.get(), Some(Refusal::Invocation)));
            let before = crate::test_allocation_observer::current_metrics().allocated;
            assert!(!held.insert_inline(
                &function,
                &function.cleanup_plan.status_sources[2].id,
                &mut budget,
                &mut failure
            ));
            assert_eq!(
                failure.take().unwrap(),
                Diagnostic::io("SPX-H006", String::new())
            );
            assert_eq!(
                crate::test_allocation_observer::current_metrics().allocated,
                before
            );
            assert_eq!(
                (
                    held.directory.len(),
                    held.directory.nodes.as_ptr(),
                    held.prefix,
                    held.index.as_ref().unwrap().nodes.len(),
                    held.index.as_ref().unwrap().nodes.as_ptr()
                ),
                snapshot
            );
        })
    });
    assert!(refused && first.is_some());
    assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
}

#[test]
fn inline_receipts_keep_exact_function_budget_and_current_key_bindings() {
    let names = (0..256)
        .map(|i| format!("binding.{i:04}"))
        .collect::<Vec<_>>();
    let keys = names
        .iter()
        .map(|n| (n.as_str(), StatusLane::OperationFailure))
        .collect::<Vec<_>>();
    let function = function(&keys);
    let wrong = function.clone();
    let cloned_key = function.cleanup_plan.status_sources[0].id.clone();
    let expected = Diagnostic::io("SPX-H006", format!("cleanup plan for function `{}` failed independent replay: status prefix insertion belongs to another subject or receiver", function.id));
    for mode in 0..3 {
        let (((), metrics), _, paid, _) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let mut foreign_budget = ReplayBudget::new();
                let mut ids = Directory::new(&function, &mut budget, "inline binding").unwrap();
                let mut held = ids.held_prefix(&function, &mut budget).unwrap();
                held.fund_failure_cell(&function, &mut budget).unwrap();
                let mut failure = None;
                let original_work = budget.remaining;
                let foreign_work = foreign_budget.remaining;
                let before = crate::bounded_output::active_remaining();
                let inserted = match mode {
                    0 => held.insert_inline(
                        &wrong,
                        &wrong.cleanup_plan.status_sources[0].id,
                        &mut budget,
                        &mut failure,
                    ),
                    1 => held.insert_inline(
                        &function,
                        &function.cleanup_plan.status_sources[0].id,
                        &mut foreign_budget,
                        &mut failure,
                    ),
                    _ => held.insert_inline(&function, &cloned_key, &mut budget, &mut failure),
                };
                assert!(!inserted);
                // A fresh foreign budget has no paid refusal-text clone cursor.
                assert_eq!(
                    failure.take().unwrap(),
                    if mode == 1 {
                        Diagnostic::io("SPX-H006", String::new())
                    } else {
                        expected.clone()
                    }
                );
                assert!(matches!(held.owner.first.get(), Some(Refusal::Binding)));
                assert_eq!(held.directory.len(), 0);
                assert_eq!(held.prefix, 0);
                // Refusal zeroes the supplied work sentinel without a foreign byte debit.
                assert_eq!(
                    foreign_budget.remaining,
                    if mode == 1 { 0 } else { foreign_work }
                );
                assert_eq!(budget.remaining, if mode == 1 { original_work } else { 0 });
                if mode != 2 {
                    assert_eq!(crate::bounded_output::active_remaining(), before);
                }
                assert!(!held.insert_inline(
                    &function,
                    &function.cleanup_plan.status_sources[0].id,
                    &mut budget,
                    &mut failure
                ));
                assert!(failure.is_some());
                assert!(held.directory.is_empty());
            })
        });
        assert!(
            !metrics.invalid
                && metrics.live == 0
                && metrics.allocated == metrics.freed
                && metrics.allocated <= paid
        );
    }
}
