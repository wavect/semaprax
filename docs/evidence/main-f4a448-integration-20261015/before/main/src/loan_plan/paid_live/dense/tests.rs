//! Full old/current logical work and actual receiving-to-last-Drop controls.
use super::*;
use crate::bounded_output::{active_remaining, with_limit_usage, with_limit_usage_refusal};
use crate::test_allocation_observer::{current_metrics, measure};

fn physical(
    limit: usize,
    checkpoints: &mut [usize; 16],
) -> (
    (bool, crate::test_allocation_observer::Metrics),
    bool,
    usize,
    Option<crate::bounded_output::ReservationRefusal>,
) {
    with_limit_usage_refusal(limit, || {
        measure(|| {
            let payer = match CfgReservation::acquire() {
                Ok(payer) => payer,
                Err(_) => return false,
            };
            let result = (|| {
                checkpoints[0] = limit - active_remaining().unwrap();
                let mut nodes = Nodes::new(65, &payer)?;
                checkpoints[1] = limit - active_remaining().unwrap();
                for (index, key) in [0, 64, 65, 4095, u16::MAX, 64].into_iter().enumerate() {
                    assert_eq!(nodes.insert(key, &payer)?, index != 5);
                    checkpoints[index + 2] = limit - active_remaining().unwrap();
                    assert!(nodes.contains(&key));
                }
                assert_eq!(nodes.len(), 5);
                assert!(nodes.values(&payer)?.eq([0, 64, 65, 4095, u16::MAX]));
                checkpoints[8] = limit - active_remaining().unwrap();
                assert!(nodes.remove(&65, &payer)?);
                assert!(!nodes.remove(&65, &payer)?);
                let values = nodes.into_vec(&payer)?;
                assert_eq!(values, [0, 64, 4095, u16::MAX]);
                checkpoints[9] = limit - active_remaining().unwrap();
                drop(values);
                Ok::<_, Diagnostic>(())
            })();
            if result.is_err() {
                let first = payer
                    .first()
                    .expect("positive receiver has only physical refusal");
                let before = current_metrics();
                assert_eq!(payer.reserve(0), Err(first));
                assert_eq!(payer.reserve(1), Err(first));
                assert!(Nodes::new(65, &payer).is_err());
                assert_eq!(current_metrics().allocated, before.allocated);
            }
            let success = result.is_ok();
            drop(result);
            drop(payer);
            success
        })
    })
}
#[test]
fn dense_live_keys_keep_full_u16_order_growth_and_each_receiving_refusal_through_drop() {
    let mut checkpoints = [0; 16];
    let ((ok, actual), refused, exact, first) = physical(usize::MAX, &mut checkpoints);
    assert!(ok && !refused && first.is_none());
    assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
    assert!(actual.allocated <= exact && actual.highwater <= exact);
    let mut frontiers = checkpoints[..10].to_vec();
    // Actual growth backing is a separate reservation after the Vec/result
    // receiver fee: exercise both sides of all three real allocations.
    frontiers.extend([
        checkpoints[2] - 2 * size_of::<u64>(),
        checkpoints[5] - 64 * size_of::<u64>(),
        checkpoints[6] - 1024 * size_of::<u64>(),
    ]);
    frontiers.extend([0, exact]);
    for frontier in frontiers {
        for limit in [frontier.saturating_sub(1), frontier] {
            let ((ok, observed), refused, used, first) = physical(limit, &mut [0; 16]);
            assert_eq!(ok, limit >= exact);
            assert_eq!(refused, !ok);
            assert_eq!(first.is_some(), !ok);
            assert!(
                !observed.invalid && observed.live == 0 && observed.allocated == observed.freed
            );
            assert!(observed.allocated <= used && observed.highwater <= used);
            if limit == 0 {
                assert_eq!(observed.allocated, 0);
            }
        }
    }
}
#[test]
fn dense_live_foreign_then_restored_owner_cannot_publish_or_allocate() {
    for cached in [false, true] {
        let (((), observed), refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let payer = CfgReservation::acquire().unwrap();
                let mut nodes = Nodes::new(4096, &payer).unwrap();
                if cached {
                    assert!(nodes.insert(63, &payer).unwrap());
                }
                let len = nodes.len();
                let pointer = nodes.words.as_ptr();
                let (_, child_refused, child_paid, _) =
                    with_limit_usage_refusal(usize::MAX, || {
                        // The nested test ledger is constructed before this closure.
                        // Its construction and Drop remain in the whole observer.
                        let before = current_metrics().allocated;
                        assert!(nodes.insert(64, &payer).is_err());
                        assert_eq!(current_metrics().allocated, before);
                    });
                assert!(!child_refused);
                assert_eq!(child_paid, 0);
                assert_eq!(
                    payer.first(),
                    Some(crate::bounded_output::cfg_reservation::Refused::Detached)
                );
                let before = current_metrics().allocated;
                assert!(nodes.insert(u16::MAX, &payer).is_err());
                assert!(nodes.remove(&63, &payer).is_err());
                assert!(nodes.values(&payer).is_err());
                assert_eq!(nodes.len(), len);
                assert_eq!(nodes.words.as_ptr(), pointer);
                assert!(!nodes.contains(&64));
                assert_eq!(current_metrics().allocated, before);
                drop(nodes);
                drop(payer);
            })
        });
        assert!(refused && first.is_some());
        assert!(!observed.invalid && observed.live == 0 && observed.allocated == observed.freed);
    }
}
#[test]
fn dense_liveness_preserves_full_old_rows_edges_and_every_checked_work_frontier() {
    for count in [1, 3, 17] {
        let (program, index) = boundary_tests::fixture_with_uses(count, true);
        let function = &program.functions[index];
        let ((), refused, _) = with_limit_usage(usize::MAX, || {
            let payer = CfgReservation::acquire().unwrap();
            let mut setup = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
            setup.allocation = Some(&payer);
            let cfg = build_cfg(function, &mut setup).unwrap();
            let reachable = work::Reachability::build(&cfg, [0].into_iter(), &mut setup).unwrap();
            let mut seeds = cfg_directory::Set::new(&payer).unwrap();
            for node in 0..cfg.points.len() {
                cfg_directory::set_insert(&mut seeds, node as u16, Some(&payer)).unwrap();
            }
            let mut full = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
            full.allocation = Some(&payer);
            let expected = super::super::live_nodes_with_pending(
                &cfg,
                LoanId(0),
                0,
                &seeds,
                &reachable,
                &mut full,
                &mut Vec::new(),
            )
            .unwrap();
            let expected = expected.into_vec(&payer).unwrap();
            for limit in 0..=full.used {
                let mut old = WorkCounter::new(limit);
                old.allocation = Some(&payer);
                let mut current = WorkCounter::new(limit);
                current.allocation = Some(&payer);
                let old_result = super::super::live_nodes_with_pending(
                    &cfg,
                    LoanId(0),
                    0,
                    &seeds,
                    &reachable,
                    &mut old,
                    &mut Vec::new(),
                )
                .map(|set| set.into_vec(&payer).unwrap());
                let current_result = live_nodes_with_pending(
                    &cfg,
                    LoanId(0),
                    0,
                    &seeds,
                    &reachable,
                    &mut current,
                    &mut Vec::new(),
                )
                .map(|set| set.into_vec(&payer).unwrap());
                assert_eq!(old.used, current.used);
                assert_eq!(old_result, current_result);
                if let Ok(rows) = current_result {
                    assert_eq!(rows, expected);
                }
            }
            let mut old_work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
            old_work.allocation = Some(&payer);
            let mut current_work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
            current_work.allocation = Some(&payer);
            let old = super::super::live_nodes_with_pending(
                &cfg,
                LoanId(0),
                0,
                &seeds,
                &reachable,
                &mut old_work,
                &mut Vec::new(),
            )
            .unwrap();
            let current = live_nodes_with_pending(
                &cfg,
                LoanId(0),
                0,
                &seeds,
                &reachable,
                &mut current_work,
                &mut Vec::new(),
            )
            .unwrap();
            assert_eq!(old_work.used, current_work.used);
            let old_live = [old];
            let current_live = [current];
            let mut full_edges = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
            full_edges.allocation = Some(&payer);
            let expected_edges =
                super::super::edge_liveness(&cfg, &old_live, &mut full_edges).unwrap();
            for limit in 0..=full_edges.used {
                let mut old = WorkCounter::new(limit);
                old.allocation = Some(&payer);
                let mut current = WorkCounter::new(limit);
                current.allocation = Some(&payer);
                let old_result = super::super::edge_liveness(&cfg, &old_live, &mut old);
                let current_result = edge_liveness(&cfg, &current_live, &mut current);
                assert_eq!(old.used, current.used);
                assert_eq!(old_result, current_result);
                if let Ok(rows) = current_result {
                    assert_eq!(rows, expected_edges);
                }
            }
            assert!(payer.first().is_none());
        });
        assert!(!refused);
    }
}

#[test]
fn dense_complete_plans_and_late_full_diagnostics_keep_old_constructor_and_last_drop() {
    for hostile in [false, true] {
        let (mut program, index) = boundary_tests::fixture_with_uses(3, true);
        if hostile {
            let expected = saved_planner::build_plan(&program, &program.functions[index]).unwrap();
            let owned = expected.loans[0].origin.root.clone();
            let ResolvedExprKind::Block { statements, .. } =
                &mut program.functions[index].body.kind
            else {
                panic!("unchanged boundary block")
            };
            let ResolvedStatement::Let { value, .. } = &mut statements[4] else {
                panic!("first retained use")
            };
            value.kind = ResolvedExprKind::Place(Place {
                root: owned,
                projections: Vec::new(),
            });
            value.ty = ResolvedType::Bytes;
            value.ownership = OwnershipMode::Own;
        }
        let function = &program.functions[index];
        let expected = saved_planner::build_plan(&program, function);
        assert_eq!(expected.is_err(), hostile);
        let run = |limit| {
            with_limit_usage_refusal(limit, || {
                measure(|| {
                    let payer = match CfgReservation::acquire() {
                        Ok(payer) => payer,
                        Err(_) => return false,
                    };
                    // Full old receiver remains inside the SAME observed owner as the
                    // ordinary current caller, including both result comparisons/Drop.
                    let old = super::super::funded_before::build(&program, function, &payer);
                    let current = funded_flat::build(&program, function, &payer);
                    let success = match (&old, &current) {
                        (Ok(old), Ok(current)) => {
                            assert!(!hostile);
                            assert_eq!(old, current);
                            assert_eq!(current, expected.as_ref().unwrap());
                            true
                        }
                        (
                            Err(storage::PlannerError::Semantic(old)),
                            Err(storage::PlannerError::Semantic(current)),
                        ) => {
                            assert!(hostile);
                            assert_eq!(old, current);
                            assert_eq!(current, expected.as_ref().unwrap_err());
                            assert_eq!(
                                current.message,
                                "move, mutation, or transfer overlaps an active shared loan"
                            );
                            assert!(current.span.is_some() && current.help.is_some());
                            true
                        }
                        _ => {
                            let first = payer
                                .first()
                                .expect("only physical short receiver may differ");
                            let at = current_metrics().allocated;
                            assert_eq!(payer.reserve(0), Err(first));
                            assert!(
                                matches!(funded_flat::build(&program, function, &payer), Err(storage::PlannerError::Physical(repeated)) if repeated == first)
                            );
                            assert_eq!(current_metrics().allocated, at);
                            false
                        }
                    };
                    drop(old);
                    drop(current);
                    drop(payer);
                    success
                })
            })
        };
        let ((ok, observed), refused, exact, first) = run(usize::MAX);
        assert!(ok && !refused && first.is_none());
        assert!(!observed.invalid && observed.live == 0 && observed.allocated == observed.freed);
        assert!(observed.allocated <= exact && observed.highwater <= exact);
        for limit in [0, exact - 1, exact] {
            let ((ok, observed), refused, used, first) = run(limit);
            assert_eq!(ok, limit == exact);
            assert_eq!(refused, !ok);
            assert_eq!(first.is_some(), !ok);
            assert!(
                !observed.invalid && observed.live == 0 && observed.allocated == observed.freed
            );
            assert!(observed.allocated <= used && observed.highwater <= used);
            if limit == 0 {
                assert_eq!(observed.allocated, 0);
            }
        }
    }
}
