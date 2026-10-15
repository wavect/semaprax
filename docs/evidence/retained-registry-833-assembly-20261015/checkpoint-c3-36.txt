use super::*;

#[test]
fn live_builder_evidence_preserves_message_and_names_only_the_known_phase() {
    let (_, overflowed, used, refusal) = crate::bounded_output::with_limit_usage_refusal(5, || {
        assert!(crate::bounded_output::reserve_active_required(2));
        assert!(!crate::bounded_output::reserve_active_required(4));
    });
    assert!(overflowed);
    assert_eq!(used, 2);
    let diagnostic = live_builder_refusal(5, refusal);
    assert_eq!(diagnostic.code, "SPX-G171");
    assert_eq!(
        diagnostic.message,
        "Workspace Semantic Graph `builder_bytes` exceeds 5"
    );
    assert!(diagnostic.path.is_none());
    assert!(diagnostic.span.is_none());
    let help = diagnostic.help.unwrap();
    assert!(help.contains("resolved-core phase first sticky ledger refusal: requested 4 bytes with 3 remaining and 0 reserved floor."));
    assert!(help.contains("Authenticated internal stage: unknown."));
    assert!(help
        .contains("cumulative reservation bytes, not a retained-memory forecast or process RSS"));
    assert!(help.contains("exact inner operation is unknown"));
    assert!(help.len() < 4096);
    let unknown = live_builder_refusal(5, None).help.unwrap();
    assert!(unknown.contains("first reservation and exact inner operation are unknown"));
}

#[test]
fn live_builder_help_names_only_a_scoped_compiler_stage() {
    let (_, overflowed, _, refusal) = crate::bounded_output::with_limit_usage_refusal(5, || {
        crate::bounded_output::with_reservation_stage(
            crate::bounded_output::ReservationStage::FreshResolution,
            || assert!(!crate::bounded_output::reserve_active_required(6)),
        );
    });
    assert!(overflowed);
    let help = live_builder_refusal(5, refusal).help.unwrap();
    assert!(help.contains("Authenticated internal stage: fresh HIR resolution."));
    assert!(help.contains("exact inner operation is unknown"));
}

#[test]
fn live_core_stage_receipts_retain_the_actual_request_and_residual() {
    use crate::bounded_output::{self, ReservationStage as Stage};
    for stage in [
        Stage::ResolvedCore,
        Stage::CompactValidation,
        Stage::UncachedSyntheticPeak,
        Stage::AuthoredModuleValidation,
        Stage::FrontendLookup,
        Stage::SelectiveResolution,
        Stage::FrontendRetention,
        Stage::ImportedVecInstances,
    ] {
        let (_, overflowed, used, refusal) = bounded_output::with_limit_usage_refusal(755, || {
            bounded_output::with_reservation_stage(Stage::ResolvedCore, || {
                bounded_output::with_reservation_stage(stage, || {
                    assert!(!bounded_output::reserve_active_required(2048));
                });
                assert!(!bounded_output::reserve_active_required(4096));
            });
        });
        assert!(overflowed);
        assert_eq!(used, 0);
        let diagnostic = live_builder_refusal(super::super::MAX_BUILDER_BYTES, refusal);
        assert_eq!(diagnostic.code, "SPX-G171");
        assert!(diagnostic.path.is_none());
        assert!(diagnostic.span.is_none());
        let help = diagnostic.help.unwrap();
        assert!(help.contains("requested 2048 bytes with 755 remaining and 0 reserved floor"));
        assert!(help.contains(&format!("Authenticated internal stage: {}.", stage.label())));
        assert!(help.contains("exact inner operation is unknown"));
        assert!(help.len() < 4096);
    }
}

#[test]
fn canonical_prepare_help_changes_only_stage_keeps_inner_unknown_and_paid_omission() {
    use crate::bounded_output::{self, ReservationRefusal, ReservationStage};
    let legacy = ReservationRefusal {
        requested: 64,
        remaining: 32,
        floor: 8,
        stage: None,
    };
    let current = ReservationRefusal {
        stage: Some(ReservationStage::CanonicalLoanStorage),
        ..legacy
    };
    let old = live_builder_refusal(1024, Some(legacy));
    let mut new = live_builder_refusal(1024, Some(current));
    let help = new.help.as_ref().unwrap();
    assert!(help.contains("requested 64 bytes with 32 remaining and 8 reserved floor"));
    assert!(
        help.contains("Authenticated internal stage: canonical shared-loan storage preparation.")
    );
    assert!(help.contains("exact inner operation is unknown"));
    new.help = new.help.map(|help| {
        help.replace(
            "Authenticated internal stage: canonical shared-loan storage preparation.",
            "Authenticated internal stage: unknown.",
        )
    });
    assert_eq!(new, old); // all six Diagnostic fields, not just primary text

    let (_, overflow, base_paid) = bounded_output::with_limit_usage(usize::MAX, || {
        drop(limit_error("builder_bytes", 1024));
    });
    assert!(!overflow && base_paid > 0);
    for limit in [base_paid, base_paid - 1, 0] {
        let baseline =
            bounded_output::with_limit_usage_refusal(limit, || limit_error("builder_bytes", 1024));
        let rendered = bounded_output::with_limit_usage_refusal(limit, || {
            live_builder_refusal(1024, Some(current))
        });
        assert_eq!(rendered.0, baseline.0);
        assert_eq!(rendered.2, baseline.2);
        assert!(rendered.1);
    }
    let (((), metrics), _, _) = bounded_output::with_limit_usage(usize::MAX, || {
        crate::test_allocation_observer::measure(|| {
            drop(live_builder_refusal(1024, Some(current)));
        })
    });
    assert!(!metrics.invalid);
    assert_eq!(metrics.allocated, metrics.freed);
    assert_eq!(metrics.live, 0);
}

#[test]
fn final_attempt_help_never_imports_canonical_label_from_an_earlier_refused_attempt() {
    use crate::bounded_output::{self, ReservationStage};
    let earlier = bounded_output::with_limit_usage_refusal(1, || {
        bounded_output::with_reservation_stage(ReservationStage::CanonicalLoanStorage, || {
            assert!(!bounded_output::reserve_active_required(2));
        });
    });
    assert_eq!(
        earlier.3.unwrap().stage,
        Some(ReservationStage::CanonicalLoanStorage)
    );
    let final_attempt = bounded_output::with_limit_usage_refusal(3, || {
        assert!(bounded_output::reserve_active_required(1));
        assert!(!bounded_output::reserve_active_required(4));
    });
    assert_eq!((final_attempt.1, final_attempt.2), (true, 1));
    let help = live_builder_refusal(3, final_attempt.3).help.unwrap();
    assert!(help.contains("requested 4 bytes with 2 remaining and 0 reserved floor"));
    assert!(help.contains("Authenticated internal stage: unknown."));
    assert!(help.contains("exact inner operation is unknown"));
    assert!(!help.contains("canonical shared-loan storage preparation"));
}
