#![allow(
    clippy::duplicate_mod,
    reason = "The complete fixture intentionally compiles independently against current and predecessor owner types."
)]
//! Complete report fields, original formatters, and the actual affine owner.
use super::{stages, Owner, Setup};
use crate::bounded_output::{reserve_active_required, with_limit_usage_refusal, ReservationStage};
use crate::test_allocation_observer::measure;
use crate::workspace_graph::constructor_failure::{attempt, receiving::CoreFailure};
use crate::workspace_graph::diagnostics::{self, constructor_report_before as before};

fn exact() -> usize {
    let ((), refused, used, first) = with_limit_usage_refusal(usize::MAX, || {
        drop(Owner::acquire(usize::MAX, 0).unwrap());
    });
    assert!(!refused && first.is_none());
    used
}

#[test]
fn constructor_final_report_full_old_fields_and_all_seven_borrowed_help_branches_match() {
    for case in 0..7 {
        let _context = diagnostics::invocation_scope();
        let _precharge = (case >= 2).then(diagnostics::precharge_scope);
        diagnostics::begin_precharge_pass();
        match case {
            1 | 6 => diagnostics::record_precharge_module("a.λ\0🙂.spx", 19),
            3 => diagnostics::begin_precharge_module("pending.λ\0🙂.spx"),
            4 | 5 => diagnostics::record_sequential_precharge_refusal(
                "step.λ\0🙂.spx",
                3,
                23,
                31,
                case == 4,
            ),
            _ => {}
        }
        let expected_base = before::builder_bytes_help(usize::MAX);
        assert_eq!(
            format!("{}", diagnostics::BorrowedBuilderHelp(usize::MAX)),
            expected_base
        );
        assert_eq!(
            diagnostics::limit_error("builder_bytes", usize::MAX),
            before::limit_error("builder_bytes", usize::MAX)
        );
        let expected = before::live_builder_refusal(usize::MAX, None);
        let ((actual, physical), refused, used, first) =
            with_limit_usage_refusal(usize::MAX, || {
                measure(|| {
                    let owner = Owner::acquire(usize::MAX, 0).unwrap();
                    let pointer = owner.report.as_ptr();
                    let text = owner.message.as_ptr();
                    let help = owner.help.as_ptr();
                    let published = owner.seal().unwrap_or_else(|_| panic!("same owner"));
                    let report = published.into_report();
                    assert_eq!(report.as_ptr(), pointer);
                    assert_eq!(report[0].message.as_ptr(), text);
                    assert_eq!(report[0].help.as_ref().unwrap().as_ptr(), help);
                    assert_eq!(report.as_slice(), std::slice::from_ref(&expected));
                    drop(report);
                })
            });
        assert_eq!(actual, ());
        assert!(!refused && first.is_none());
        assert!(!physical.invalid && physical.live == 0);
        assert_eq!(physical.allocated, physical.freed);
        assert!(physical.allocated <= used);
    }
}

#[test]
fn constructor_final_report_exact_short_zero_and_sticky_keep_receipt_and_whole_drop() {
    let _context = diagnostics::invocation_scope();
    let exact = exact();
    let ((), refused, used, first) = with_limit_usage_refusal(exact, || {
        let owner = Owner::acquire(usize::MAX, 0).unwrap();
        let _stage = crate::bounded_output::reservation_stage_guard(
            ReservationStage::LoanOrderedDirectoryNode,
        );
        assert!(!reserve_active_required(1));
        drop(owner.seal().unwrap_or_else(|_| panic!("same sticky owner")));
    });
    assert!(refused && used == exact);
    let first = first.unwrap();
    assert_eq!((first.requested, first.remaining, first.floor), (1, 0, 0));
    assert_eq!(
        first.stage,
        Some(ReservationStage::LoanOrderedDirectoryNode)
    );
    let expected = before::live_builder_refusal(usize::MAX, Some(first));
    let (((), physical), refused, used, repeated) = with_limit_usage_refusal(exact, || {
        measure(|| {
            let owner = Owner::acquire(usize::MAX, 0).unwrap();
            let pointer = owner.report.as_ptr();
            let _stage = crate::bounded_output::reservation_stage_guard(
                ReservationStage::LoanOrderedDirectoryNode,
            );
            assert!(!reserve_active_required(1));
            let stopped = crate::test_allocation_observer::current_metrics().allocated;
            let report = owner
                .seal()
                .unwrap_or_else(|_| panic!("same sticky owner"))
                .into_report();
            assert_eq!(report.as_ptr(), pointer);
            assert_eq!(report.as_slice(), std::slice::from_ref(&expected));
            assert!(!reserve_active_required(0));
            assert_eq!(
                crate::test_allocation_observer::current_metrics().allocated,
                stopped
            );
            drop(report);
        })
    });
    assert!(refused && used == exact);
    assert_eq!(repeated, Some(first));
    assert!(!physical.invalid && physical.live == 0);
    assert_eq!(physical.allocated, physical.freed);
    assert!(physical.allocated <= used);
    for limit in [0, Owner::control_bytes(0).unwrap() - 1, exact - 1] {
        let ((result, physical), refused, used, first) = with_limit_usage_refusal(limit, || {
            measure(|| Owner::acquire(usize::MAX, 0).map(drop))
        });
        assert!(matches!(result, Err(Setup::Budget { .. })));
        assert!(refused && first.is_some());
        assert!(!physical.invalid && physical.live == 0);
        assert_eq!(physical.allocated, 0);
        assert_eq!(physical.freed, 0);
        assert!(used <= limit);
    }
    let (((), physical), refused, used, first) = with_limit_usage_refusal(exact, || {
        measure(|| drop(Owner::acquire(usize::MAX, 0).unwrap()))
    });
    assert!(!refused && first.is_none());
    assert_eq!(used, exact);
    assert!(!physical.invalid && physical.live == 0 && physical.allocated == physical.freed);
}

#[test]
fn constructor_final_report_foreign_restored_owner_cannot_publish_or_reset_first_detachment() {
    let _context = diagnostics::invocation_scope();
    let exact = exact();
    let ((), refused, used, first) = with_limit_usage_refusal(exact + 7, || {
        let owner = Owner::acquire(usize::MAX, 0).unwrap();
        let pointer = owner.report.as_ptr();
        let ((unpublished, physical), child_refused, child_used, child_first) =
            with_limit_usage_refusal(3, || measure(|| owner.seal().err().expect("foreign")));
        assert!(!child_refused && child_first.is_none());
        assert_eq!(child_used, 0);
        assert_eq!(physical.allocated, 0);
        assert_eq!(unpublished.refusal, Setup::Detached);
        assert_eq!(unpublished.owner.report.as_ptr(), pointer);
        let (unpublished, physical) = measure(|| unpublished.owner.seal().err().expect("restored"));
        assert_eq!(unpublished.refusal, Setup::Detached);
        assert_eq!(unpublished.owner.report.as_ptr(), pointer);
        assert_eq!(physical.allocated, 0);
        let ((), disposal) = measure(|| drop(unpublished));
        assert_eq!(disposal.allocated, 0);
        assert!(!reserve_active_required(0));
    });
    assert!(refused && used == exact);
    let first = first.unwrap();
    assert_eq!((first.requested, first.remaining, first.floor), (0, 7, 0));
}

#[test]
fn constructor_attempt_typed_reserve_seals_before_restore_and_does_not_run_after_setup_refusal() {
    let _context = diagnostics::invocation_scope();
    for limit in [0, 1] {
        let ((attempt, physical), refused, _, first) = with_limit_usage_refusal(limit, || {
            measure(|| attempt::run::<()>(limit, || panic!("setup refusal must not enter core")))
        });
        assert!(refused && first.is_some());
        assert_eq!(physical.allocated, 0);
        let (result, publication) = attempt.into_parts();
        assert!(matches!(
            result,
            Err(CoreFailure::Setup(Setup::Budget { .. }))
        ));
        assert!(publication.is_none());
    }
    let (attempt, refused, used, first) = with_limit_usage_refusal(usize::MAX, || {
        attempt::run::<()>(usize::MAX, || attempt::reserve(usize::MAX))
    });
    assert!(refused && first.is_some());
    assert!(used > 0);
    let (result, publication) = attempt.into_parts();
    assert!(matches!(
        result,
        Err(CoreFailure::Setup(Setup::Budget { .. }))
    ));
    let report = publication.expect("same original owner published before restore");
    assert_eq!(
        report.as_slice(),
        [before::live_builder_refusal(usize::MAX, first)].as_slice()
    );
}

#[test]
fn constructor_report_stage_capacity_is_closed_and_original_formatter_bodies_are_complete() {
    for (ordinal, stage) in stages::STAGES.iter().enumerate() {
        assert_eq!(*stage as usize, ordinal);
        assert!(stage.label().len() <= stages::longest().len());
    }
    assert_eq!(
        stages::STAGES.last(),
        Some(&ReservationStage::RetainedLoanSidecars)
    );
    let saved = include_str!("../../diagnostics/constructor_report_before.rs");
    let start = saved
        .find("pub(in crate::workspace_graph) fn limit_error(")
        .unwrap();
    let restored = saved[start..].replace("pub(in crate::workspace_graph) fn ", "pub(super) fn ");
    let restored = restored.replace(
        "pub(super) fn builder_bytes_help(",
        "fn builder_bytes_help(",
    );
    assert_eq!(restored, include_str!("../../../../docs/evidence/constructor-report-receiving-825/complete-report-functions-before.rs.txt"));
}

#[test]
fn constructor_final_report_capacity_failure_is_terminal_and_no_owner_is_never_a_byte_refusal() {
    assert!(crate::bounded_output::active_remaining().is_none());
    let (error, physical) = measure(|| attempt::reserve(0));
    assert!(matches!(error, Err(CoreFailure::Setup(Setup::NoOwner))));
    assert_eq!(physical.allocated, 0);
    let _context = diagnostics::invocation_scope();
    let exact = exact();
    let (((), physical), refused, used, first) = with_limit_usage_refusal(exact, || {
        measure(|| {
            let mut owner = Owner::acquire(usize::MAX, 0).unwrap();
            // Hostile private-state mutation, never an admitted producer state.
            // It exercises rollback without acquiring or truncating backing.
            while owner.help.len() < owner.help.capacity() {
                owner.help.push(' ');
            }
            let capacity = owner.help.capacity();
            let stopped = crate::test_allocation_observer::current_metrics().allocated;
            let unpublished = owner.seal().err().expect("no remaining output capacity");
            assert_eq!(unpublished.refusal, Setup::Capacity);
            assert_eq!(unpublished.owner.help.len(), capacity);
            let unpublished = unpublished
                .owner
                .seal()
                .err()
                .expect("terminal capacity failure");
            assert_eq!(unpublished.refusal, Setup::Capacity);
            assert_eq!(unpublished.owner.help.len(), capacity);
            assert_eq!(
                crate::test_allocation_observer::current_metrics().allocated,
                stopped
            );
            drop(unpublished);
        })
    });
    assert!(!refused && first.is_none());
    assert_eq!(used, exact);
    assert!(!physical.invalid && physical.live == 0 && physical.allocated == physical.freed);
}

#[test]
fn constructor_setup_compatibility_keeps_six_fields_and_never_reclassifies_storage_as_budget() {
    let _context = diagnostics::invocation_scope();
    let cases = [
        (
            Setup::NoOwner,
            "constructor report receiver has no active builder owner",
        ),
        (
            Setup::Allocation,
            "constructor report receiving allocation refused",
        ),
        (
            Setup::Capacity,
            "constructor report receiving capacity differs from its paid request",
        ),
        (
            Setup::Detached,
            "constructor report receiver is detached from its original builder owner",
        ),
        (
            Setup::Arithmetic,
            "constructor report receiving layout overflowed",
        ),
    ];
    for (setup, message) in cases {
        // This deliberately observes the remaining allocating public fallback.
        // An unacquired owner cannot certify its positive output backing.
        let (report, physical) = measure(|| CoreFailure::Setup(setup).into_legacy_unfunded());
        assert_eq!(report.len(), 1);
        let diagnostic = &report[0];
        assert_eq!(diagnostic.code, "SPX-G173");
        assert_eq!(diagnostic.severity, crate::diagnostic::Severity::Error);
        assert_eq!(diagnostic.message.as_ref(), message);
        assert_eq!(diagnostic.path, None);
        assert_eq!(diagnostic.span, None);
        assert_eq!(diagnostic.help, None);
        assert!(physical.allocated > 0);
        drop(report);
    }
    let report = CoreFailure::Setup(Setup::Budget { maximum: 17 }).into_legacy_unfunded();
    assert_eq!(
        report.as_slice(),
        [before::limit_error("builder_bytes", 17)].as_slice()
    );
}
