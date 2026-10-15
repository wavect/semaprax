//! Legacy full-input counters and real sparse backing are distinct scopes.
use super::*;

fn six(with_loans: bool) -> String {
    let mut source = String::from("module local;");
    for index in 0..6 {
        let name = if index == 5 {
            "main".to_owned()
        } else {
            format!("f{index}")
        };
        if with_loans && index < 4 {
            source.push_str(&format!(" @id(\"local.f{index}\") fn {name}(value:own Bytes)->usize {{let view=bytes_as_slice(value);byte_len(view)}}"));
        } else {
            source.push_str(&format!(
                " @id(\"local.f{index}\") fn {name}()->i64 {{{index}}}"
            ));
        }
    }
    source
}
fn keep(function: &hir::ResolvedFunction) -> bool {
    matches!(function.id.as_str(), "local.f0" | "local.f2" | "local.f4")
}

#[test]
fn checked_retention_keeps_full_input_counter_and_sparse_backing_at_literal_limits() {
    let (pass, _, owner) =
        crate::project::incremental::checked_hir::tests::cold_source(&six(false));
    assert_eq!(owner.functions.len(), 6);
    assert!(owner.functions.iter().all(|f| f.loan_plan.loans.is_empty()));
    // Independent current field-layout census, excluding the separately
    // charged LoanPlan sidecar. No historical whole-function width is pinned.
    let fields = std::mem::size_of::<hir::DeclarationId>()
        + std::mem::size_of::<String>()
        + std::mem::size_of::<Vec<hir::ResolvedParam>>()
        + std::mem::size_of::<hir::ValueId>()
        + std::mem::size_of::<hir::ResolvedType>()
        + std::mem::size_of::<Vec<String>>()
        + std::mem::size_of::<Option<hir::ResolvedYieldsClause>>()
        + 2 * std::mem::size_of::<Vec<hir::ResolvedExpr>>()
        + std::mem::size_of::<hir::ResolvedExpr>()
        + std::mem::size_of::<crate::cleanup::CleanupInventory>()
        + std::mem::size_of::<crate::cleanup_plan::CleanupPlan>()
        + std::mem::size_of::<crate::ast::Span>();
    let alignment = std::mem::align_of::<hir::ResolvedFunction>();
    let fixed = fields.div_ceil(alignment) * alignment;
    assert_eq!(
        GRAPH_ACCOUNTED_RESOLVED_FUNCTION_BYTES, fixed,
        "current function carrier equals the independent field census"
    );
    assert_eq!(
        std::mem::size_of::<hir::ResolvedFunction>(),
        fixed + std::mem::size_of::<crate::loan_plan::LoanPlan>()
    );
    let required = 6 * fixed;
    let payload = 3 * std::mem::size_of::<usize>();
    let (result, overflow, charged, _) =
        crate::bounded_output::with_limit_usage_refusal(required, || {
            crate::test_allocation_observer::measure(|| {
                let view = Retained::select_accounted(
                    owner.clone(),
                    |p| &p.functions,
                    keep,
                    fixed,
                    retained_function_loan_bytes,
                )
                .unwrap();
                for (ordinal, function) in view.iter().enumerate() {
                    assert!(std::ptr::eq(function, &owner.functions[2 * ordinal]));
                }
                drop(view);
            })
            .1
        });
    assert!(!overflow);
    assert_eq!(charged, required);
    assert_eq!(
        (
            result.allocations,
            result.reallocations,
            result.allocated,
            result.highwater,
            result.freed,
            result.live
        ),
        (1, 0, payload, payload, payload, 0)
    );
    assert!(!result.invalid);
    let ((refused, retry, allocations), overflow, charged, _) =
        crate::bounded_output::with_limit_usage_refusal(required - 1, || {
            let ((refused, retry), observed) = crate::test_allocation_observer::measure(|| {
                let refused = Retained::select_accounted(
                    owner.clone(),
                    |p| &p.functions,
                    keep,
                    fixed,
                    retained_function_loan_bytes,
                )
                .is_err();
                let retry = Retained::select(owner.clone(), |p| &p.functions, |_| true).is_err();
                (refused, retry)
            });
            (refused, retry, observed)
        });
    assert!(overflow && refused && retry);
    assert!(charged < required);
    // Refusal diagnostics remain observed; no claim of total diagnostic funding.
    assert_eq!(owner.functions.len(), 6);
    assert!(!allocations.invalid && allocations.live == 0);
    drop(pass);
    drop(owner);
}

#[test]
fn all_input_loan_sidecars_match_owned_retention_before_filtering_and_survive_cache_drop() {
    let (mut pass, synthetic, owner) =
        crate::project::incremental::checked_hir::tests::cold_source(&six(true));
    assert!(owner.functions[..4]
        .iter()
        .all(|f| !f.loan_plan.loans.is_empty()));
    assert!(!keep(&owner.functions[1]) && !keep(&owner.functions[3]));
    let (warm, _, _) = pass.checked_module("local.spx", &synthetic).unwrap();
    // Independent owning resolution keeps the original construction capacities;
    // a deep clone would shrink loan vectors and cease to be the cold oracle.
    let old_input = crate::hir::resolve(&synthetic).unwrap().functions;
    let (old, overflow, expected, _) = crate::bounded_output::with_limit_usage_refusal(
        super::super::super::MAX_BUILDER_BYTES,
        || {
            super::super::super::filter_owned_vec_accounted(
                old_input,
                GRAPH_ACCOUNTED_RESOLVED_FUNCTION_BYTES,
                retained_function_loan_bytes,
                keep,
                false,
            )
        },
    );
    assert!(!overflow);
    let old = old.unwrap();
    assert!(expected > 6 * GRAPH_ACCOUNTED_RESOLVED_FUNCTION_BYTES);
    let (view, overflow, charged, _) =
        crate::bounded_output::with_limit_usage_refusal(expected, || {
            Retained::select_accounted(
                warm.clone(),
                |p| &p.functions,
                keep,
                GRAPH_ACCOUNTED_RESOLVED_FUNCTION_BYTES,
                retained_function_loan_bytes,
            )
        });
    assert!(!overflow);
    assert_eq!(charged, expected);
    let view = view.unwrap();
    assert_eq!(
        view.iter().collect::<Vec<_>>(),
        old.iter().collect::<Vec<_>>()
    );
    // Current exact owner remains alive through cache-first drop, including
    // the sidecars of functions excluded from this visible selection.
    drop(pass);
    drop(warm);
    drop(owner);
    assert_eq!(view.len(), 3);
    assert!(!view.get(0).unwrap().loan_plan.loans.is_empty());
    drop(view);
    drop(old);
    let (_, _, owner) = crate::project::incremental::checked_hir::tests::cold_source(&six(true));
    let ((first, retry), overflow, _, _) =
        crate::bounded_output::with_limit_usage_refusal(expected - 1, || {
            let first = Retained::select_accounted(
                owner.clone(),
                |p| &p.functions,
                keep,
                GRAPH_ACCOUNTED_RESOLVED_FUNCTION_BYTES,
                retained_function_loan_bytes,
            )
            .is_err();
            let retry = reserve_builder_structure(0).is_err();
            (first, retry)
        });
    assert!(overflow && first && retry);
}
