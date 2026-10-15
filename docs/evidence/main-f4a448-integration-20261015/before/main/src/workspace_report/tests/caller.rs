use super::*;

#[test]
fn legacy_unlock_precedence_moves_override_and_retains_paid_primary_through_drop() {
    let program = crate::hir::resolve(
        &crate::check(
            "module report.unlock; @id(\"main\") fn main()->i64{0}",
            "report.spx",
        )
        .unwrap(),
    )
    .unwrap();
    let functions = program.functions();
    let function = functions.get(0).unwrap();
    let override_report = vec![Diagnostic::io(
        "SPX-I210",
        "cannot release workspace LOCK: owning test",
    )];
    let override_pointer = override_report.as_ptr();
    let override_text = override_report[0].message.as_ptr();
    let ((), cost) = measure(|| {
        let owner = acquire(paid());
        let pointers = owner.backing();
        let refusal = owner.prepare(function).err().unwrap();
        let error =
            WorkspaceReportFailure::selected(refusal, None).with_unlock_override(override_report);
        assert_eq!(error.reported().unwrap().as_ptr(), override_pointer);
        assert_eq!(error.reported().unwrap()[0].message.as_ptr(), override_text);
        assert_eq!(error.reported().unwrap()[0].code, "SPX-I210");
        assert_eq!(error.retained_report().unwrap().as_ptr(), pointers.0);
        assert_eq!(
            error.retained_report().unwrap()[0].message.as_ptr(),
            pointers.1
        );
        assert!(!error.has_paid_report());
        let mut error = error.into_paid_report().unwrap_err();
        let ledger = error.selected_owner_mut().unwrap().ledger_mut().unwrap();
        let first = ledger.reserve(0).unwrap_err();
        assert_eq!(ledger.reserve(usize::MAX).unwrap_err(), first);
        drop(error);
    });
    assert!(!cost.invalid);
    assert_eq!(
        (cost.allocations, cost.reallocations, cost.allocated),
        (2, 0, paid())
    );
    assert_eq!(cost.live, 0);
}

#[test]
fn typed_mapping_never_replaces_zero_or_exact_short_selected_owner() {
    for limit in [0, paid() - 1] {
        let ((), cost) = measure(|| {
            let refusal = Acquired::try_acquire(Physical::new(limit), FORM)
                .err()
                .unwrap();
            let mut error = WorkspaceReportFailure::selected(refusal, Some(FORM))
                .map_legacy(|_| panic!("selected failure must never enter legacy remapper"));
            render(&error);
            let ledger = error.selected_owner_mut().unwrap().ledger_mut().unwrap();
            let first = ledger.reserve(0).unwrap_err();
            assert_eq!(ledger.reserve(1).unwrap_err(), first);
            drop(error);
        });
        assert!(!cost.invalid);
        assert_eq!(
            (
                cost.allocations,
                cost.reallocations,
                cost.allocated,
                cost.live
            ),
            (0, 0, 0, 0)
        );
    }
}

#[test]
fn complete_prior_report_receiver_and_legacy_move_are_independent() {
    let archived =
        include_str!("../../../docs/evidence/replay-report-caller/src__workspace_report.rs.before");
    let original = include_str!("../original.rs");
    assert_eq!(
        original,
        archived
            .strip_suffix("\n#[cfg(test)]\nmod tests;\n")
            .unwrap()
    );
    let old_report = FORM.legacy();
    let new_report = FORM.legacy();
    let old_pointer = old_report.as_ptr();
    let new_pointer = new_report.as_ptr();
    let old = crate::workspace_report::original::WorkspaceReportFailure::from(old_report);
    let current = WorkspaceReportFailure::from(new_report);
    assert_eq!(old.reported(), current.reported());
    assert_eq!(old.has_paid_report(), current.has_paid_report());
    let old_report = old.into_legacy_unfunded();
    let current_report = current.into_legacy_unfunded();
    assert_eq!(old_report.as_ptr(), old_pointer);
    assert_eq!(current_report.as_ptr(), new_pointer);
}

#[test]
fn all_original_filesystem_preparers_remain_complete_independent_receivers() {
    for (current, old) in [
        (include_str!("../../semantic_workspace_change.rs"), include_str!("../../../docs/evidence/replay-report-caller/src__semantic_workspace_change.rs.before")),
        (include_str!("../../semantic_workspace_operations.rs"), include_str!("../../../docs/evidence/replay-report-caller/src__semantic_workspace_operations.rs.before")),
        (include_str!("../../semantic_workspace_structural_change.rs"), include_str!("../../../docs/evidence/replay-report-caller/src__semantic_workspace_structural_change.rs.before")),
    ] {
        assert!(current == format!("{old}pub mod reporting;\n") || current == format!("{old}\npub mod reporting;\n"));
    }
}
