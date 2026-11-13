//! Distinct owned and identity gateways keep their original ledger boundaries.
use super::super::{before_scope_memo, before_terminal_leaf_empty_memo};
use super::*;
use crate::bounded_output::{active_remaining, with_limit_usage_refusal_site};
use crate::test_allocation_observer::{current_metrics, measure};

#[test]
fn owned_gateway_refuses_only_retained_parent_while_identity_gateway_stays_inert() {
    let function = super::super::super::lexical_plan_tests::fixture(
        super::super::super::lexical_plan_tests::WHILE,
    )
    .1;
    // Original owned producer, new owned gateway, immediate identity producer,
    // and unchanged shared identity gateway all remain independent witnesses.
    for route in 0..4 {
        for child_limit in [0, 1 << 26] {
            let (((), metrics), refused, _, first, site) =
                with_limit_usage_refusal_site(1 << 26, || {
                    measure(|| {
                        let mut budget = ReplayBudget::new();
                        macro_rules! exercise {
                            ($memo:expr, $gateway:ident) => {{
                                let mut memo = $memo;
                                memo.authenticate(&function, &mut budget).unwrap();
                                memo.insert_empty_bound(&function, 0, &mut budget).unwrap();
                                assert_eq!(memo.state(&0), 1);
                                let remaining = active_remaining();
                                let work = (budget.remaining, budget.skeleton_remaining);
                                let mut allocated = 0;
                                let ((), child_refused, child_paid, child_first, child_site) =
                                    with_limit_usage_refusal_site(child_limit, || {
                                        let child_remaining = active_remaining();
                                        allocated = current_metrics().allocated;
                                        assert!(memo.$gateway(&function, &budget));
                                        assert!(memo.failed());
                                        assert_eq!(memo.state(&0), 0);
                                        assert_eq!(active_remaining(), child_remaining);
                                        assert_eq!(current_metrics().allocated, allocated);
                                    });
                                assert!(!child_refused && child_paid == 0);
                                assert!(child_first.is_none() && child_site.is_none());
                                assert_eq!(active_remaining(), remaining);
                                assert_eq!((budget.remaining, budget.skeleton_remaining), work);
                                // The restored parent cannot revive either API's views.
                                assert!(!memo.$gateway(&function, &budget));
                                assert_eq!(memo.state(&0), 0);
                                assert_eq!(
                                    memo.authenticate(&function, &mut budget),
                                    Err(Diagnostic::io("SPX-H006", String::new()))
                                );
                                assert_eq!(active_remaining(), remaining);
                                assert_eq!((budget.remaining, budget.skeleton_remaining), work);
                                assert_eq!(current_metrics().allocated, allocated);
                                drop(memo);
                            }};
                        }
                        match route {
                            0 => exercise!(
                                before_terminal_leaf_empty_memo::Memo::<()>::new(),
                                preflight
                            ),
                            1 => exercise!(Memo::<()>::new(), preflight_owned),
                            2 => exercise!(before_scope_memo::Memo::<()>::new(), preflight),
                            3 => exercise!(Memo::<()>::new(), preflight),
                            _ => unreachable!(),
                        }
                        drop(budget);
                    })
                });
            assert_eq!(refused, route < 2);
            assert_eq!(first.is_some(), route < 2);
            assert_eq!(site.is_some(), route < 2);
            assert!(!metrics.invalid && metrics.live == 0);
            assert_eq!(metrics.allocated, metrics.freed);
        }
    }
}

#[test]
fn immediate_memo_is_complete_and_shared_identity_entries_are_unchanged() {
    let before = include_str!("../before_scope_memo.rs");
    let archived = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/opt831-owned-scope-gateway-20261017/empty_memo.whole-before.txt"
    ));
    assert_eq!(before, archived);
    let current = include_str!("../empty_memo.rs");
    let start = "    pub(super) fn authenticate(";
    let end = "    // Canonical owned validation";
    let old_identity = before
        .split_once(start)
        .unwrap()
        .1
        .split_once("    pub(super) fn unpublished")
        .unwrap()
        .0;
    let current_identity = current
        .split_once(start)
        .unwrap()
        .1
        .split_once(end)
        .unwrap()
        .0;
    assert_eq!(
        current_identity, old_identity,
        "complete authenticate and preflight bodies"
    );
}
