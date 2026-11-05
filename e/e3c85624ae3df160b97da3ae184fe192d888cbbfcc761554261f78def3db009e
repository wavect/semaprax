//! Legacy rendering remains explicitly outside the paid fixed-report contract.
use super::*;
use crate::test_allocation_observer::measure;
#[test]
fn sealed_atoms_keep_saved_detailed_text_and_allocate_no_identity_payload() {
    let program = crate::hir::resolve(
        &crate::check(
            "module refusal.text; @id(\"main\") fn main()->i64 {1}",
            "refusal.spx",
        )
        .unwrap(),
    )
    .unwrap();
    let function = &program.functions[0];
    let id = function.body.id.clone();
    let storage = StorageId::Temporary(id.clone());
    let cases = [
        (
            Atom::RepeatedStatus(id.clone()),
            format!("status source for `{id}` is repeated"),
        ),
        (
            Atom::UnknownStatus(id.clone()),
            format!("status source references unknown expression `{id}`"),
        ),
        (
            Atom::NoncontiguousSlot(7),
            "cleanup slot at index 7 has non-contiguous identity or index".into(),
        ),
        (
            Atom::RepeatedStorage(storage.clone()),
            format!("cleanup storage `{storage:?}` is repeated"),
        ),
        (
            Atom::DeadCallFlag(id.clone(), LivenessFlagId(17)),
            format!("call `{id}` atomically consumes dead argument flag 17"),
        ),
        (
            Atom::RepeatedCallFlag(id.clone(), LivenessFlagId(17)),
            format!("call `{id}` atomically consumes argument flag 17 twice"),
        ),
        (
            Atom::RepeatedVariantEpoch(id.clone()),
            format!("call `{id}` atomically consumes a variant epoch twice"),
        ),
        (
            Atom::MismatchedFailure(id.clone()),
            format!("selected failure `{id}` does not match the pending failing edge"),
        ),
    ];
    for (atom, text) in cases {
        let (receipt, observed) = measure(|| Receipt {
            source: id.clone(),
            atom,
        });
        assert_eq!(observed.allocations, 0);
        assert_eq!(receipt.source().as_str().as_ptr(), id.as_str().as_ptr());
        let legacy = receipt.legacy(function);
        assert_eq!(legacy.code, "SPX-H006");
        assert_eq!(
            legacy.message,
            format!("cleanup plan for function `main` failed independent replay: {text}")
        );
    }
}

// Complete independent original adapter, including every owning message arm.
fn complete_original_legacy(receipt: Receipt, function: &ResolvedFunction) -> Diagnostic {
    let message = match receipt.atom {
        Atom::SelectedSession(detail) => detail.to_owned(),
        Atom::RepeatedStatus(id) => format!("status source for `{id}` is repeated"),
        Atom::UnknownStatus(id) => {
            format!("status source references unknown expression `{id}`")
        }
        Atom::NoncontiguousSlot(index) => {
            format!("cleanup slot at index {index} has non-contiguous identity or index")
        }
        Atom::DeadCallFlag(call, flag) => format!(
            "call `{call}` atomically consumes dead argument flag {}",
            flag.0
        ),
        Atom::RepeatedCallFlag(call, flag) => format!(
            "call `{call}` atomically consumes argument flag {} twice",
            flag.0
        ),
        Atom::RepeatedVariantEpoch(call) => {
            format!("call `{call}` atomically consumes a variant epoch twice")
        }
        Atom::MismatchedFailure(expression) => {
            format!("selected failure `{expression}` does not match the pending failing edge")
        }
        Atom::RepeatedStorage(storage) => format!("cleanup storage `{storage:?}` is repeated"),
    };
    super::super::replay_error_old(function, message)
}

#[test]
fn every_atom_matches_complete_original_diagnostic_under_finite_account_through_drop() {
    use crate::cleanup_plan::replay::allocation::{enter, OracleScope};
    let program = crate::hir::resolve(
        &crate::check(
            r#"module refusal.text; @id("main") fn main()->i64 {1}"#,
            "refusal.spx",
        )
        .unwrap(),
    )
    .unwrap();
    let function = &program.functions[0];
    let id = &function.body.id;
    let constructors: [fn(ExpressionId) -> Atom; 8] = [
        Atom::RepeatedStatus,
        Atom::UnknownStatus,
        |_| Atom::NoncontiguousSlot(7),
        |id| Atom::RepeatedStorage(StorageId::Temporary(id)),
        |id| Atom::DeadCallFlag(id, LivenessFlagId(17)),
        |id| Atom::RepeatedCallFlag(id, LivenessFlagId(17)),
        Atom::RepeatedVariantEpoch,
        Atom::MismatchedFailure,
    ];
    for make in constructors {
        let old = complete_original_legacy(
            Receipt {
                source: id.clone(),
                atom: make(id.clone()),
            },
            function,
        );
        let candidate = Receipt {
            source: id.clone(),
            atom: make(id.clone()),
        };
        let scope = OracleScope::open(old.message.len()).unwrap();
        let lease = scope.lease();
        let ((), metrics) = measure(|| {
            let invocation = enter(&program, function, lease).unwrap();
            let diagnostic = candidate.legacy(function);
            assert_eq!(
                diagnostic, old,
                "all six fields of the complete original adapter"
            );
            drop(diagnostic);
            assert_eq!(invocation.exit(), Ok(()));
        });
        assert!(!metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert!(metrics.allocated <= scope.receipt().charged);
        assert_eq!(scope.receipt().charged, old.message.len());
        assert!(scope.receipt().first_refusal.is_none());
        assert!(scope.receipt().physical_failure.is_none());
    }
}
