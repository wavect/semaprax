//! Real selective/fresh receivers in one unchanged 64MiB owner with full old HIR.
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::hir::ResolvedTypeDeclarationKind;
use crate::test_allocation_observer::measure;
const SOURCE: &str = r#"module nominal.receiving;
@id("row") record Row { @id("row.x") x:i64, }
@id("main") fn main()->i64 {0}
"#;
const CAP: usize = 64 * 1024 * 1024;
fn cases(
    program: &crate::hir::ResolvedProgram,
) -> &crate::hir::NominalPayload<crate::hir::ResolvedVariantCaseDeclaration> {
    let ty = program
        .types
        .iter()
        .find(|ty| ty.id.as_str() == crate::prelude::OPTION_ID)
        .unwrap();
    let ResolvedTypeDeclarationKind::Variant { cases } = &ty.kind else {
        panic!("base variant")
    };
    cases
}
#[test]
fn actual_base_receiver_retains_one_payload_across_fresh_borrowed_and_selective_full_old_controls()
{
    let program = crate::check(SOURCE, "nominal-receiving.spx").unwrap();
    let (((), drop_metrics), overflowed, _, first) = with_limit_usage_refusal(CAP, || {
        let authored = BTreeMap::new();
        let mut core = crate::workspace_graph::expected_projection::CoreProviderProofs::new(
            &authored,
            std::slice::from_ref(&program),
        )
        .unwrap();
        // Old and new complete HIR remain in this SAME owner and scope.
        let old = crate::hir::identity_resolution::nominal_original::resolve(&program).unwrap();
        let owned = Input::Owned(program.clone());
        let actual =
            resolve_module_with_core(&mut core, None, &program.path, &owned, None).unwrap();
        assert_eq!(actual.0, old.0);
        assert_eq!(
            crate::cache_codec::encode(&actual.0).unwrap(),
            crate::cache_codec::encode(&old.0).unwrap()
        );
        let receipt = core.base_prelude.receipt().unwrap();
        assert_eq!(receipt.used, receipt.limit);
        assert_eq!(receipt.remaining, 0);
        assert!(receipt.first.is_none());
        let view = crate::workspace_graph::expected_projection::borrowed_leaf::construct(&program)
            .unwrap();
        let borrowed = Input::Borrowed(view);
        let other =
            resolve_module_with_core(&mut core, None, &program.path, &borrowed, None).unwrap();
        assert_eq!(other.0, old.0);
        assert!(cases(&actual.0).same_owner(cases(&other.0)));
        assert_eq!(
            core.base_prelude.receipt(),
            Some(receipt),
            "borrowed reuse acquires no new producer/work counter"
        );
        let cache =
            FrontendPass::paid_receiver_candidate(program.clone(), old.0.clone(), old.1.costs());
        let selective =
            resolve_module_with_core(&mut core, Some(&cache), &program.path, &owned, None).unwrap();
        assert_eq!(selective.0, old.0);
        assert!(cases(&actual.0).same_owner(cases(&selective.0)));
        assert_eq!(core.base_prelude.receipt(), Some(receipt));
        // Returned HIR keeps real payload ownership after the invocation dies.
        drop(core);
        assert_eq!(cases(&actual.0).as_ptr(), cases(&other.0).as_ptr());
        let ((), metrics) = measure(|| {
            drop(actual);
            drop(other);
            drop(selective);
        });
        ((), metrics)
    });
    assert!(!overflowed);
    assert!(first.is_none());
    assert!(!drop_metrics.invalid);
    assert_eq!(drop_metrics.allocated, 0);
}
#[test]
fn actual_base_work_failure_moves_original_prepaid_report_and_retains_exact_typed_first_error() {
    let program = crate::check(SOURCE, "nominal-work-refusal.spx").unwrap();
    let ((report, receipt, pointers, retry, metrics), overflowed, _, first) =
        with_limit_usage_refusal(CAP, || {
            let authored = BTreeMap::new();
            let mut core = crate::workspace_graph::expected_projection::CoreProviderProofs::new(
                &authored,
                std::slice::from_ref(&program),
            )
            .unwrap();
            core.base_prelude.restrict_work(0);
            let input = Input::Owned(program.clone());
            let error =
                resolve_module_with_core(&mut core, None, "nominal-work-refusal.spx", &input, None)
                    .unwrap_err();
            let ModuleFailure::Report(report) = error else {
                panic!("setup already funded")
            };
            let receipt = core.base_prelude.receipt().unwrap();
            let pointers = ACQUIRED.with(|cell| cell.get().1.unwrap());
            let (retry, metrics) = measure(|| {
                core.base_prelude
                    .ensure(&mut |_| -> Result<(), Refused> {
                        panic!("selected failure must not retry callback")
                    })
                    .err()
                    .unwrap()
            });
            (report, receipt, pointers, retry, metrics)
        });
    assert!(
        !overflowed,
        "finite producer work refusal does not fabricate a byte refusal"
    );
    assert!(first.is_none());
    assert_eq!(receipt.used, 0);
    assert_eq!(receipt.remaining, 0);
    let refusal = receipt.first.unwrap();
    assert_eq!(retry, InvocationFailure::Work(refusal));
    assert_eq!(refusal.requested, 1);
    assert_eq!(refusal.remaining, 0);
    assert_eq!(metrics.allocated, 0);
    assert!(!metrics.invalid);
    assert_eq!(report.as_ptr() as usize, pointers.0);
    assert_eq!(report[0].message.as_ptr() as usize, pointers.1);
    assert_eq!(report[0].code, "SPX-H006");
    assert_eq!(
        report[0].message,
        "base prelude producer work bound refused"
    );
    assert!(report[0].path.is_none() && report[0].span.is_none() && report[0].help.is_none());
}
#[test]
fn actual_source_diagnostic_precedes_any_base_producer_activation() {
    let source = SOURCE.replace("@id(\"row.x\")", "@id(\"main\")");
    let program = crate::parse(&source, "nominal-invalid-receiver.spx").unwrap();
    let expected = crate::source_verify::verify_presenting(&program, None);
    assert!(expected.iter().any(|d| d.severity.is_error()));
    let ((report, receipt), _, _, _) = with_limit_usage_refusal(CAP, || {
        let authored = BTreeMap::new();
        let mut core = crate::workspace_graph::expected_projection::CoreProviderProofs::new(
            &authored,
            std::slice::from_ref(&program),
        )
        .unwrap();
        core.base_prelude.restrict_work(0);
        let input = Input::Owned(program.clone());
        let ModuleFailure::Report(report) = resolve_module_with_core(
            &mut core,
            None,
            "nominal-invalid-receiver.spx",
            &input,
            None,
        )
        .unwrap_err() else {
            panic!("source diagnostic")
        };
        (report, core.base_prelude.receipt())
    });
    assert_eq!(report, expected);
    assert!(receipt.is_none());
}

#[test]
fn source_abort_owner_preserves_actual_old_failure_carrier_layouts() {
    use std::mem::{align_of, size_of};
    #[allow(dead_code, reason = "exact historical layout control")]
    enum OldModuleFailure {
        Setup(Refused),
        Report(Vec<Diagnostic>),
    }
    #[allow(dead_code, reason = "exact historical layout control")]
    enum OldNominalFailure {
        Resolve(crate::hir::identity_resolution::Failure<Refused>),
        Base(crate::hir::prelude_bundle::InvocationFailure<Refused>),
    }
    assert_eq!(size_of::<OldModuleFailure>(), size_of::<ModuleFailure>());
    assert_eq!(align_of::<OldModuleFailure>(), align_of::<ModuleFailure>());
    assert_eq!(
        size_of::<OldNominalFailure>(),
        size_of::<crate::hir::identity_resolution::NominalReceiverFailure>()
    );
    assert_eq!(
        align_of::<OldNominalFailure>(),
        align_of::<crate::hir::identity_resolution::NominalReceiverFailure>()
    );
}
