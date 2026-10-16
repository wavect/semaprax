//! The duplicated driver entry uses the same paid profile/owner twins.
//! These are direct receiving controls, not source admission or whole781 proof.
use super::*;
use crate::test_allocation_observer::measure;

const FORM: ReportForm = ReportForm::new("SPX-T781", "driver selected report");
fn fixture() -> ResolvedProgram {
    crate::hir::resolve(
        &crate::check(
            "module driver.refusal; @id(\"main:é\") fn main()->i64 {0}",
            "driver-refusal.spx",
        )
        .unwrap(),
    )
    .unwrap()
}
fn prepared(function: &ResolvedFunction) -> Preparation {
    Acquired::try_acquire(PhysicalReservation::new(1 << 26), FORM)
        .ok()
        .unwrap()
        .prepare(function)
        .ok()
        .unwrap()
}
fn same(a: &Diagnostic, b: &Diagnostic) {
    assert_eq!(a.code, b.code);
    assert_eq!(a.severity, b.severity);
    assert_eq!(a.message, b.message);
    assert_eq!(a.path, b.path);
    assert_eq!(a.span, b.span);
    assert_eq!(a.help, b.help);
}
fn run(
    context: &dyn ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    lexical: bool,
    original: bool,
) -> Result<(), expression_facts::errors::EngineError> {
    match (lexical, original) {
        (true, true) => {
            lexical_replay::validate_paid_before_local_receivers(context, function, budget)
        }
        (true, false) => lexical_replay::validate_paid(context, function, budget),
        (false, true) => {
            scalar_replay::validate_paid_before_local_receivers(context, function, budget)
        }
        (false, false) => scalar_replay::validate_paid(context, function, budget),
    }
}
#[test]
fn duplicated_driver_profile_and_owner_keep_raw_oracle_and_transfer_one_paid_report() {
    let program = fixture();
    let context = primitive_fixture::Context::new(&program);
    let anchor = &program.functions[0];
    for lexical in [false, true] {
        for profile in [false, true] {
            // Same immutable body; only the independently tested entry predicate
            // is hostile. No public source authenticator is bypassed or claimed.
            let mut foreign = anchor.clone();
            if profile {
                foreign.effects.push("foreign.effect".to_owned());
            }
            let mut old = prepared(anchor);
            let expression_facts::errors::EngineError::Owned(expected) =
                run(&context, &foreign, &mut old.budget, lexical, true).unwrap_err()
            else {
                panic!("complete original driver keeps its owned Diagnostic")
            };
            let mut detailed = prepared(anchor);
            let expression_facts::errors::EngineError::Structural(detail) =
                run(&context, &foreign, &mut detailed.budget, lexical, false).unwrap_err()
            else {
                panic!("the separate legacy renderer receives the paid semantic data")
            };
            same(&expected, &detail.legacy(&foreign));
            drop(detailed);
            let ((), whole_drop) = measure(|| {
                let mut current = prepared(anchor);
                let backing = current.report_backing();
                let ledger = current.budget.physical.as_deref().unwrap() as *const _;
                let (error, receiving) = measure(|| {
                    run(&context, &foreign, &mut current.budget, lexical, false).unwrap_err()
                });
                assert!(!receiving.invalid);
                assert_eq!(
                    receiving.allocations, 0,
                    "no dynamic Diagnostic at this boundary"
                );
                let expression_facts::errors::EngineError::Structural(receipt) = error else {
                    panic!("the duplicated local entry keeps prepaid semantic data")
                };
                assert_eq!(receipt.stage(), "selected session");
                let refused = current
                    .refuse_engine(expression_facts::errors::EngineError::Structural(receipt));
                assert_eq!(
                    refused.structural_receipt().unwrap().stage(),
                    "selected session"
                );
                let mut refused = refused;
                let retained = refused.ledger_mut().unwrap();
                assert_eq!(retained as *const _, ledger);
                let charged = retained.charged();
                let first = retained.reserve(0).unwrap_err();
                assert_eq!(retained.reserve(1).unwrap_err(), first);
                assert_eq!(retained.charged(), charged);
                let report = refused.report().unwrap();
                assert_eq!((report.len(), report.capacity()), (1, 1));
                assert_eq!(report.as_ptr(), backing.0);
                assert_eq!(report[0].message.as_ptr(), backing.1);
                assert_eq!(report[0].code, FORM.code());
                assert_eq!(report[0].severity, crate::diagnostic::Severity::Error);
                assert_eq!(report[0].message, FORM.message());
                assert!(
                    report[0].path.is_none()
                        && report[0].span.is_none()
                        && report[0].help.is_none()
                );
                drop(refused);
            });
            assert!(!whole_drop.invalid);
            assert_eq!(
                whole_drop.live, 0,
                "report, receipt, scratch and original ledger drop together"
            );
            drop((expected, old));
        }
    }
}

#[test]
fn duplicated_driver_complete_legacy_bodies_and_current_sources_stay_exact() {
    use sha2::{Digest, Sha256};
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(
                include_str!("../../lexical_replay.rs").as_bytes()
            ))
        ),
        "00cfdd547cad552bde792319f9804ef2d36c50d853972098debec15fe14caa05"
    );
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(
                include_str!("../../scalar_replay.rs").as_bytes()
            ))
        ),
        "414c0444a55574badd9d9b9a83a1f0c7298f57cc183eb6afb88b981cd2912b02"
    );
}
