//! Narrow issue810 acceptance controls; full receivers remain in the parent.
use super::*;
use crate::bounded_output::{reserve_active_required, with_limit_usage_refusal};
use crate::cleanup_plan::replay::allocation::{opt_in, OracleScope};
use crate::test_allocation_observer::measure;

fn program() -> crate::hir::ResolvedProgram {
    let source =
        "module oracle.closure; @id(\"main\") fn main()->i64 {let a=if 1==1 {1} else {2}; a}";
    crate::hir::resolve(&crate::check(source, "oracle-closure.spx").unwrap()).unwrap()
}

#[test]
fn undecided_is_distinct_and_never_relabels_a_decided_mismatch() {
    let program = program();
    let function = &program.functions[0];
    // A decided false still reaches the complete original mismatch assertion.
    require_decided(function, &Ok(Some(false)));
    require_decided(function, &Ok(Some(true)));
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        require_decided(function, &Ok(None));
    }));
    let payload = outcome.err().expect("undecided must refuse visibly");
    let message = payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&str>().copied())
        .unwrap();
    assert!(message.contains("comparison is undecided"));
    assert!(!message.contains("disagrees with enumeration"));
}

#[test]
fn complete_opted_hook_keeps_predebited_and_first_sticky_production_receipts_through_drop() {
    let program = program();
    let run = |sticky, execute| {
        with_limit_usage_refusal(1 << 20, || {
            assert!(reserve_active_required(32));
            if sticky {
                assert!(!reserve_active_required((1 << 20) - 31));
            }
            let ((receipt, zero), observed) = measure(|| {
                let receipt = if execute {
                    // Same declared finite prototype account; neither fresh
                    // quota nor oracle operation replaces the live ledger.
                    let scope = OracleScope::open_with_work(1 << 26, 1 << 25).unwrap();
                    let lease = scope.lease();
                    let selected = opt_in(lease).unwrap();
                    for function in &program.functions {
                        cross_check(&program, function);
                    }
                    drop(selected);
                    Some(scope.finish())
                } else {
                    None
                };
                (receipt, reserve_active_required(0))
            });
            assert_eq!(zero, !sticky, "prior live sticky authority survives");
            assert!(!observed.invalid);
            assert_eq!(observed.live, 0);
            assert_eq!(observed.allocated, observed.freed);
            if let Some(receipt) = receipt {
                assert_eq!(receipt.started, 2 * program.functions.len());
                assert_eq!(receipt.completed, receipt.started);
                assert!(receipt.charged > 0 && receipt.work > 0);
                assert!(receipt.first_refusal.is_none());
                assert!(receipt.first_work_refusal.is_none());
                assert!(receipt.physical_failure.is_none());
                assert!(observed.allocated <= receipt.charged);
            }
        })
    };
    for sticky in [false, true] {
        let baseline = run(sticky, false);
        let actual = run(sticky, true);
        assert_eq!(
            actual, baseline,
            "entire live receipt, not only a zero counter"
        );
        assert_eq!(actual.1, sticky);
        assert_eq!(
            actual.2, 32,
            "prior cumulative usage is never consumed or reset"
        );
        if sticky {
            let first = actual.3.unwrap();
            assert_eq!(first.requested, (1 << 20) - 31);
            assert_eq!(first.remaining, (1 << 20) - 32);
        } else {
            assert!(actual.3.is_none());
        }
    }
}
