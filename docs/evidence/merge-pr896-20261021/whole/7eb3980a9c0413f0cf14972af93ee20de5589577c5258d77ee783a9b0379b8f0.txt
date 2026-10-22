//! Actual caller adoption; the complete saved receiver stays strict v1.
use crate::bounded_output::cfg_reservation::CfgReservation;
use crate::bounded_output::with_limit_usage;
use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedProgram;
use crate::loan_plan::{build_plan, funded_flat, saved_planner, storage, LoanPlan};
use crate::test_allocation_observer::measure;
const SOURCE: &str = include_str!("../../../../tests/fixtures/nested-byte-slice-traversal.spx");
fn program() -> ResolvedProgram {
    let ast = crate::check(SOURCE, "candidate-nested-bytes.spx").unwrap();
    crate::hir::resolve(&ast).unwrap()
}
fn semantic(result: Result<LoanPlan, storage::PlannerError>) -> Result<LoanPlan, Diagnostic> {
    match result {
        Ok(plan) => Ok(plan),
        Err(storage::PlannerError::Semantic(error)) => Err(error),
        Err(storage::PlannerError::Physical(_)) => {
            panic!("ample current receiver refused physically")
        }
    }
}
#[test]
fn actual_ordinary_and_funded_candidates_replay_nested_bytes_before_plan_selection() {
    let program = program();
    crate::hir::validate(&program).unwrap();
    for function in &program.functions {
        let expected = build_plan(&program, function).unwrap();
        assert_eq!(expected, function.loan_plan);
        let ((), refused, _) = with_limit_usage(1 << 26, || {
            let owner = CfgReservation::acquire().unwrap();
            let current = semantic(funded_flat::build(&program, function, &owner)).unwrap();
            assert_eq!(current, expected);
        });
        assert!(!refused);
    }
    let sum = program
        .functions
        .iter()
        .find(|function| function.name == "sum")
        .unwrap();
    let error = saved_planner::build_plan(&program, sum).unwrap_err();
    // Complete six-field legacy diagnostic, not a code-only recognition.
    assert_eq!(
        error,
        Diagnostic::io_static(
            "SPX-H006",
            "borrowed byte traversal is nested in another traversal"
        )
    );
    let crate::hir::ResolvedExprKind::Block { statements, .. } = &sum.body.kind else {
        panic!("sum block")
    };
    let outer = statements
        .iter()
        .find_map(|statement| match statement {
            crate::hir::ResolvedStatement::Let { binding, value, .. }
                if binding.name == "#for-bytes" =>
            {
                Some(value)
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(
        crate::hir::byte_traversal_region(outer).err().unwrap(),
        error
    );
    let ((), refused, _) = with_limit_usage(1 << 26, || {
        let owner = CfgReservation::acquire().unwrap();
        for legacy in [false, true] {
            let before = if legacy {
                crate::loan_plan::paid_candidate_legacy::inspect(&program, sum, &owner)
            } else {
                crate::loan_plan::paid_candidate_before::inspect(&program, sum, &owner)
            };
            assert_eq!(before.unwrap_err(), error);
        }
    });
    assert!(!refused);
    let wire = crate::cache_codec::encode(&program).unwrap();
    let decoded: ResolvedProgram = crate::cache_codec::decode(&wire).unwrap();
    crate::hir::validate(&decoded).unwrap();
    crate::loan_plan::validate_program(&decoded).unwrap();
    assert_eq!(crate::cache_codec::encode(&decoded).unwrap(), wire);
}
#[test]
fn actual_nested_candidate_whole_receiver_exact_short_zero_and_selected_refusal_drop() {
    let program = program();
    let function = program
        .functions
        .iter()
        .find(|function| function.name == "sum")
        .unwrap();
    let expected = build_plan(&program, function).unwrap();
    let (((), metrics), refused, exact) = with_limit_usage(usize::MAX, || {
        measure(|| {
            let owner = CfgReservation::acquire().unwrap();
            let result = semantic(funded_flat::build(&program, function, &owner)).unwrap();
            assert_eq!(result, expected);
            drop(result);
            drop(owner);
        })
    });
    assert!(!refused && exact > 0 && !metrics.invalid);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.allocated, metrics.freed);
    assert!(metrics.allocated <= exact);
    for limit in [0, exact - 1, exact] {
        let ((success, metrics), refused, used) = with_limit_usage(limit, || {
            measure(|| {
                let owner = match CfgReservation::acquire() {
                    Ok(owner) => owner,
                    Err(_) => return false,
                };
                let result = funded_flat::build(&program, function, &owner);
                let success = result.is_ok();
                if !success {
                    assert!(matches!(&result, Err(storage::PlannerError::Physical(_))));
                }
                if let Ok(plan) = &result {
                    assert_eq!(plan, &expected);
                }
                drop(result);
                if !success {
                    let before = crate::test_allocation_observer::current_metrics().allocated;
                    assert!(owner.reserve(0).is_err());
                    assert!(owner.reserve(1).is_err());
                    assert_eq!(
                        crate::test_allocation_observer::current_metrics().allocated,
                        before
                    );
                }
                drop(owner);
                success
            })
        });
        assert_eq!(success, limit == exact);
        assert_eq!(refused, limit != exact);
        assert!(used <= limit && !metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.allocated, metrics.freed);
    }
}

#[test]
fn actual_classifier_preserves_full_semantics_and_original_child_refusal_and_foreign_first() {
    use crate::bounded_output::cfg_reservation::Refused;
    use crate::bounded_output::{
        active_remaining, reserve_active_required, with_limit_usage_refusal,
    };
    let semantic_error = Diagnostic {
        code: "SPX-H006",
        severity: crate::diagnostic::Severity::Error,
        message: "malformed byte receiver".into(),
        path: Some("malformed.spx".into()),
        span: Some(crate::ast::Span {
            start: 3,
            end: 7,
            line: 1,
            column: 4,
        }),
        help: Some("retain the exact malformed receiver diagnostic".into()),
    };
    let expected = semantic_error.clone();
    let ((), refused, _) = with_limit_usage(1 << 26, || {
        let owner = CfgReservation::acquire().unwrap();
        let (result, metrics) = measure(|| storage::classify::<()>(Err(semantic_error), &owner));
        let Err(storage::PlannerError::Semantic(error)) = result else {
            panic!("semantic primary")
        };
        assert_eq!(error, expected);
        assert_eq!(owner.first(), None);
        assert_eq!(metrics.allocated, 0);
        drop(error);
    });
    assert!(!refused);
    let ((actual, expected), refused, _, first) = with_limit_usage_refusal(1 << 26, || {
        let owner = CfgReservation::acquire().unwrap();
        let remaining = active_remaining().unwrap();
        let expected = crate::bounded_output::ReservationRefusal {
            requested: remaining + 1,
            remaining,
            floor: 0,
            stage: None,
        };
        assert!(!reserve_active_required(expected.requested));
        // The child selected the live ledger receipt before the CFG owner was
        // queried. Classification must latch that original receipt, not a new 0.
        assert_eq!(owner.first(), None);
        let (actual, metrics) = measure(|| {
            let Err(storage::PlannerError::Physical(Refused::Budget(actual))) =
                storage::classify::<()>(
                    Err(Diagnostic::io_static("SPX-H006", "child boundary")),
                    &owner,
                )
            else {
                panic!("typed original child receipt")
            };
            assert_eq!(owner.first(), Some(Refused::Budget(actual)));
            assert!(matches!(storage::classify(Ok(()), &owner),
                Err(storage::PlannerError::Physical(Refused::Budget(again))) if again == actual));
            actual
        });
        assert_eq!(
            (metrics.allocations, metrics.allocated, metrics.live),
            (0, 0, 0)
        );
        (actual, expected)
    });
    assert!(refused);
    assert_eq!(actual, expected);
    assert_eq!(first, Some(expected));
    let ((), refused, _) = with_limit_usage(1 << 26, || {
        let owner = CfgReservation::acquire().unwrap();
        let (((), metrics), child_refused, child_used, child_first) =
            with_limit_usage_refusal(0, || {
                measure(|| {
                    assert!(matches!(
                        storage::classify(Ok(()), &owner),
                        Err(storage::PlannerError::Physical(Refused::Detached))
                    ));
                })
            });
        assert!(!child_refused && child_used == 0 && child_first.is_none());
        assert_eq!(metrics.allocated, 0);
        let ((), metrics) = measure(|| {
            assert!(matches!(
                storage::classify(Ok(()), &owner),
                Err(storage::PlannerError::Physical(Refused::Detached))
            ));
        });
        assert_eq!(metrics.allocated, 0);
        assert_eq!(owner.first(), Some(Refused::Detached));
    });
    // The foreign call poisons its held parent owner, preserving Detached even
    // after restoration; no freshly entered child ledger supplies byte credit.
    assert!(refused);
}
