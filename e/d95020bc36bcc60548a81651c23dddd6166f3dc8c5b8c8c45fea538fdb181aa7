//! Independent equal fixtures compare the untouched Result receiver to the unit route.
use super::*;
use crate::bounded_output::{
    active_remaining, clear_active_floor, reserve_active_required, set_active_floor,
    with_limit_usage_refusal,
};
use crate::test_allocation_observer::{current_metrics, measure};
fn function() -> ResolvedFunction {
    let module = crate::check(
        "module zero.auth; @id(\"main\") fn main()->i64 {0}",
        "zero-auth.spx",
    )
    .unwrap();
    crate::hir::resolve(&module).unwrap().functions.remove(0)
}

#[test]
fn held_zero_gateway_all_production_payers_preserve_full_diagnostic_first_receipt_floor_and_drop() {
    let function = function();
    for payer in 0..3 {
        for frontier in 0..6 {
            let observe = |unit: bool| {
                with_limit_usage_refusal(1 << 20, || {
                    {
                        let mut budget = ReplayBudget::new();
                        if payer == 0 {
                            budget.payer = AllocationPayer::LegacyUnobserved;
                        } else if payer == 2 {
                            budget = ReplayBudget::with_physical(&function, 1 << 18).unwrap();
                        }
                        // Positive setup remains original and prepaid. The measured
                        // zero call must never bootstrap another refusal String.
                        if payer != 0 {
                            budget.reserve_bytes(&function, 1, "setup").unwrap();
                        }
                        let remaining = active_remaining().unwrap();
                        match frontier {
                            1 => {
                                assert!(reserve_active_required(remaining));
                            }
                            2 => {
                                assert!(set_active_floor(remaining));
                            }
                            3 => {
                                assert!(set_active_floor(remaining));
                                let (_, _, used, _) = with_limit_usage_refusal(1, || {
                                    assert!(reserve_active_required(1))
                                });
                                assert_eq!(used, 1);
                            }
                            4 => {
                                assert!(!reserve_active_required(remaining + 1));
                            }
                            5 => {
                                if payer != 0 {
                                    drop(budget.budget_refusal(
                                        &function,
                                        "selected earlier refusal",
                                        "",
                                    ));
                                    assert!(budget.refusal_spent && budget.refusal_text.is_none());
                                }
                            }
                            _ => {}
                        }
                        let at = current_metrics().allocated;
                        let mut held = None;
                        if unit {
                            budget.authenticate_zero_into(&function, "parity phase", &mut held);
                        } else {
                            held = budget.reserve_bytes(&function, 0, "parity phase").err();
                        }
                        assert_eq!(current_metrics().allocated, at);
                        let result = (
                            held,
                            budget.remaining,
                            budget.skeleton_remaining,
                            budget.refusal_spent,
                            budget.refusal_text.is_none(),
                            budget.physical.as_ref().map(|physical| physical.charged()),
                            active_remaining(),
                        );
                        clear_active_floor();
                        drop(budget);
                        result
                    }
                })
            };
            let ((), allocation) = measure(|| {
                let old = observe(false);
                let new = observe(true);
                assert_eq!(old.0, new.0, "payer={payer} frontier={frontier}");
                assert_eq!((old.1, old.2, old.3), (new.1, new.2, new.3));
                drop(old);
                drop(new);
            });
            assert!(
                !allocation.invalid
                    && allocation.live == 0
                    && allocation.allocated == allocation.freed
            );
        }
    }
}

#[test]
fn held_zero_gateway_existing_six_field_slot_is_never_overwritten_or_queried() {
    let function = function();
    let mut budget = ReplayBudget::new();
    budget.payer = AllocationPayer::ActiveBuilder;
    let mut expected = Diagnostic::io("SPX-H006", "preselected complete error".to_owned());
    expected.severity = crate::diagnostic::Severity::Warning;
    expected.path = Some("held.spx".to_owned());
    expected.span = Some(crate::ast::Span {
        start: 1,
        end: 3,
        line: 2,
        column: 4,
    });
    expected.help = Some("complete held help".to_owned());
    let mut held = Some(expected.clone());
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(0, || {
        measure(|| {
            budget.authenticate_zero_into(&function, "must not run", &mut held);
        })
    });
    assert_eq!(held, Some(expected));
    assert!(!refused && paid == 0 && first.is_none() && allocation.allocated == 0);
}

#[test]
fn held_zero_gateway_oracle_exact_attempts_sticky_expired_and_zero_owner_parity() {
    let function = function();
    for scenario in 0..4 {
        let observe = |unit| {
            let scope = allocation::OracleScope::open_with_work(4096, 64).unwrap();
            let lease = scope.lease();
            let mut budget = ReplayBudget::with_skeleton_limit_oracle(0, lease);
            budget.reserve_bytes(&function, 1, "oracle setup").unwrap();
            match scenario {
                1 => {
                    assert!(lease.charge(4096).is_err());
                }
                2 => {
                    drop(budget.budget_refusal(&function, "selected earlier oracle refusal", ""));
                    assert!(budget.refusal_spent && budget.refusal_text.is_none());
                }
                3 => {
                    lease.fail_physical("original first physical refusal");
                }
                _ => {}
            }
            let mut held = None;
            if unit {
                budget.authenticate_zero_into(&function, "oracle parity", &mut held);
            } else {
                held = budget.reserve_bytes(&function, 0, "oracle parity").err();
            }
            let receipt = scope.receipt();
            drop(budget);
            drop(scope);
            (held, receipt)
        };
        assert_eq!(observe(false), observe(true));
    }
    // Expired tokens must not charge the replacement live account.
    let expired = {
        let scope = allocation::OracleScope::open(0).unwrap();
        scope.lease()
    };
    let observe = |unit| {
        let scope = allocation::OracleScope::open(0).unwrap();
        let mut budget = ReplayBudget::with_skeleton_limit_oracle(0, expired);
        let mut held = None;
        if unit {
            budget.authenticate_zero_into(&function, "expired", &mut held);
        } else {
            held = budget.reserve_bytes(&function, 0, "expired").err();
        }
        let receipt = scope.receipt();
        drop(budget);
        drop(scope);
        (held, receipt)
    };
    assert_eq!(observe(false), observe(true));
}

#[test]
fn held_zero_gateway_fresh_zero_oracle_never_bootstraps_text_or_active_payer() {
    let function = function();
    let scope = allocation::OracleScope::open(0).unwrap();
    let lease = scope.lease();
    let mut budget = ReplayBudget::with_skeleton_limit_oracle(0, lease);
    let mut held = None;
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(0, || {
        measure(|| {
            budget.authenticate_zero_into(&function, "fresh zero oracle", &mut held);
            assert!(held.is_none() && budget.refusal_text.is_none() && !budget.refusal_spent);
        })
    });
    assert!(!refused && paid == 0 && first.is_none() && allocation.allocated == 0);
    assert_eq!(scope.receipt().calls, 1);
    assert_eq!(scope.receipt().charged, 0);
}
