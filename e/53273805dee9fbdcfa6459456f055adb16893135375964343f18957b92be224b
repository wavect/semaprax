//! The actual primitive lexical caller owns its paid outward report.
use super::*;
use crate::cleanup_plan::{
    ReplayPhysicalRefusal, SelectedReplayCause, SelectedReplayRefusal, SelectedReplayUnpaidReason,
};
use crate::test_allocation_observer::{measure, Metrics};

fn replay_under(
    build: &WorkspaceGraphBuild,
    limit: usize,
) -> (Result<bool, SelectedReplayRefusal>, Metrics, bool, usize) {
    let ((answer, metrics), overflowed, used) =
        crate::bounded_output::with_limit_usage(limit, || {
            measure(|| {
                replay_lexical_paid(
                    writer(build),
                    &build.hir.modules,
                    &build.hir.declarations,
                    REPORT,
                )
            })
        });
    (answer, metrics, overflowed, used)
}
fn first_receipt(refusal: &mut SelectedReplayRefusal) -> ReplayPhysicalRefusal {
    let ledger = refusal.ledger_mut().expect("retained ledger");
    let first = ledger.reserve(0).unwrap_err();
    assert_eq!(ledger.reserve(1).unwrap_err(), first);
    first
}
fn assert_public(report: &[Diagnostic]) {
    assert_eq!(report.len(), 1);
    assert_eq!(report[0].code, "SPX-G410");
    assert_eq!(
        report[0].message,
        "retained static UTF-8 source-site authority disagrees"
    );
    assert_eq!(report[0].message_owned_capacity(), report[0].message.len());
    assert!(report[0].path.is_none() && report[0].span.is_none() && report[0].help.is_none());
}
fn lexical_anchor(build: &WorkspaceGraphBuild) -> &hir::ResolvedFunction {
    &writer(build).functions[0]
}

#[test]
fn actual_caller_zero_and_report_one_short_return_the_unpaid_ledger_without_allocating() {
    let build = build(LEXICAL_SOURCE);
    let slot = std::mem::size_of::<Diagnostic>();
    for (limit, charged, reason) in [
        (0, 0, "cleanup replay report carrier reservation refused"),
        (
            slot - 1,
            0,
            "cleanup replay report carrier reservation refused",
        ),
        (
            REPORT.paid_bytes() - 1,
            slot,
            "cleanup replay report text reservation refused",
        ),
    ] {
        let (answer, metrics, overflowed, used) = replay_under(&build, limit);
        let Err(mut refusal) = answer else {
            panic!("a one-short enclosing budget cannot fund the report carrier")
        };
        assert!(!metrics.invalid);
        assert_eq!(
            (metrics.allocations, metrics.allocated, metrics.live),
            (0, 0, 0),
            "no Diagnostic, Vec, String or preparation backing before refusal"
        );
        assert!(overflowed);
        assert_eq!(used, charged);
        assert!(!refusal.is_paid());
        assert_eq!(refusal.setup_reason().map(|r| r.message()), Some(reason));
        assert_eq!(
            first_receipt(&mut refusal),
            ReplayPhysicalRefusal::EnclosingBudget
        );
        assert_eq!(refusal.ledger_mut().unwrap().charged(), charged);
        // The unpaid refusal stays typed; it never claims funding.
        let (unpaid, typed) = measure(|| refusal.into_published());
        assert_eq!(typed.allocations, 0);
        let unpaid = unpaid.expect_err("unpaid carrier");
        assert!(matches!(
            unpaid.reason(),
            SelectedReplayUnpaidReason::Carrier(_)
        ));
        // Only the named unfunded boundary allocates the same literal.
        assert_public(&unpaid.into_unfunded());
    }
}

#[test]
fn actual_caller_paid_setup_refusal_keeps_original_carrier_ledger_and_drop() {
    let build = build(LEXICAL_SOURCE);
    let paid = REPORT.paid_bytes();
    // ONE observer spans acquisition, transfer, publication and Drop. A fresh
    // pointer table at Drop would be unable to observe these original frees.
    let (((), metrics), overflowed, used) = crate::bounded_output::with_limit_usage(paid, || {
        measure(|| {
            let mut refusal = match replay_lexical_paid(
                writer(&build),
                &build.hir.modules,
                &build.hir.declarations,
                REPORT,
            ) {
                Err(refusal) => refusal,
                Ok(_) => panic!("an exact report payment leaves no preparation backing"),
            };
            assert!(refusal.is_paid());
            assert_public(refusal.report().unwrap());
            let original = refusal.report().unwrap();
            let backing = (original.as_ptr(), original[0].message.as_ptr());
            assert_eq!(
                refusal.setup_reason().map(|r| r.message()),
                Some("cleanup replay physical reservation exceeds the bound")
            );
            assert_eq!(
                refusal.refused().map(|r| r.cause()),
                Some(SelectedReplayCause::Setup)
            );
            assert_eq!(
                first_receipt(&mut refusal),
                ReplayPhysicalRefusal::EnclosingBudget
            );
            assert_eq!(refusal.ledger_mut().unwrap().charged(), paid);
            let report = refusal.into_published().expect("paid G410 carrier");
            assert_public(&report);
            assert_eq!(report.as_ptr(), backing.0);
            assert_eq!(report[0].message.as_ptr(), backing.1);
            drop(report);
        })
    });
    assert!(overflowed);
    assert_eq!(used, paid);
    assert!(!metrics.invalid);
    assert_eq!(metrics.allocations, 2, "exactly the paid slot and text");
    assert_eq!(
        metrics.reallocations, 0,
        "publication kept the original backing"
    );
    assert_eq!(metrics.allocated, paid);
    assert_eq!(metrics.freed, paid);
    assert_eq!(metrics.live, 0);
}

#[test]
fn actual_caller_authentication_refusal_allocates_and_discards_nothing() {
    for attack in [0, 1] {
        let mut build = build(LEXICAL_SOURCE);
        let function = writer_mut(&mut build)
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "static.lexical")
            .unwrap();
        match attack {
            0 => function.name.push('x'),
            _ => function.return_type = hir::ResolvedType::Bool,
        }
        let (answer, metrics, _, _) = replay_under(&build, MAX_CHANGE_BUILDER_BYTES);
        let Err(refusal) = answer else {
            panic!("attack {attack} must refuse")
        };
        // Carrier slot and text plus the neutral preparation (ledger box and
        // prepaid refusal text). A discarded legacy Vec<Diagnostic> or String
        // before the transfer would appear here as a free or a reallocation.
        assert!(!metrics.invalid);
        assert_eq!(
            (metrics.freed, metrics.reallocations),
            (0, 0),
            "attack {attack}: nothing allocated and discarded before return"
        );
        assert_eq!(metrics.live, metrics.allocated, "attack {attack}");
        assert!(refusal.is_paid(), "attack {attack}");
        assert_public(refusal.report().unwrap());
        let refused = refusal.refused().unwrap();
        assert_eq!(
            refused.cause(),
            SelectedReplayCause::Mismatch,
            "attack {attack}"
        );
        assert!(refusal.receipt().is_none(), "no budget or Session receipt");
    }
}

#[test]
fn actual_caller_authentication_and_session_refusals_transfer_one_paid_report() {
    for attack in [0, 1, 3] {
        let mut build = build(LEXICAL_SOURCE);
        let function = writer_mut(&mut build)
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "static.lexical")
            .unwrap();
        match attack {
            0 => function.name.push('x'),
            1 => function.return_type = hir::ResolvedType::Bool,
            _ => {
                function
                    .cleanup_plan
                    .lexical_control
                    .as_mut()
                    .unwrap()
                    .exits[0]
                    .site = function.body.id.clone()
            }
        }
        let (answer, _, _, _) = replay_owned_under(&build, MAX_CHANGE_BUILDER_BYTES);
        let Err(mut refusal) = answer else {
            panic!("attack {attack} must refuse")
        };
        assert!(refusal.is_paid(), "attack {attack}");
        assert_public(refusal.report().unwrap());
        assert_eq!(refusal.setup_reason(), None, "refused after preparation");
        if attack == 3 {
            // The independently replayed cause is the Session's own retained
            // Diagnostic, distinguishable from a source/namespace mismatch
            // without any help text, clone or format.
            assert_eq!(
                refusal.refused().map(|r| r.cause()),
                Some(SelectedReplayCause::Session)
            );
            assert_eq!(refusal.receipt().expect("Session receipt").code, "SPX-H006");
        }
        let first = first_receipt(&mut refusal);
        if attack < 2 {
            assert_eq!(
                first,
                ReplayPhysicalRefusal::LocalLimit,
                "rejected on transfer"
            );
        }
        // The paid report restores no admission: the owner remains failed.
        let mut preparation = refusal.into_preparation().expect("retained owner");
        let anchor = lexical_anchor(&build);
        assert!(preparation.charge(anchor, 0, "restore").is_err());
        assert!(preparation
            .reserve_array::<u8>(anchor, 0, "restore")
            .is_err());
        assert!(preparation.try_charge(anchor, 0, "restore").is_err());
        drop(preparation);
        // The legacy pass-through route publishes the identical literal.
        assert_public(&build.source_graph_schemas().unwrap_err());
    }
}

#[test]
fn actual_caller_one_short_budget_refusal_retains_the_prepaid_receipt() {
    let build = build(LEXICAL_SOURCE);
    let (answer, _, overflowed, used) = replay_under(&build, MAX_CHANGE_BUILDER_BYTES);
    assert!(matches!(answer, Ok(true)) && !overflowed);
    let (answer, _, overflowed, _) = replay_under(&build, used - 1);
    let Err(refusal) = answer else {
        panic!("one byte short of the successful route must refuse")
    };
    assert!(overflowed);
    assert!(refusal.is_paid());
    assert_public(refusal.report().unwrap());
    let cause = refusal.refused().unwrap().cause();
    assert!(
        matches!(
            cause,
            SelectedReplayCause::Budget | SelectedReplayCause::Session
        ),
        "{cause:?}"
    );
    // Wherever the last debit falls, the first receipt is the budget's
    // prepaid refusal text, moved once rather than formatted afresh.
    let receipt = refusal.receipt().expect("prepaid budget receipt");
    assert_eq!(receipt.code, "SPX-H006");
    assert_eq!(
        receipt.message_owned_capacity(),
        lexical_anchor(&build).id.as_str().len() + 512
    );
    assert!(receipt.message.starts_with("cleanup plan for function `"));
}

#[test]
fn reporting_views_publish_the_callers_prepaid_form_and_keep_success_unchanged() {
    const FORM: crate::cleanup_plan::SelectedReplayReportForm =
        crate::cleanup_plan::SelectedReplayReportForm::new("SPX-T781", "owning remap control");
    let retained = build(LEXICAL_SOURCE);
    assert_eq!(
        retained.source_graph_schemas().unwrap()["writer.spx"],
        "semaprax.graph.v77"
    );
    let view = retained.into_change_view_reporting(FORM).unwrap();
    assert!(view
        .modules
        .iter()
        .any(|module| module.source_graph_schema == "semaprax.graph.v77"));
    let forged = || {
        let mut forged = build(LEXICAL_SOURCE);
        writer_mut(&mut forged)
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "static.lexical")
            .unwrap()
            .name
            .push('x');
        forged
    };
    let Err(report) = forged().into_change_view_reporting(FORM) else {
        panic!("forged retained lexical source must refuse")
    };
    assert_eq!(report.len(), 1);
    assert_eq!(
        (report[0].code, report[0].message.as_ref()),
        (FORM.code(), FORM.message())
    );
    assert_eq!(report[0].message_owned_capacity(), FORM.message().len());
    // The typed route: the paid carrier for exactly FORM moves out, while a
    // different requested form fails closed to the unpaid boundary.
    let Err(refusal) = forged().into_change_view_paid(FORM) else {
        panic!("forged retained lexical source must refuse")
    };
    let (published, moved) = measure(|| refusal.into_report(FORM));
    assert_eq!(moved.allocations, 0);
    assert_eq!(published.expect("paid FORM").len(), 1);
    let Err(refusal) = forged().into_change_view_paid(FORM) else {
        panic!("forged retained lexical source must refuse")
    };
    let (other, typed) = measure(|| refusal.into_report(REPORT));
    assert_eq!(typed.allocations, 0, "no diagnostic is rebuilt");
    assert_eq!(
        other
            .expect_err("a FORM carrier never stands in for G410")
            .reason(),
        SelectedReplayUnpaidReason::FormMismatch
    );
}

// Complete original report observer, selecting the whole original caller.
fn replay_owned_under(
    build: &WorkspaceGraphBuild,
    limit: usize,
) -> (Result<bool, SelectedReplayRefusal>, Metrics, bool, usize) {
    let ((answer, metrics), overflowed, used) =
        crate::bounded_output::with_limit_usage(limit, || {
            measure(|| {
                super::super::primitive::replay_owned_before(
                    writer(build),
                    &build.hir.modules,
                    &build.hir.declarations,
                    REPORT,
                )
            })
        });
    (answer, metrics, overflowed, used)
}

#[test]
fn actual_typed_caller_retains_exact_child_atom_source_full_old_fields_and_first_sticky_drop() {
    let mut build = build(LEXICAL_SOURCE);
    let function = writer_mut(&mut build)
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "static.lexical")
        .unwrap();
    function
        .cleanup_plan
        .lexical_control
        .as_mut()
        .unwrap()
        .exits[0]
        .site = function.body.id.clone();
    let subject = writer(&build)
        .functions
        .iter()
        .find(|f| f.id.as_str() == "static.lexical")
        .unwrap();
    let (old, _, _, _) = replay_owned_under(&build, MAX_CHANGE_BUILDER_BYTES);
    let old = match old {
        Err(old) => old,
        Ok(_) => panic!("whole original caller refuses"),
    };
    let (current, _, _, _) = replay_under(&build, MAX_CHANGE_BUILDER_BYTES);
    let mut current = match current {
        Err(current) => current,
        Ok(_) => panic!("typed actual caller refuses"),
    };
    assert_public(current.report().unwrap());
    assert_eq!(
        current.report(),
        old.report(),
        "all six fixed outward report fields unchanged"
    );
    assert_eq!(
        current.refused().unwrap().cause(),
        SelectedReplayCause::Session
    );
    let ((source, atom), observed) = measure(|| {
        let receipt = current
            .structural_receipt()
            .expect("actual first structural receipt");
        let (source, atom) = receipt
            .borrowed_child_receipt()
            .expect("exact selected-child variant");
        assert_eq!(source, &subject.body.id);
        assert_eq!(source.as_str().as_ptr(), subject.body.id.as_str().as_ptr());
        (source.as_str().as_ptr(), atom)
    });
    assert_eq!(
        observed.allocations, 0,
        "borrowed typed observation allocates nothing"
    );
    let (legacy, whole_old) = current
        .structural_receipt()
        .unwrap()
        .child_legacy_pair(subject);
    assert_eq!(
        legacy, whole_old,
        "all six fields from the complete independent original renderer"
    );
    assert_eq!(
        &legacy,
        old.receipt()
            .expect("whole original caller genuinely retains Owned Diagnostic")
    );
    let first = first_receipt(&mut current);
    let saved = current
        .structural_receipt()
        .unwrap()
        .borrowed_child_receipt()
        .unwrap();
    assert_eq!((saved.0.as_str().as_ptr(), saved.1), (source, atom));
    let mut preparation = current.into_preparation().expect("same failed owner");
    assert!(preparation
        .try_charge(lexical_anchor(&build), 0, "typed sticky")
        .is_err());
    let saved = preparation
        .structural_receipt()
        .unwrap()
        .borrowed_child_receipt()
        .unwrap();
    assert_eq!((saved.0.as_str().as_ptr(), saved.1), (source, atom));
    drop(preparation);
    drop(old);
    drop(legacy);
    drop(whole_old);
    // A separate whole physical interval includes construction, report, source
    // observation, failed-owner recovery and complete Drop; no legacy rendering.
    let (((), metrics), _) = crate::bounded_output::with_limit(MAX_CHANGE_BUILDER_BYTES, || {
        measure(|| {
            let mut refusal = match replay_lexical_paid(
                writer(&build),
                &build.hir.modules,
                &build.hir.declarations,
                REPORT,
            ) {
                Err(refusal) => refusal,
                Ok(_) => panic!("same exact mutation refuses"),
            };
            assert_public(refusal.report().unwrap());
            let saved = refusal
                .structural_receipt()
                .unwrap()
                .borrowed_child_receipt()
                .unwrap();
            assert_eq!((saved.0.as_str().as_ptr(), saved.1), (source, atom));
            assert_eq!(first_receipt(&mut refusal), first);
            let before = crate::test_allocation_observer::current_metrics().allocations;
            let mut preparation = refusal.into_preparation().unwrap();
            assert!(preparation
                .try_charge(lexical_anchor(&build), 0, "typed sticky Drop")
                .is_err());
            let saved = preparation
                .structural_receipt()
                .unwrap()
                .borrowed_child_receipt()
                .unwrap();
            assert_eq!((saved.0.as_str().as_ptr(), saved.1), (source, atom));
            drop(preparation);
            assert_eq!(
                crate::test_allocation_observer::current_metrics().allocations,
                before,
                "transfer/sticky/Drop adds no allocation"
            );
        })
    });
    assert!(!metrics.invalid && metrics.live == 0);
    assert_eq!(metrics.allocated, metrics.freed);
}

#[test]
fn complete_original_owned_static_caller_and_report_assertions_keep_exact_source() {
    let before = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/static-typed-session-receipt-20261018/whole-before-typed-primitive.rs.txt"
    ));
    let start = before
        .find("pub(in crate::workspace_graph::source_schema) fn replay_paid(")
        .unwrap();
    let end = before[start..].find("\n/// Local authentication").unwrap() + start;
    let expected = before[start..end].trim_end().replacen(
        "pub(in crate::workspace_graph::source_schema) fn replay_paid(",
        "pub(in crate::workspace_graph) fn replay_owned_before(",
        1,
    );
    let current = include_str!("../primitive.rs");
    let start = current
        .find("pub(in crate::workspace_graph) fn replay_owned_before(")
        .unwrap();
    assert_eq!(
        current[start..].trim_end(),
        expected,
        "complete original caller, one name/visibility glue"
    );
    let before = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/static-typed-session-receipt-20261018/before/src/workspace_graph/source_schema/static_text/tests/paid_report.rs"));
    let marker =
        "#[test]\nfn actual_caller_authentication_and_session_refusals_transfer_one_paid_report()";
    let old = before[before.find(marker).unwrap()..]
        .split("\n#[test]")
        .next()
        .unwrap();
    let current = include_str!("paid_report.rs");
    let actual = current[current.find(marker).unwrap()..]
        .split("\n#[test]")
        .next()
        .unwrap();
    assert_eq!(
        actual,
        old.replace(
            "= replay_under(&build, MAX_CHANGE_BUILDER_BYTES);",
            "= replay_owned_under(&build, MAX_CHANGE_BUILDER_BYTES);"
        ),
        "every original Owned assertion retained on the whole old caller"
    );
    let genuine = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/static-typed-session-receipt-20261018/genuine-owned-pointer-test.rs.txt"
    ));
    assert_eq!(
        include_str!("../../../../cleanup_plan/replay/selected_entry/tests/paid.rs"),
        genuine,
        "genuine Owned pointer/move path is unchanged in full"
    );
}
