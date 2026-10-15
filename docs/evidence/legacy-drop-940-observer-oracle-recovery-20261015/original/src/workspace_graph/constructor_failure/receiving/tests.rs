//! Actual receiving slot; legacy helper/public-output funding stays separate.
use super::{CoreFailure, Receiver, Setup, before};
use crate::bounded_output::{reserve_active_required, with_limit_usage_refusal};
use crate::diagnostic::Diagnostic;
use crate::test_allocation_observer::measure;
use crate::workspace_graph::constructor_failure::Failure;
use std::mem::size_of;
fn primary() -> Diagnostic {
    Diagnostic::io_static("SPX-G173", "primary constructor failure")
}

#[test]
fn constructor_receiving_exact_short_zero_preserves_full_primary_pointer_and_whole_drop() {
    let exact = Receiver::setup_bytes().unwrap();
    for limit in [0, exact - 1, exact] {
        let ((outcome, physical), refused, used, first) = with_limit_usage_refusal(limit, || {
            measure(|| {
                let receiver = Receiver::acquire()?;
                let pointer = receiver.report.as_ptr();
                let error = receiver.receive(Failure::One(primary()));
                let report = error.into_legacy_unfunded();
                assert_eq!(report.as_ptr(), pointer);
                assert_eq!(report.capacity(), 1);
                assert_eq!(report.as_slice(), [primary()].as_slice());
                drop(report);
                Ok::<_, Setup>(())
            })
        });
        assert_eq!(outcome.is_ok(), limit == exact);
        assert_eq!(refused, limit < exact);
        assert_eq!(used, if limit == exact { exact } else { 0 });
        assert_eq!(first.is_some(), limit < exact);
        assert!(!physical.invalid);
        assert_eq!(physical.allocated, physical.freed);
        assert_eq!(physical.live, 0);
        assert_eq!(
            physical.allocated,
            if limit == exact {
                size_of::<Diagnostic>()
            } else {
                0
            }
        );
        assert!(physical.allocated <= used);
    }
    let (((), physical), refused, used, first) = with_limit_usage_refusal(exact, || {
        measure(|| {
            let receiver = Receiver::acquire().unwrap();
            drop(receiver);
        })
    });
    assert!(!refused && first.is_none());
    assert_eq!(used, exact);
    assert!(!physical.invalid && physical.live == 0 && physical.allocated == physical.freed);
}
#[test]
fn constructor_receiving_after_sticky_moves_existing_slot_without_a_new_debit_or_allocation() {
    let exact = Receiver::setup_bytes().unwrap();
    let (((), physical), refused, used, first) = with_limit_usage_refusal(exact, || {
        measure(|| {
            let receiver = Receiver::acquire().unwrap();
            let pointer = receiver.report.as_ptr();
            assert!(!reserve_active_required(1));
            let report = receiver
                .receive(Failure::One(primary()))
                .into_legacy_unfunded();
            assert_eq!(report.as_ptr(), pointer);
            assert_eq!(report.as_slice(), [primary()].as_slice());
            assert!(!reserve_active_required(0));
            drop(report);
        })
    });
    assert!(refused);
    assert_eq!(used, exact);
    let first = first.unwrap();
    assert_eq!((first.requested, first.remaining, first.floor), (1, 0, 0));
    assert!(!physical.invalid && physical.live == 0 && physical.allocated == physical.freed);
    assert_eq!(physical.allocated, size_of::<Diagnostic>());
}
#[test]
fn constructor_receiving_foreign_then_restored_owner_retains_primary_and_never_reactivates() {
    let exact = Receiver::setup_bytes().unwrap();
    let ((), refused, used, first) = with_limit_usage_refusal(exact + 7, || {
        let receiver = Receiver::acquire().unwrap();
        let pointer = receiver.report.as_ptr();
        let ((error, physical), child_refused, child_used, child_first) =
            with_limit_usage_refusal(3, || measure(|| receiver.receive(Failure::One(primary()))));
        assert!(!child_refused && child_first.is_none());
        assert_eq!(child_used, 0);
        assert_eq!(physical.allocated, 0);
        let CoreFailure::Unreceived {
            primary: kept,
            owner,
            refusal,
        } = error
        else {
            panic!("foreign owner must keep affine primary")
        };
        assert_eq!(refusal, Setup::Detached);
        assert_eq!(owner.report.as_ptr(), pointer);
        assert_eq!(kept.diagnostics(), [primary()].as_slice());
        assert!(!reserve_active_required(0));
        let (error, physical) = measure(|| owner.receive(kept));
        assert_eq!(physical.allocated, 0);
        let CoreFailure::Unreceived {
            primary: kept,
            owner,
            refusal,
        } = error
        else {
            panic!("restoration never resets detachment")
        };
        assert_eq!(refusal, Setup::Detached);
        assert_eq!(owner.report.as_ptr(), pointer);
        assert_eq!(kept.diagnostics(), [primary()].as_slice());
        let (_, disposal) = measure(|| {
            drop(kept);
            drop(owner);
        });
        assert_eq!(disposal.allocated, 0);
    });
    assert!(refused);
    assert_eq!(used, exact);
    let first = first.unwrap();
    assert_eq!((first.requested, first.remaining, first.floor), (0, 7, 0));
}
#[test]
fn constructor_receiving_many_moves_complete_order_and_backing_without_paid_reclassification() {
    let exact = Receiver::setup_bytes().unwrap();
    let reports = vec![
        Diagnostic::error("SPX-G173", "first.λ\0🙂", crate::ast::Span::default())
            .at_path("a.spx")
            .with_help("help.🙂"),
        primary(),
    ];
    let expected = reports.clone();
    let pointer = reports.as_ptr();
    let capacity = reports.capacity();
    let (report, refused, used, first) = with_limit_usage_refusal(exact, || {
        let owner = Receiver::acquire().unwrap();
        let (error, physical) = measure(|| owner.receive(Failure::Many(reports)));
        assert_eq!(physical.allocated, 0);
        assert!(matches!(error, CoreFailure::Legacy(_)));
        error.into_legacy_unfunded()
    });
    assert!(!refused && first.is_none());
    assert_eq!(used, exact);
    assert_eq!(report, expected);
    assert_eq!(report.as_ptr(), pointer);
    assert_eq!(report.capacity(), capacity);
    assert_eq!(
        report.iter().map(Diagnostic::json).collect::<Vec<_>>(),
        expected.iter().map(Diagnostic::json).collect::<Vec<_>>()
    );
    drop(report);
}
#[test]
fn constructor_receiving_complete_old_core_body_and_actual_linked_programs_remain_independent() {
    let saved = include_str!("before.rs");
    let start = saved
        .find("pub(in crate::workspace_graph) fn build_resolved_core_current(")
        .unwrap();
    let restored = saved[start..].replacen(
        "pub(in crate::workspace_graph) fn build_resolved_core_current(",
        "fn build_resolved_core_current(",
        1,
    );
    assert_eq!(
        restored,
        include_str!(
            "../../../../docs/evidence/constructor-report-receiving-825/complete-core-before.rs.txt"
        )
    );
    let sources = || {
        vec![
        crate::workspace_graph::WorkspaceSource{path:"lib/value.spx".to_owned(),source:"module lib.value; @id(\"value\") fn value()->i64{41}".to_owned()},
        crate::workspace_graph::WorkspaceSource{path:"app/main.spx".to_owned(),source:"module app.main; use function @id(\"value\") from lib.value as value; @id(\"app.main\") fn main()->i64{value()+1}".to_owned()},
        crate::workspace_graph::WorkspaceSource{path:"test/main.spx".to_owned(),source:"module test.main; use function @id(\"value\") from lib.value as value; @id(\"test.main\") fn main()->i64{value()+2}".to_owned()},
    ]
    };
    let old = before::with_before(|| {
        crate::workspace_graph::build_owned(sources())
            .unwrap()
            .into_linked_scalar_programs("app.main", "test.main")
            .unwrap()
    });
    let current = crate::workspace_graph::build_owned(sources())
        .unwrap()
        .into_linked_scalar_programs("app.main", "test.main")
        .unwrap();
    assert_eq!(current, old);
    assert_eq!(
        crate::codegen::emit_hir_c(&current.0).unwrap(),
        crate::codegen::emit_hir_c(&old.0).unwrap()
    );
    assert_eq!(
        crate::codegen::emit_hir_c(&current.1).unwrap(),
        crate::codegen::emit_hir_c(&old.1).unwrap()
    );
    // Builder debit metadata deliberately differs: the real unused receiving
    // slot is acquired/charged on success. No budget field is normalized.
}
