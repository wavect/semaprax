//! Full old AVL shape and complete actual receiver/last-Drop controls.
use super::{
    Diagnostic, Directory, Owner, Refusal, ReplayBudget, ResolvedFunction, StatusSourceId,
};
use super::super::{ExpressionId, MAX_REPLAY_WORK_UNITS, StatusLane, StatusProducer, StatusSource};
use crate::bounded_output::{ReservationRefusal, with_limit_usage_refusal};
use crate::test_allocation_observer::{Metrics, current_metrics, measure};

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
fn same_tree(old: &Directory<&StatusSourceId>, current: &Directory<&StatusSourceId>) {
    assert_eq!(
        (old.root, old.first, old.len()),
        (current.root, current.first, current.len())
    );
    assert!(old.balanced() && current.balanced());
    assert!(old.keys().eq(current.keys()));
    for (a, b) in old.nodes.iter().zip(&current.nodes) {
        assert_eq!(a.key, b.key);
        assert_eq!(
            (a.left, a.right, a.parent, a.next, a.height),
            (b.left, b.right, b.parent, b.next, b.height)
        );
    }
}
#[test]
fn certified_status_insertion_keeps_complete_original_avl_shape_and_byte_lane_order() {
    let names = [
        "",
        "\0",
        "a",
        "a\0",
        "aé",
        "a界",
        "a\u{10ffff}",
        "aa",
        "ab",
        "z",
    ];
    let lanes = [
        StatusLane::OwnerAdmission,
        StatusLane::OperationFailure,
        StatusLane::AllocationFailure,
        StatusLane::ContractFalse,
    ];
    let keys: Vec<_> = names
        .iter()
        .flat_map(|name| lanes.iter().map(|lane| (*name, *lane)))
        .collect();
    for mut order in [
        keys.clone(),
        keys.iter().rev().copied().collect(),
        (0..keys.len())
            .map(|i| keys[(i * 17) % keys.len()])
            .collect(),
    ] {
        order.push(order[0]);
        order.push(*order.last().unwrap());
        let function = function(&order);
        let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut old_budget = ReplayBudget::new();
                let mut budget = ReplayBudget::new();
                let mut old = Directory::new(
                    &function,
                    &mut old_budget,
                    "cleanup status identity directory",
                )
                .unwrap();
                let mut current =
                    Directory::new(&function, &mut budget, "cleanup status identity directory")
                        .unwrap();
                let owner = Owner::new(&function, &current, &mut budget).unwrap();
                for source in &function.cleanup_plan.status_sources {
                    let expected = old.insert(&function, &source.id, &mut old_budget).unwrap();
                    let actual = current
                        .insert_certified(&function, &source.id, &mut budget, &owner)
                        .unwrap();
                    assert_eq!(actual, expected);
                    same_tree(&old, &current);
                }
                drop(owner);
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
        assert!(allocation.allocated <= paid && allocation.highwater <= paid);
    }
    let stem = "status.shared-prefix.界\0".repeat(16);
    let names: Vec<_> = (0..129).map(|i| format!("{stem}{i:03}")).collect();
    let keys: Vec<_> = names
        .iter()
        .map(|name| (name.as_str(), StatusLane::OperationFailure))
        .collect();
    let function = function(&keys);
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut old_budget = ReplayBudget::new();
            let mut budget = ReplayBudget::new();
            let mut old = Directory::new(
                &function,
                &mut old_budget,
                "cleanup status identity directory",
            )
            .unwrap();
            let mut current =
                Directory::new(&function, &mut budget, "cleanup status identity directory")
                    .unwrap();
            let owner = Owner::new(&function, &current, &mut budget).unwrap();
            for source in &function.cleanup_plan.status_sources {
                assert_eq!(
                    old.insert(&function, &source.id, &mut old_budget).unwrap(),
                    current
                        .insert_certified(&function, &source.id, &mut budget, &owner)
                        .unwrap()
                );
                same_tree(&old, &current);
            }
            assert!(
                budget.remaining > old_budget.remaining,
                "only this actual repeated-prefix construction has a source-specific work reduction"
            );
            drop(owner);
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
    assert!(allocation.allocated <= paid && allocation.highwater <= paid);
}

fn observe(
    function: &ResolvedFunction,
    current: bool,
    work: usize,
    bytes: usize,
) -> (
    (bool, usize, Metrics),
    bool,
    usize,
    Option<ReservationRefusal>,
) {
    let expected_work = Diagnostic::io(
        "SPX-H006",
        format!(
            "cleanup plan for function `{}` failed independent replay: cleanup replay work budget exhausted during cleanup status identity directory",
            function.id
        ),
    );
    let ((value, allocation), refused, paid, first) = with_limit_usage_refusal(bytes, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            budget.remaining = work;
            let mut directory =
                match Directory::new(function, &mut budget, "cleanup status identity directory") {
                    Ok(value) => value,
                    Err(error) => {
                        drop(error);
                        drop(budget);
                        return (false, 0);
                    }
                };
            let owner = if current {
                match Owner::new(function, &directory, &mut budget) {
                    Ok(value) => Some(value),
                    Err(error) => {
                        if bytes == usize::MAX {
                            assert_eq!(error, expected_work);
                        }
                        drop(error);
                        let at = current_metrics().allocated;
                        let retry = Owner::new(function, &directory, &mut budget)
                            .err()
                            .expect("selected setup refusal remains sticky");
                        assert_eq!(retry, Diagnostic::io("SPX-H006", String::new()));
                        drop(retry);
                        assert_eq!(current_metrics().allocated, at);
                        drop(directory);
                        let used = work - budget.remaining;
                        drop(budget);
                        return (false, used);
                    }
                }
            } else {
                None
            };
            let mut success = true;
            for source in &function.cleanup_plan.status_sources {
                let result = match &owner {
                    Some(owner) => {
                        directory.insert_certified(function, &source.id, &mut budget, owner)
                    }
                    None => directory.insert(function, &source.id, &mut budget),
                };
                match result {
                    Ok(inserted) => assert!(inserted),
                    Err(error) => {
                        assert_eq!(error.code, "SPX-H006");
                        assert_eq!(error.severity, crate::diagnostic::Severity::Error);
                        assert_eq!(error.path, None);
                        assert_eq!(error.span, None);
                        assert_eq!(error.help, None);
                        if bytes == usize::MAX {
                            assert_eq!(error, expected_work);
                        }
                        drop(error);
                        success = false;
                        let at = current_metrics().allocated;
                        let remaining = budget.remaining;
                        let retry = match &owner {
                            Some(owner) => {
                                directory.insert_certified(function, &source.id, &mut budget, owner)
                            }
                            None => directory.insert(function, &source.id, &mut budget),
                        };
                        assert_eq!(retry, Err(Diagnostic::io("SPX-H006", String::new())));
                        drop(retry);
                        assert_eq!(current_metrics().allocated, at);
                        assert_eq!(budget.remaining, remaining);
                        break;
                    }
                }
            }
            let used = work - budget.remaining;
            drop(owner);
            drop(directory);
            drop(budget);
            (success, used)
        })
    });
    assert!(
        !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
    );
    assert!(allocation.allocated <= paid && allocation.highwater <= paid);
    ((value.0, value.1, allocation), refused, paid, first)
}
#[test]
fn certified_status_actual_work_and_every_small_byte_frontier_are_sticky_through_drop() {
    let function = function(&[
        ("same.0", StatusLane::OperationFailure),
        ("same.1", StatusLane::OperationFailure),
        ("same.2", StatusLane::OperationFailure),
    ]);
    for current in [false, true] {
        let ((ok, work, _), refused, bytes, first) =
            observe(&function, current, MAX_REPLAY_WORK_UNITS, usize::MAX);
        assert!(ok && !refused && first.is_none());
        for limit in 0..=work {
            let ((ok, used, _), refused, _, _) = observe(&function, current, limit, usize::MAX);
            assert_eq!(ok, limit == work);
            assert!(!refused);
            assert_eq!(used, limit);
        }
        for limit in 0..=bytes {
            let ((ok, _, allocation), refused, used, first) =
                observe(&function, current, MAX_REPLAY_WORK_UNITS, limit);
            assert_eq!(ok, limit == bytes);
            assert_eq!(refused, !ok);
            assert_eq!(first.is_some(), !ok);
            assert!(used <= limit);
            if limit == 0 {
                assert_eq!(allocation.allocated, 0);
            }
        }
    }
}
#[test]
fn certified_status_foreign_restored_and_wrong_subject_never_reactivate_the_bound_owner() {
    let function = function(&[
        ("same.0", StatusLane::OperationFailure),
        ("same.1", StatusLane::ContractFalse),
    ]);
    let expected = Diagnostic::io(
        "SPX-H006",
        format!(
            "cleanup plan for function `{}` failed independent replay: status prefix insertion belongs to another invocation",
            function.id
        ),
    );
    for cached in [false, true] {
        let (((), allocation), refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let mut directory =
                    Directory::new(&function, &mut budget, "cleanup status identity directory")
                        .unwrap();
                let owner = Owner::new(&function, &directory, &mut budget).unwrap();
                if cached {
                    assert!(
                        directory
                            .insert_certified(
                                &function,
                                &function.cleanup_plan.status_sources[0].id,
                                &mut budget,
                                &owner
                            )
                            .unwrap()
                    );
                }
                let len = directory.len();
                let pointer = directory.nodes.as_ptr();
                let key = &function.cleanup_plan.status_sources[usize::from(cached)].id;
                let (_, child_refused, child_paid, _) =
                    with_limit_usage_refusal(usize::MAX, || {
                        let at = current_metrics().allocated;
                        let error = directory
                            .insert_certified(&function, key, &mut budget, &owner)
                            .unwrap_err();
                        assert_eq!(error, expected);
                        drop(error);
                        assert_eq!(current_metrics().allocated, at);
                    });
                assert!(!child_refused && child_paid == 0);
                assert!(matches!(owner.first.get(), Some(Refusal::Invocation)));
                // The nested test ledger setup/Drop remains wholly observed.
                let at = current_metrics().allocated;
                let error = directory
                    .insert_certified(&function, key, &mut budget, &owner)
                    .unwrap_err();
                assert_eq!(error, Diagnostic::io("SPX-H006", String::new()));
                drop(error);
                assert_eq!(current_metrics().allocated, at);
                assert_eq!(directory.len(), len);
                assert_eq!(directory.nodes.as_ptr(), pointer);
                drop(owner);
                drop(directory);
                drop(budget);
            })
        });
        assert!(refused && first.is_some());
        assert!(
            !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
        );
    }
    let wrong = function.clone();
    let expected = Diagnostic::io(
        "SPX-H006",
        format!(
            "cleanup plan for function `{}` failed independent replay: status prefix insertion belongs to another subject or receiver",
            wrong.id
        ),
    );
    let (((), allocation), _, paid, _) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            let mut directory =
                Directory::new(&function, &mut budget, "cleanup status identity directory")
                    .unwrap();
            let owner = Owner::new(&function, &directory, &mut budget).unwrap();
            let result = directory.insert_certified(
                &wrong,
                &wrong.cleanup_plan.status_sources[0].id,
                &mut budget,
                &owner,
            );
            assert_eq!(result.unwrap_err(), expected);
            assert!(matches!(owner.first.get(), Some(Refusal::Binding)));
            drop(owner);
            drop(directory);
            drop(budget);
        })
    });
    assert!(
        !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
    );
    assert!(allocation.allocated <= paid && allocation.highwater <= paid);
}
