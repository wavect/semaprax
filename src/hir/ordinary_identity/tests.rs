use super::*;
use crate::bounded_output::{
    active_remaining, reserve_active_required, with_limit_usage, with_limit_usage_refusal,
};
use crate::test_allocation_observer::measure;
use std::mem::size_of;

const SIMPLE: &str = "module identity.ordinary; @id(\"main\") fn main()->i64 {0}";
const INVARIANT: &str = r#"module identity.ordinary;
@id("range.type") record Range {
    @id("range.lo") lo:i64,
    @id("range.hi") hi:i64,
} requires lo <= hi
@id("app.main") fn main()->i64 {
    let row = Range { lo:1, hi:2 };
    row.hi
}
"#;
// Saved ordinary public analysis receiver, without the new payer dispatch.
fn old(program: &Program) -> Analysis {
    let diagnostics = source_verify::verify(program);
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity.is_error())
    {
        return Analysis {
            diagnostics,
            resolved: None,
        };
    }
    match super::super::resolve_verified(program, None) {
        Ok((resolved, _)) => Analysis {
            diagnostics,
            resolved: Some(resolved),
        },
        Err(diagnostic) => Analysis {
            diagnostics: vec![diagnostic],
            resolved: None,
        },
    }
}
fn same(actual: &Analysis, expected: &Analysis) {
    assert_eq!(actual.diagnostics, expected.diagnostics);
    assert_eq!(actual.resolved, expected.resolved);
}

#[test]
fn ordinary_public_identity_payment_preserves_two_pass_hir_and_removes_only_temporary_text() {
    let program = crate::check(INVARIANT, "ordinary-identity.spx").unwrap();
    // Exact checked input and independent full-HIR reference are held outside
    // both same-process observations. No cold-static or whole-funding claim.
    let expected = old(&program);
    assert!(expected.resolved.is_some());
    assert!(expected
        .diagnostics
        .iter()
        .all(|diagnostic| !diagnostic.severity.is_error()));
    const FIRST: &[&str] = &[
        "range.type",
        "core.option",
        "core.result",
        "app.main",
        "range.type#invariant",
        "range.lo",
        "range.hi",
        "core.option.none",
        "core.option.some",
        "core.option.some.value",
        "core.result.ok",
        "core.result.ok.value",
        "core.result.err",
        "core.result.err.error",
    ];
    const SECOND: &[&str] = &[
        "range.type",
        "core.option",
        "core.result",
        "app.main",
        "range.type#invariant",
        "range.type#check",
        "range.lo",
        "range.hi",
        "core.option.none",
        "core.option.some",
        "core.option.some.value",
        "core.result.ok",
        "core.result.ok.value",
        "core.result.err",
        "core.result.err.error",
    ];
    let texts: usize = FIRST.iter().chain(SECOND).map(|text| text.len()).sum();
    assert_eq!(FIRST.len() + SECOND.len(), 29);
    let (legacy, failed, old_used) = with_limit_usage(64 * 1024 * 1024, || {
        measure(|| {
            let result = old(&program);
            same(&result, &expected);
            drop(result);
        })
    });
    assert!(!failed);
    let (paid, failed, new_used) = with_limit_usage(64 * 1024 * 1024, || {
        measure(|| {
            let result = crate::hir::analyze(&program);
            same(&result, &expected);
            drop(result);
        })
    });
    assert!(!failed);
    let ((), legacy) = legacy;
    let ((), paid) = paid;
    assert!(!legacy.invalid && !paid.invalid);
    assert_eq!(legacy.live, 0);
    assert_eq!(paid.live, 0);
    assert_eq!(legacy.allocated, legacy.freed);
    assert_eq!(paid.allocated, paid.freed);
    // Literal source request census, not observed totals as a cost formula.
    assert_eq!(legacy.allocations + 2, paid.allocations + 29);
    assert_eq!(
        legacy.allocated + size_of::<Diagnostic>() + 48,
        paid.allocated + texts
    );
    let word = size_of::<usize>();
    let identities: usize = FIRST
        .iter()
        .chain(SECOND)
        .map(|text| (2 * word + text.len()).div_ceil(word) * word)
        .sum();
    assert_eq!(
        new_used,
        old_used + IdentityReservation::setup_bytes().unwrap() + identities
    );
}

#[test]
fn ordinary_public_identity_refusal_moves_paid_report_and_keeps_exact_first_sticky_receipt() {
    let program = crate::check(SIMPLE, "ordinary-identity.spx").unwrap();
    assert_eq!(crate::prelude::OPTION_ID, "core.option");
    let word = size_of::<usize>();
    let identity = (2 * word + 11).div_ceil(word) * word;
    let setup = size_of::<IdentityReservation>() + size_of::<Diagnostic>() + 48;
    assert_eq!(IdentityReservation::setup_bytes(), Some(setup));
    for limit in [setup, setup + identity - 1] {
        let (((), observed), failed, used, first) = with_limit_usage_refusal(limit, || {
            measure(|| {
                let result = crate::hir::analyze(&program);
                assert!(result.resolved.is_none());
                assert_eq!(result.diagnostics.len(), 1);
                assert_eq!(result.diagnostics[0].code, "SPX-H006");
                assert_eq!(
                    result.diagnostics[0].message,
                    "declaration identity backing reservation refused"
                );
                assert_eq!(active_remaining(), Some(limit - setup));
                assert!(!reserve_active_required(0));
                assert!(!reserve_active_required(1));
                drop(result);
            })
        });
        assert!(failed);
        assert_eq!(used, setup);
        assert_eq!(first.unwrap().requested, identity);
        assert_eq!(first.unwrap().remaining, limit - setup);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
        assert!(!observed.invalid);
    }
    let (((), observed), failed, _, _) = with_limit_usage_refusal(setup + identity - 1, || {
        measure(|| {
            let mut pointers = None;
            let report =
                try_resolve_verified_observed(&program, |owner| pointers = Some(owner.pointers()))
                    .unwrap()
                    .err()
                    .unwrap();
            let pointers = pointers.unwrap();
            assert_eq!(report.as_ptr(), pointers.0);
            assert_eq!(report[0].message.as_ptr(), pointers.1);
            drop(report);
        })
    });
    assert!(failed);
    assert_eq!(observed.live, 0);
    assert_eq!(observed.allocated, observed.freed);
}

#[test]
fn ordinary_identity_setup_and_source_errors_preserve_legacy_public_priority() {
    let program = crate::check(SIMPLE, "ordinary-identity.spx").unwrap();
    for limit in [0, IdentityReservation::setup_bytes().unwrap() - 1] {
        let (expected, _, _) = with_limit_usage(limit, || old(&program));
        let (actual, _, _) = with_limit_usage(limit, || crate::hir::analyze(&program));
        same(&actual, &expected);
    }
    let invalid = crate::parse(
        "module invalid; @id(\"main\") fn main()->i64 { true }",
        std::path::Path::new("invalid.spx"),
    )
    .unwrap();
    let setup = IdentityReservation::setup_bytes().unwrap();
    let (expected, old_failed, old_used) = with_limit_usage(setup, || old(&invalid));
    assert!(expected.resolved.is_none());
    let (actual, failed, used) = with_limit_usage(setup, || crate::hir::analyze(&invalid));
    same(&actual, &expected);
    assert_eq!(failed, old_failed);
    assert_eq!(
        used, old_used,
        "source rejection acquires no additional report or identity backing"
    );
    let ((error, observed), failed, used) = with_limit_usage(0, || {
        measure(|| try_resolve_verified(&program).err().unwrap())
    });
    assert!(matches!(error, Refused::Budget(_)));
    assert!(failed);
    assert_eq!(used, 0);
    assert_eq!(
        (observed.allocations, observed.allocated, observed.live),
        (0, 0, 0)
    );
}

// Saved original receiver with the setup hook inserted after the original
// inventory/helpers/prelude phase. No production fallback helper is reused.
fn old_with_setup_refusal(
    program: &Program,
) -> Result<(ResolvedProgram, FunctionResolutionWork), Vec<Diagnostic>> {
    let clause = record_invariants::clause_program(program);
    let inventory = clause.as_ref().unwrap_or(program);
    let helpers = crate::loop_calls::byte_writer::SourceIndex::new(inventory);
    let prelude = crate::prelude::declarations_for_program(inventory);
    assert!(matches!(refuse_setup(), Err(Refused::Budget(_))));
    let resolved = (Resolver {
        function_cells: std::borrow::Cow::Owned(crate::source_verify::function_cells(inventory)),
        program: inventory,
        byte_helpers: helpers,
        declarations: DeclarationIndex::from_verified_with_prelude(inventory, prelude)
            .map_err(|error| vec![error])?,
        reuse: None,
        function_work: FunctionResolutionWork::default(),
        fused_producer: None,
        partition_profile: false,
    })
    .resolve()
    .map_err(|error| vec![error])?;
    let Some(desugared) = record_invariants::desugar(program, &resolved.0) else {
        return Ok(resolved);
    };
    (Resolver {
        function_cells: std::borrow::Cow::Owned(crate::source_verify::function_cells(&desugared)),
        program: &desugared,
        byte_helpers: crate::loop_calls::byte_writer::SourceIndex::new(&desugared),
        declarations: DeclarationIndex::from_verified(&desugared).map_err(|error| vec![error])?,
        reuse: None,
        function_work: FunctionResolutionWork::default(),
        fused_producer: None,
        partition_profile: false,
    })
    .resolve()
    .map_err(|error| vec![error])
}
fn refuse_setup() -> Result<IdentityReservation, Refused> {
    // Inject the real sticky budget refusal at the acquisition boundary. This
    // tests fallback ownership, not an allocator-failure simulation or funding
    // for the legacy semantic diagnostic selected after setup has failed.
    assert!(!reserve_active_required(usize::MAX));
    IdentityReservation::acquire()
}

#[test]
fn setup_refusal_reuses_one_invariant_inventory_and_preserves_legacy_diagnostic_and_drop() {
    let program = crate::check(INVARIANT, "ordinary-identity.spx").unwrap();
    let (expected, failed, _) =
        with_limit_usage(64 * 1024 * 1024, || old_with_setup_refusal(&program));
    assert!(failed);
    let compare =
        |actual: &Result<(ResolvedProgram, FunctionResolutionWork), Vec<Diagnostic>>| match (
            actual, &expected,
        ) {
            (Ok((actual, _)), Ok((expected, _))) => assert_eq!(actual, expected),
            (Err(actual), Err(expected)) => assert_eq!(actual, expected),
            _ => panic!("setup fallback changed success/refusal selection"),
        };
    let (((), old_metrics), old_failed, old_used, old_first) =
        with_limit_usage_refusal(64 * 1024 * 1024, || {
            measure(|| {
                let result = old_with_setup_refusal(&program);
                compare(&result);
                drop(result);
            })
        });
    let (((), new_metrics), new_failed, new_used, new_first) =
        with_limit_usage_refusal(64 * 1024 * 1024, || {
            measure(|| {
                let result = resolve_with_acquisition(&program, true, refuse_setup, |_| {
                    panic!("refused acquisition cannot lend a paid owner")
                })
                .unwrap();
                compare(&result);
                assert!(!reserve_active_required(0));
                drop(result);
            })
        });
    assert!(old_failed && new_failed);
    assert_eq!(old_used, new_used);
    assert_eq!(old_first, new_first);
    assert_eq!(new_first.unwrap().requested, usize::MAX);
    // Entire single-inventory construction, legacy publication and teardown
    // are observed together; a second clause clone cannot hide after refusal.
    assert_eq!(old_metrics, new_metrics);
    assert!(!new_metrics.invalid);
    assert_eq!(new_metrics.live, 0);
    assert_eq!(new_metrics.allocated, new_metrics.freed);
}
