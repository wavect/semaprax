//! Actual acquired backing and borrowed source/owner lifetime, through Drop.
use super::*;
use crate::bounded_output::{active_remaining, with_limit_usage_refusal};
use crate::test_allocation_observer::{current_metrics, measure};
fn program() -> ResolvedProgram {
    crate::hir::resolve(&crate::check(
        "module operands.owner; @id(\"main:é\") fn main()->i64 {let mut n=0;while n<3 {n=n+1;if n==1 {return 7;};0} return 9;}",
        "operand-owner.spx").unwrap()).unwrap()
}
fn function(program: &ResolvedProgram) -> &ResolvedFunction {
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "main:é")
        .unwrap();
    assert!(
        function.cleanup_plan.lexical_control.is_some(),
        "actual lexical loop fixture"
    );
    function
}
// Directory::new initializes both receiving slots for its optional frozen
// lookup. Option<Box<T>> has the documented one-pointer null niche; the owning
// directory test pins that actual FrozenStatusIndex layout independently.
fn directory_receivers() -> usize {
    2 * std::mem::size_of::<usize>()
}

fn controls() -> usize {
    std::mem::size_of::<(
        ReturnOperands<'static, 'static>,
        ReturnOperands<'static, 'static>,
        Result<ReturnOperands<'static, 'static>, Diagnostic>,
        ClassifierLedger,
        std::slice::Iter<'static, lexical_hir::ExitFact<'static>>,
        Option<&'static lexical_hir::ExitFact<'static>>,
        Option<&'static ResolvedExpr>,
        Directory<&'static ExpressionId>,
        Result<Directory<&'static ExpressionId>, Diagnostic>,
        Result<bool, Diagnostic>,
        Result<(), Diagnostic>,
        Result<(), Diagnostic>,
        Result<lexical_cert::LexicalAuth, Diagnostic>,
        &'static ReturnOperands<'static, 'static>,
    )>()
}
#[test]
fn exact_one_short_zero_bytes_and_work_pay_actual_controls_and_first_node_through_drop() {
    let p = program();
    let f = function(&p);
    let mut scan = ReplayBudget::new();
    let mut facts = lexical_hir::collect(f, &mut scan).unwrap();
    facts
        .exits
        .retain(|e| e.kind == LexicalExitKind::Return && e.value.is_some());
    facts.exits.truncate(1);
    assert_eq!(facts.exits.len(), 1);
    let key = &facts.exits[0].value.unwrap().id;
    let setup = f.id.as_str().len() + 512;
    let bytes = setup
        + controls()
        + directory_receivers()
        + Directory::<&ExpressionId>::node_layout_bytes();
    // 14 control initializers, one directory setup, one fact visit, one relocation slot, one node
    // initialization (the existing directory's VISIT_UNITS = 3).
    let work = 14 + 1 + 1 + 1 + 3;
    for allowance in [bytes, bytes - 1, 0] {
        let (((), observed), refused, used, first) = with_limit_usage_refusal(allowance, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let outcome = ReturnOperands::new::<Diagnostic>(f, &facts, &mut budget);
                assert_eq!(outcome.is_ok(), allowance == bytes);
                if let Ok(index) = outcome {
                    assert_eq!(MAX_REPLAY_WORK_UNITS - budget.remaining, work);
                    assert_eq!(active_remaining(), Some(0));
                    let pointer = index.ids.nodes_pointer_for_test();
                    assert!(index.contains(f, &facts, key, &mut budget).unwrap());
                    assert_eq!(index.ids.nodes_pointer_for_test(), pointer);
                    drop(index);
                } else {
                    let before = active_remaining();
                    assert!(ReturnOperands::new::<Diagnostic>(f, &facts, &mut budget).is_err());
                    assert_eq!(active_remaining(), before);
                }
                drop(budget);
            })
        });
        assert_eq!(refused, allowance != bytes);
        assert_eq!(first.is_some(), allowance != bytes);
        assert!(used <= allowance);
        assert!(!observed.invalid && observed.live == 0 && observed.allocated == observed.freed);
        assert!(observed.allocated <= used);
    }
    for allowance in [work, work - 1, 0] {
        let (((), observed), _, _, _) = with_limit_usage_refusal(bytes, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                budget.remaining = allowance;
                let result = ReturnOperands::new::<Diagnostic>(f, &facts, &mut budget);
                assert_eq!(result.is_ok(), allowance == work);
                assert_eq!(budget.remaining, 0);
                drop(result);
                drop(budget);
            })
        });
        assert!(!observed.invalid && observed.live == 0 && observed.allocated == observed.freed);
    }
}
#[test]
fn duplicate_ids_borrow_exact_input_and_spare_reuse_authenticates_sticky_failure() {
    let p = program();
    let f = function(&p);
    let mut scan = ReplayBudget::new();
    let mut facts = lexical_hir::collect(f, &mut scan).unwrap();
    facts.exits.truncate(1);
    let first = &facts.exits[0];
    let duplicate = lexical_hir::ExitFact {
        expression: first.expression,
        kind: first.kind,
        target: first.target,
        value: first.value,
    };
    facts.exits.push(duplicate);
    let key = &facts.exits[0].value.unwrap().id;
    let (((), observed), _, _, _) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            let index = ReturnOperands::new::<Diagnostic>(f, &facts, &mut budget).unwrap();
            assert_eq!(index.ids.len(), 1);
            assert!(index.contains(f, &facts, key, &mut budget).unwrap());
            assert_eq!(
                index.ids.first_key_for_test().unwrap().as_str().as_ptr(),
                key.as_str().as_ptr()
            );
            budget.remaining = 0;
            assert!(index.contains(f, &facts, key, &mut budget).is_err());
            let prior = active_remaining();
            let allocated = current_metrics().allocated;
            assert!(index.contains(f, &facts, key, &mut budget).is_err());
            assert_eq!(active_remaining(), prior);
            assert_eq!(current_metrics().allocated, allocated);
            drop(index);
            drop(budget);
        })
    });
    assert!(!observed.invalid && observed.live == 0 && observed.allocated == observed.freed);
}
#[test]
fn foreign_function_facts_budget_and_restored_nested_owner_never_reuse_backing() {
    let p = program();
    let f = function(&p);
    let foreign = f.clone();
    let mut scan = ReplayBudget::new();
    let facts = lexical_hir::collect(f, &mut scan).unwrap();
    let other_facts = lexical_hir::collect(f, &mut scan).unwrap();
    let expected = Diagnostic::io_static(
        "SPX-H006",
        "ordinary lexical return operands lost their owner",
    );
    for attack in 0..4 {
        let (((), observed), _, _, _) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let mut other_budget = ReplayBudget::new();
                let index = ReturnOperands::new::<Diagnostic>(f, &facts, &mut budget).unwrap();
                let result = match attack {
                    0 => index.authenticate(&foreign, &facts, &mut budget),
                    1 => index.authenticate(f, &other_facts, &mut budget),
                    2 => index.authenticate(f, &facts, &mut other_budget),
                    _ => {
                        with_limit_usage_refusal(usize::MAX, || {
                            index.authenticate(f, &facts, &mut budget)
                        })
                        .0
                    }
                };
                assert!(result.is_err());
                assert_eq!(result.as_ref().unwrap_err(), &expected);
                drop(result);
                let allocated = current_metrics().allocated;
                let remaining = active_remaining();
                let retry = index.authenticate(f, &facts, &mut budget);
                assert!(retry.is_err());
                assert_eq!(retry.as_ref().unwrap_err(), &expected);
                drop(retry);
                assert_eq!(current_metrics().allocated, allocated);
                assert_eq!(active_remaining(), remaining);
                drop(index);
                drop(other_budget);
                drop(budget);
            })
        });
        assert!(!observed.invalid && observed.live == 0 && observed.allocated == observed.freed);
    }
}

#[test]
fn expired_original_active_owner_cannot_restore_an_inventory_after_scope_exit() {
    let p = program();
    let f = function(&p);
    let mut scan = ReplayBudget::new();
    let facts = lexical_hir::collect(f, &mut scan).unwrap();
    let (_, observed) = measure(|| {
        let mut budget = ReplayBudget::new();
        let (index, refused, _, _) = with_limit_usage_refusal(usize::MAX, || {
            budget = ReplayBudget::new();
            ReturnOperands::new::<Diagnostic>(f, &facts, &mut budget).unwrap()
        });
        assert!(!refused);
        let allocated = current_metrics().allocated;
        assert!(index.authenticate(f, &facts, &mut budget).is_err());
        assert!(index.authenticate(f, &facts, &mut budget).is_err());
        assert_eq!(current_metrics().allocated, allocated);
        drop(index);
        drop(budget);
    });
    assert!(!observed.invalid && observed.live == 0 && observed.allocated == observed.freed);
}

#[test]
fn empty_and_duplicate_absolute_frontiers_follow_actual_control_node_and_comparison_work() {
    let p = program();
    let f = function(&p);
    let mut scan = ReplayBudget::new();
    let mut facts = lexical_hir::collect(f, &mut scan).unwrap();
    facts.exits.truncate(1);
    let one = &facts.exits[0];
    let duplicate = lexical_hir::ExitFact {
        expression: one.expression,
        kind: one.kind,
        target: one.target,
        value: one.value,
    };
    facts.exits.push(duplicate);
    for count in [0, 1, 2] {
        let rows = facts.exits.split_off(count);
        let key_bytes = facts
            .exits
            .first()
            .and_then(|e| e.value)
            .map_or(0, |e| e.id.as_str().len());
        let expected_bytes = f.id.as_str().len()
            + 512
            + controls()
            + directory_receivers()
            + if count == 0 {
                0
            } else {
                Directory::<&ExpressionId>::node_layout_bytes()
            };
        let expected_work = 14
            + 1
            + count
            + if count == 0 { 0 } else { 1 + 3 }
            + if count == 2 { key_bytes + 1 + 3 } else { 0 };
        let ((), refused, used, _) = with_limit_usage_refusal(expected_bytes, || {
            let mut budget = ReplayBudget::new();
            let index = ReturnOperands::new::<Diagnostic>(f, &facts, &mut budget).unwrap();
            assert_eq!(index.ids.len(), count.min(1));
            assert_eq!(MAX_REPLAY_WORK_UNITS - budget.remaining, expected_work);
            let remaining = active_remaining();
            index.authenticate(f, &facts, &mut budget).unwrap();
            assert_eq!(active_remaining(), remaining);
            drop(index);
            drop(budget);
        });
        assert!(!refused);
        assert_eq!(used, expected_bytes);
        facts.exits.extend(rows);
    }
}

#[test]
fn ordinary_operand_producer_costs_are_controls_plus_fact_visits_and_actual_directory_operations() {
    let p = program();
    let f = function(&p);
    let mut scan = ReplayBudget::new();
    let facts = lexical_hir::collect(f, &mut scan).unwrap();
    assert!(facts
        .exits
        .iter()
        .any(|exit| exit.kind == LexicalExitKind::Return && exit.value.is_some()));
    let mut observations = Vec::new();
    for actual_producer in [false, true] {
        let ((usage, metrics), refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                assert!(!budget.selected() && budget.pays_allocations());
                // Both sides acquire their real builder setup before the seam.
                budget.prepare_builder(f).unwrap();
                let before_work = budget.remaining;
                let before_bytes = active_remaining().unwrap();
                if actual_producer {
                    let index = ReturnOperands::new::<Diagnostic>(f, &facts, &mut budget).unwrap();
                    assert_eq!(index.ids.len(), 2);
                    drop(index);
                } else {
                    // Independently execute only the real directory operations
                    // over the same borrowed operands, in the same fact order.
                    let mut directory = Directory::new(
                        f,
                        &mut budget,
                        "independent ordinary operand directory seam",
                    )
                    .unwrap();
                    for exit in &facts.exits {
                        if let (LexicalExitKind::Return, Some(value)) = (exit.kind, exit.value) {
                            directory.insert(f, &value.id, &mut budget).unwrap();
                        }
                    }
                    assert_eq!(directory.len(), 2);
                    drop(directory);
                }
                let usage = (
                    before_work - budget.remaining,
                    before_bytes - active_remaining().unwrap(),
                );
                drop(budget);
                usage
            })
        });
        assert!(!refused && first.is_none());
        assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
        observations.push(usage);
    }
    let directory = observations[0];
    let producer = observations[1];
    // Fourteen actual receiving/initializer fields, one inspected ExitFact each,
    // and the independently executed relocation/comparison/AVL/node charges.
    assert_eq!(producer.0, 14 + facts.exits.len() + directory.0);
    assert_eq!(producer.1, controls() + directory.1);
    eprintln!("#832 ordinary producer={producer:?} directory_only={directory:?} controls_bytes={} initializer_work=14 fact_visits={}; old/current whole driver frontiers are exercised by lexical_replay::operand_tests", controls(), facts.exits.len());
}
