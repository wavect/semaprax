//! Full old AVL shape and complete actual receiver/last-Drop controls.
use super::*;
use crate::bounded_output::{with_limit_usage_refusal, ReservationRefusal};
use crate::test_allocation_observer::{current_metrics, measure, Metrics};

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
                    assert!(directory
                        .insert_certified(
                            &function,
                            &function.cleanup_plan.status_sources[0].id,
                            &mut budget,
                            &owner
                        )
                        .unwrap());
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

#[test]
fn held_prefix_keeps_every_original_link_and_shrinks_only_from_current_keys() {
    let names = [
        "shared.é.000",
        "shared.é.002",
        "shared.é.001",
        "shared.é.000",
        "shared.è",
        "shared.\0",
        "a",
        "",
        "界",
    ];
    let keys = names
        .iter()
        .flat_map(|name| {
            [
                StatusLane::OperationFailure,
                StatusLane::OwnerAdmission,
                StatusLane::ContractFalse,
                StatusLane::AllocationFailure,
            ]
            .map(|lane| (*name, lane))
        })
        .collect::<Vec<_>>();
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
            let owner = Owner::new(&function, &old, &mut old_budget).unwrap();
            let mut held = current.held_prefix(&function, &mut budget).unwrap();
            for source in &function.cleanup_plan.status_sources {
                let expected = old
                    .insert_certified(&function, &source.id, &mut old_budget, &owner)
                    .unwrap();
                assert_eq!(
                    held.insert(&function, &source.id, &mut budget).unwrap(),
                    expected
                );
                same_tree(&old, held.directory);
                let mut keys = held.directory.keys();
                let first = keys.next().unwrap().expression.as_str().as_bytes();
                let mut common = first.len();
                for key in keys {
                    common = first[..common]
                        .iter()
                        .zip(key.expression.as_str().as_bytes())
                        .take_while(|(left, right)| left == right)
                        .count();
                }
                assert_eq!(held.prefix, common);
            }
            drop(held);
            drop(owner);
            drop(current);
            drop(old);
            drop(budget);
            drop(old_budget);
        })
    });
    assert!(!refused && first.is_none());
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    assert!(allocation.allocated <= paid);
}

#[test]
fn held_prefix_preserves_physical_frontiers_and_final_drop_without_a_new_allowance() {
    let names = (0..128)
        .rev()
        .map(|index| format!("{}.{index:06}", "common.é".repeat(24)))
        .collect::<Vec<_>>();
    let keys = names
        .iter()
        .map(|name| (name.as_str(), StatusLane::OperationFailure))
        .collect::<Vec<_>>();
    let function = function(&keys);
    let observe = |held, available, floor| {
        with_limit_usage_refusal(available + floor, || {
            assert!(crate::bounded_output::set_active_floor(floor));
            measure(|| {
                let mut budget = ReplayBudget::new();
                let start = budget.remaining;
                let result = (|| {
                    let mut directory = Directory::new(
                        &function,
                        &mut budget,
                        "cleanup status identity directory",
                    )?;
                    if held {
                        let mut builder = directory.held_prefix(&function, &mut budget)?;
                        for source in &function.cleanup_plan.status_sources {
                            assert!(builder.insert(&function, &source.id, &mut budget)?);
                        }
                        drop(builder);
                    } else {
                        let owner = Owner::new(&function, &directory, &mut budget)?;
                        for source in &function.cleanup_plan.status_sources {
                            assert!(directory.insert_certified(
                                &function,
                                &source.id,
                                &mut budget,
                                &owner
                            )?);
                        }
                        drop(owner);
                    }
                    assert_eq!(directory.len(), 128);
                    assert!(directory.balanced());
                    drop(directory);
                    Ok::<_, Diagnostic>(())
                })();
                let ok = result.is_ok();
                let work = start - budget.remaining;
                drop(result);
                if !ok {
                    assert!(budget
                        .reserve_bytes(&function, 0, "sticky held prefix")
                        .is_err());
                }
                drop(budget);
                (ok, work)
            })
        })
    };
    let (((ok, old_work), old), refused, old_bytes, first) = observe(false, usize::MAX, 0);
    assert!(ok && !refused && first.is_none());
    let (((ok, new_work), new), refused, exact, first) = observe(true, usize::MAX, 0);
    assert!(ok && !refused && first.is_none());
    assert!(new_work < old_work && exact < old_bytes);
    assert_eq!(new.allocated, old.allocated);
    for allocation in [old, new] {
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
    }
    for floor in [0, 37] {
        for available in [0, exact - 1, exact] {
            let (((ok, _), allocation), refused, spent, first) = observe(true, available, floor);
            assert_eq!(ok, available == exact);
            assert_eq!(refused, !ok);
            assert_eq!(first.is_some(), !ok);
            if let Some(first) = first {
                assert_eq!(first.floor, floor);
            }
            assert!(spent <= available && allocation.allocated <= spent);
            assert!(!allocation.invalid && allocation.live == 0);
            assert_eq!(allocation.allocated, allocation.freed);
            if available == 0 {
                assert_eq!(allocation.allocated, 0);
            }
        }
    }
}

#[test]
fn held_prefix_stays_within_the_original_work_ceiling_on_a_wide_identity_set() {
    let prefix = "shared.".repeat(146);
    let names = (0..16_000)
        .map(|index| format!("{prefix}.{index:06}"))
        .collect::<Vec<_>>();
    let keys = names
        .iter()
        .map(|name| (name.as_str(), StatusLane::OperationFailure))
        .collect::<Vec<_>>();
    let function = function(&keys);
    let run = |held| {
        crate::bounded_output::with_limit(usize::MAX, || {
            let mut budget = ReplayBudget::new();
            let mut directory =
                Directory::new(&function, &mut budget, "cleanup status identity directory")
                    .unwrap();
            let result = (|| {
                if held {
                    let mut builder = directory.held_prefix(&function, &mut budget)?;
                    for source in &function.cleanup_plan.status_sources {
                        assert!(builder.insert(&function, &source.id, &mut budget)?);
                    }
                    drop(builder);
                } else {
                    let owner = Owner::new(&function, &directory, &mut budget)?;
                    for source in &function.cleanup_plan.status_sources {
                        assert!(directory.insert_certified(
                            &function,
                            &source.id,
                            &mut budget,
                            &owner
                        )?);
                    }
                    drop(owner);
                }
                Ok::<_, Diagnostic>(())
            })();
            let ok = result.is_ok();
            if let Err(error) = result {
                assert_eq!(error.code, "SPX-H006");
                assert!(error
                    .message
                    .contains("cleanup replay work budget exhausted"));
            }
            if ok {
                assert_eq!(directory.len(), 16_000);
                assert!(directory.balanced());
            }
            ok
        })
        .0
    };
    assert!(!run(false));
    assert!(run(true));
}

#[test]
fn held_prefix_never_uses_its_bound_in_a_foreign_or_restored_invocation() {
    let function = function(&[
        ("same.prefix.0", StatusLane::OperationFailure),
        ("same.prefix.1", StatusLane::OperationFailure),
        ("same.prefix.2", StatusLane::OperationFailure),
    ]);
    let expected=Diagnostic::io("SPX-H006",format!("cleanup plan for function `{}` failed independent replay: status prefix insertion belongs to another invocation",function.id));
    let (((), allocation), refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            let mut directory =
                Directory::new(&function, &mut budget, "cleanup status identity directory")
                    .unwrap();
            let mut held = directory.held_prefix(&function, &mut budget).unwrap();
            for source in &function.cleanup_plan.status_sources[..2] {
                assert!(held.insert(&function, &source.id, &mut budget).unwrap());
            }
            let prefix = held.prefix;
            let len = held.directory.len();
            let pointer = held.directory.nodes.as_ptr();
            assert!(prefix > 0);
            let (_, child_refused, child_paid, child_first) =
                with_limit_usage_refusal(usize::MAX, || {
                    let before = current_metrics().allocated;
                    let error = held
                        .insert(
                            &function,
                            &function.cleanup_plan.status_sources[2].id,
                            &mut budget,
                        )
                        .unwrap_err();
                    assert_eq!(error, expected);
                    drop(error);
                    assert_eq!(current_metrics().allocated, before);
                });
            assert!(!child_refused && child_paid == 0 && child_first.is_none());
            assert!(matches!(held.owner.first.get(), Some(Refusal::Invocation)));
            let before = current_metrics().allocated;
            let error = held
                .insert(
                    &function,
                    &function.cleanup_plan.status_sources[2].id,
                    &mut budget,
                )
                .unwrap_err();
            assert_eq!(error, Diagnostic::io("SPX-H006", String::new()));
            drop(error);
            assert_eq!(current_metrics().allocated, before);
            assert_eq!(held.prefix, prefix);
            assert_eq!(held.directory.len(), len);
            assert_eq!(held.directory.nodes.as_ptr(), pointer);
            drop(held);
            drop(directory);
            drop(budget);
        })
    });
    assert!(refused && first.is_some());
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    let wrong = function.clone();
    let expected=Diagnostic::io("SPX-H006",format!("cleanup plan for function `{}` failed independent replay: status prefix insertion belongs to another subject or receiver",wrong.id));
    let (((), allocation), _, paid, _) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            let mut directory =
                Directory::new(&function, &mut budget, "cleanup status identity directory")
                    .unwrap();
            let mut held = directory.held_prefix(&function, &mut budget).unwrap();
            let result = held.insert(
                &wrong,
                &wrong.cleanup_plan.status_sources[0].id,
                &mut budget,
            );
            assert_eq!(result.unwrap_err(), expected);
            assert!(matches!(held.owner.first.get(), Some(Refusal::Binding)));
            assert_eq!(held.directory.len(), 0);
            assert_eq!(held.prefix, 0);
            drop(held);
            drop(directory);
            drop(budget);
        })
    });
    assert!(!allocation.invalid && allocation.live == 0 && allocation.allocated <= paid);
    assert_eq!(allocation.allocated, allocation.freed);
}

#[test]
fn unobserved_wide_status_construction_uses_the_same_authenticated_held_bound() {
    let prefix = "shared.".repeat(146);
    let names = (0..16_000)
        .map(|index| format!("{prefix}.{index:06}"))
        .collect::<Vec<_>>();
    let keys = names
        .iter()
        .map(|name| (name.as_str(), StatusLane::OperationFailure))
        .collect::<Vec<_>>();
    let function = function(&keys);
    let mut budget = ReplayBudget::unobserved();
    let mut directory =
        Directory::new(&function, &mut budget, "cleanup status identity directory").unwrap();
    let mut held = directory.held_prefix(&function, &mut budget).unwrap();
    for source in &function.cleanup_plan.status_sources {
        assert!(held.insert(&function, &source.id, &mut budget).unwrap());
    }
    drop(held);
    assert_eq!(directory.len(), 16_000);
    assert!(directory.balanced());
    assert!(budget.remaining < MAX_REPLAY_WORK_UNITS && budget.remaining > 0);
}
