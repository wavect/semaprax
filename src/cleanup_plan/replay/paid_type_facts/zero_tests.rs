//! Zero-byte authentication on the real classifier payer. A healthy zero request
//! constructs nothing; positive requests keep the original bootstrap and charges.
use super::tests::{fixtures, payer};
use super::*;
use crate::bounded_output::{
    active_refused, reserve_active_required, set_active_floor, with_limit,
    with_limit_usage_refusal, CappedString,
};
use crate::test_allocation_observer::{current_metrics, measure};

#[test]
fn healthy_zero_authenticates_without_constructing_refusal_text() {
    let program = fixtures().into_iter().next().unwrap();
    let function = &program.functions[0];
    let (((), metrics), refused, used, receipt) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            assert_eq!(budget.payer, AllocationPayer::ActiveBuilder);
            payer(function, &mut budget).bytes(0).unwrap();
            payer(function, &mut budget).bytes(0).unwrap();
            assert!(budget.refusal_text.is_none() && !budget.refusal_spent);
            drop(budget);
        })
    });
    assert!(!refused && receipt.is_none() && used == 0);
    assert!(!metrics.invalid && metrics.allocations == 0 && metrics.live == 0);
    assert_eq!(metrics.allocated, 0);
    assert_eq!(metrics.allocated, metrics.freed);
    // Even a zero limit admits a healthy zero request.
    let (((), metrics), refused, used, receipt) = with_limit_usage_refusal(0, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            payer(function, &mut budget).bytes(0).unwrap();
            drop(budget);
        })
    });
    assert!(!refused && receipt.is_none() && used == 0 && metrics.allocations == 0);
    assert!(!metrics.invalid && metrics.live == 0);
    assert_eq!(metrics.allocated, metrics.freed);
}

#[test]
fn positive_requests_keep_exact_bootstrap_with_or_without_a_zero_touch() {
    let program = fixtures().into_iter().next().unwrap();
    let function = &program.functions[0];
    let text = function.id.as_str().len() + 512;
    let run = |limit: usize, zero_first: bool| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                if zero_first {
                    payer(function, &mut budget).bytes(0).unwrap();
                }
                let result = payer(function, &mut budget).bytes(8);
                let ok = result.is_ok();
                drop(result);
                drop(budget);
                ok
            })
        })
    };
    for limit in [text - 1, text, text + 7, text + 8] {
        let plain = run(limit, false);
        assert_eq!(plain, run(limit, true), "zero touch changed limit {limit}");
        let ((ok, metrics), refused, used, receipt) = plain;
        assert!(!metrics.invalid && metrics.live == 0);
        assert_eq!(metrics.allocated, metrics.freed);
        match limit {
            l if l == text + 8 => {
                assert!(ok && !refused && used == text + 8 && receipt.is_none());
                assert_eq!(metrics.allocated, text);
            }
            l if l == text - 1 => {
                assert!(!ok && refused && used == 0 && metrics.allocated == 0);
                assert_eq!(receipt.unwrap().requested, text);
            }
            _ => {
                assert!(!ok && refused && used == text);
                assert_eq!(receipt.unwrap().requested, 8);
            }
        }
    }
}

#[test]
fn pre_refused_zero_stays_sticky_and_retries_allocate_nothing() {
    let program = fixtures().into_iter().next().unwrap();
    let function = &program.functions[0];
    let (((), metrics), refused, used, receipt) = with_limit_usage_refusal(0, || {
        measure(|| {
            assert!(!reserve_active_required(1));
            let mut budget = ReplayBudget::new();
            let first = payer(function, &mut budget).bytes(0).unwrap_err();
            let before = current_metrics();
            let retry = payer(function, &mut budget).bytes(0).unwrap_err();
            assert_eq!(first, Diagnostic::io_static("SPX-H006", ""));
            assert_eq!(first, retry);
            assert_eq!(current_metrics(), before);
            drop((first, retry, budget));
        })
    });
    assert!(refused && receipt.is_some() && used == 0);
    assert!(!metrics.invalid && metrics.allocated == 0 && metrics.live == 0);
}

#[test]
fn floors_and_child_breach_decide_zero_with_the_same_selection() {
    let program = fixtures().into_iter().next().unwrap();
    let function = &program.functions[0];
    let (((), metrics), refused, used, receipt) = with_limit_usage_refusal(10, || {
        assert!(set_active_floor(6));
        measure(|| {
            let mut budget = ReplayBudget::new();
            payer(function, &mut budget).bytes(0).unwrap();
            assert!(budget.refusal_text.is_none());
            drop(budget);
        })
    });
    assert!(!refused && receipt.is_none() && used == 0 && metrics.allocations == 0);
    // A child that breaches the parent's floor leaves the parent refused.
    let (((), metrics), refused, used, receipt) = with_limit_usage_refusal(10, || {
        assert!(set_active_floor(8));
        let (_, child_refused) = with_limit(4, || {
            let mut output = CappedString::new();
            output.push_str("abcd");
        });
        assert!(!child_refused);
        assert!(active_refused());
        measure(|| {
            let mut budget = ReplayBudget::new();
            let first = payer(function, &mut budget).bytes(0).unwrap_err();
            assert_eq!(first, Diagnostic::io_static("SPX-H006", ""));
            drop((first, budget));
        })
    });
    assert!(refused && receipt.is_some() && used == 4 && metrics.allocations == 0);
}

#[test]
fn stale_or_nested_ledger_zero_is_still_refused_before_allocation() {
    let program = fixtures().into_iter().next().unwrap();
    let function = &program.functions[0];
    let (((), metrics), refused, _, _) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            let mut held = payer(function, &mut budget);
            let ((), _, _, _) = with_limit_usage_refusal(usize::MAX, || {
                let before = current_metrics();
                let result = held.bytes(0);
                assert_eq!(result, Err(Diagnostic::io_static("SPX-H006", "")));
                drop(result);
                assert_eq!(before.allocated, current_metrics().allocated);
            });
            drop(held);
            drop(budget);
        })
    });
    assert!(refused && !metrics.invalid && metrics.live == 0);
    assert_eq!(metrics.allocated, metrics.freed);
}

#[test]
fn selected_and_legacy_zero_requests_keep_their_original_routes() {
    let program = fixtures().into_iter().next().unwrap();
    let function = &program.functions[0];
    let selected_error = Diagnostic::io(
        "SPX-H006",
        format!(
            "cleanup plan for function `{}` failed independent replay: cleanup replay physical reservation exhausted during selected sticky",
            function.id
        ),
    );
    let ((), metrics) = measure(|| {
        let mut selected = ReplayBudget::with_physical(function, 1 << 20).unwrap();
        assert!(selected.refusal_text.is_some());
        let before = current_metrics();
        selected
            .reserve_bytes(function, 0, "selected zero")
            .unwrap();
        assert_eq!(current_metrics(), before);
        assert!(selected.refusal_text.is_some());
        selected.physical.as_deref_mut().unwrap().reject();
        let first = selected
            .reserve_bytes(function, 0, "selected sticky")
            .unwrap_err();
        assert_eq!(first, selected_error);
        let before_retry = current_metrics();
        let retry = selected
            .reserve_bytes(function, 0, "selected sticky")
            .unwrap_err();
        assert_eq!(retry, Diagnostic::io_static("SPX-H006", ""));
        assert_eq!(before_retry, current_metrics());
        drop((first, retry));
        drop(selected);
    });
    assert!(!metrics.invalid && metrics.live == 0);
    assert_eq!(metrics.allocated, metrics.freed);
    let (((), metrics), _, used, _) = with_limit_usage_refusal(0, || {
        measure(|| {
            assert!(!reserve_active_required(1));
            let mut legacy = ReplayBudget::unobserved();
            legacy.reserve_bytes(function, 0, "legacy zero").unwrap();
            drop(legacy);
        })
    });
    assert!(used == 0 && metrics.allocations == 0 && metrics.live == 0);
}
