use super::*;
use crate::bounded_output::{with_limit_usage_refusal, ReservationStage};
use crate::test_allocation_observer::{current_metrics, measure};
const MAXIMUM: usize = 16384;
fn install(branch: usize) {
    BUILDER_PHASE.with(|v| {
        v.set(if branch < 2 {
            BuilderPhase::Live
        } else {
            BuilderPhase::Precharge
        })
    });
    PRECHARGE_MODULES.with(|v| v.set(7));
    PRECHARGE_BYTES.with(|v| v.set(9271));
    if matches!(branch, 1 | 3) {
        PRECHARGE_DOMINANT.with(|v| *v.borrow_mut() = Some(("provider/é\0.spx".to_owned(), 4007)));
    }
    if branch == 4 {
        PRECHARGE_PENDING.with(|v| *v.borrow_mut() = Some("pending/é\0.spx".to_owned()));
    }
    if branch >= 5 {
        PRECHARGE_SEQUENTIAL.with(|v| {
            *v.borrow_mut() = Some(SequentialRefusal {
                path: "sequence/é\0.spx".to_owned(),
                earlier: 3,
                retained: 111,
                additional: 222,
                resolving: branch == 5,
            })
        });
    }
}
fn first() -> Option<ReservationRefusal> {
    Some(ReservationRefusal {
        requested: 1234,
        remaining: 17,
        floor: 5,
        stage: Some(ReservationStage::CorePrebound),
    })
}

#[test]
fn every_help_branch_matches_all_six_fields_and_partial_parent_receipts() {
    for branch in 0..7 {
        for live in [false, true] {
            for limit in [0, 1, 63, 127, 255, 511, 1023, 2047, 4095] {
                let old = with_limit_usage_refusal(limit, || {
                    let scope = invocation_scope();
                    install(branch);
                    let diagnostic = if live {
                        live_builder_refusal_at(MAXIMUM, first(), None)
                    } else {
                        limit_error("builder_bytes", MAXIMUM)
                    };
                    drop(scope);
                    diagnostic
                });
                let current = with_limit_usage_refusal(limit, || {
                    let scope = invocation_scope();
                    install(branch);
                    let captured = if live {
                        Captured::live(MAXIMUM, first(), None)
                    } else {
                        Captured::limit(MAXIMUM)
                    };
                    drop(scope);
                    // Explicit ordinary IO materialization doesn't reserve or
                    // read any later TLS. It is not claimed physically funded.
                    let diagnostic = captured.materialize_unfunded();
                    drop(captured);
                    diagnostic
                });
                assert_eq!(old, current, "branch={branch} live={live} limit={limit}");
            }
        }
    }
}

#[test]
fn moved_context_and_whole_drop_need_no_report_backing_or_clone() {
    for branch in 0..7 {
        let (((), physical), refused, _, first) = with_limit_usage_refusal(65536, || {
            measure(|| {
                let scope = invocation_scope();
                install(branch);
                let pointers =
                    PRECHARGE_DOMINANT.with(|v| v.borrow().as_ref().map(|x| x.0.as_ptr()));
                let before = current_metrics().allocations;
                let captured = Captured::live(MAXIMUM, None, None);
                assert_eq!(current_metrics().allocations, before);
                assert_eq!(
                    captured.context.as_ref().unwrap().dominant_pointer(),
                    pointers
                );
                drop(scope);
                drop(captured);
            })
        });
        assert!(!refused && first.is_none());
        assert!(!physical.invalid && physical.live == 0 && physical.allocated == physical.freed);
    }
}

#[test]
fn foreign_restored_tls_and_output_budgets_cannot_rewrite_captured_fields() {
    for branch in 0..7 {
        let expected = {
            let scope = invocation_scope();
            install(branch);
            let value = live_builder_refusal_at(MAXIMUM, first(), None);
            drop(scope);
            value
        };
        let held = {
            let scope = invocation_scope();
            install(branch);
            let value = Captured::live(MAXIMUM, first(), None);
            drop(scope);
            value
        };
        let decoy = invocation_scope();
        install((branch + 1) % 7);
        let ((), refused, used, receipt) = with_limit_usage_refusal(0, || {
            assert_eq!(held.materialize_unfunded(), expected);
        });
        assert!(!refused && used == 0 && receipt.is_none());
        for limit in [0, 3, 31, 127, 1024] {
            let old = with_limit_usage_refusal(limit, || {
                let mut out = String::new();
                expected.write_json(&mut out).unwrap();
                out
            });
            let current = with_limit_usage_refusal(limit, || {
                let mut out = String::new();
                held.write_json(&mut out).unwrap();
                out
            });
            assert_eq!(old, current);
        }
        drop(decoy);
        assert_eq!(held.materialize_unfunded(), expected);
    }
}
