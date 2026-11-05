use super::*;

pub(super) fn explicit_owner_siblings_match_legacy_selective_routes_and_body_reuse() {
    use crate::bounded_output::identity_reservation::{IdentityReservation, Refused};
    use crate::bounded_output::{with_limit_usage, with_limit_usage_refusal};
    let source = "module cold; @id(\"helper\") fn helper(value:i64)->i64 {value+1} @id(\"main\") fn main()->i64 {helper(1)}";
    let program = crate::check(source, "cold.spx").unwrap();
    let mut pass = pass(source, &program);
    let view = SyntheticProgramView::new(&program, Vec::new()).unwrap();
    let old = pass
        .resolve_functions(&program.path, &program, None)
        .unwrap()
        .unwrap();
    // Each full resolution is one module route: it acquires its own owner on
    // its own equivalent real workspace builder ledger. Sharing one arbitrary
    // 1 MiB ledger across both exhausted the independent-replay cursor.
    let (owned, failed, owned_used) =
        with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES, || {
            let mut owner = IdentityReservation::acquire().unwrap();
            pass.resolve_functions_paid(&program.path, &program, None, &mut owner)
                .unwrap()
                .unwrap()
        });
    assert!(!failed);
    assert!(owned_used > 0);
    let (borrowed, failed, borrowed_used) =
        with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES, || {
            let mut owner = IdentityReservation::acquire().unwrap();
            pass.resolve_borrowed_cold_source_paid(&program.path, &view, None, &mut owner)
                .unwrap()
                .unwrap()
        });
    assert!(!failed);
    assert!(borrowed_used > 0);
    // Paid costs are measured on an active ledger, so the legacy route is
    // compared under its own equivalent active ledger. The no-active run keeps
    // its zero-cost compatibility inventory, which is not normalized.
    let (old_active, old_failed, _) =
        with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES, || {
            pass.resolve_functions(&program.path, &program, None)
                .unwrap()
                .unwrap()
        });
    assert!(!old_failed);
    assert_eq!(old_active.resolved, old.resolved);
    assert_eq!(
        old.function_costs.keys().collect::<Vec<_>>(),
        old_active.function_costs.keys().collect::<Vec<_>>()
    );
    assert!(old.function_costs.values().all(|cost| *cost == 0));
    for new in [&owned, &borrowed] {
        assert_eq!(new.resolved, old.resolved);
        assert_eq!(new.function_costs, old_active.function_costs);
        assert_eq!(
            (new.reused_functions, new.static_text),
            (old.reused_functions, old.static_text)
        );
    }
    // Retain the independent pre-nominal identity receiver and its original
    // first identity-backing request on an equivalent real owner.
    let word = std::mem::size_of::<usize>();
    let report =
        std::mem::size_of::<IdentityReservation>() + std::mem::size_of::<Diagnostic>() + 48;
    let (legacy_refused, legacy_failed, _, legacy_receipt) =
        with_limit_usage_refusal(report, || {
            let mut owner = IdentityReservation::acquire().unwrap();
            crate::hir::identity_resolution::resolve_checked(
                &program,
                None,
                None,
                None,
                None,
                |bytes| owner.reserve(bytes),
            )
            .err()
            .unwrap()
        });
    assert!(legacy_failed);
    let legacy_receipt = legacy_receipt.unwrap();
    assert_eq!(
        legacy_receipt.requested,
        (2 * word + 11).div_ceil(word) * word
    );
    assert_eq!(legacy_receipt.remaining, 0);
    assert_eq!(legacy_receipt.floor, 0);
    assert_eq!(legacy_receipt.stage, None);
    assert!(matches!(legacy_refused,
        crate::hir::identity_resolution::Failure::Identity(Refused::Budget(first))
        if first == legacy_receipt));
    // The current receiver first funds its actual enlarged nominal map node,
    // before constructing identities. Refusal still returns the same exact
    // caller-owned Budget receipt and moves no HIR; it invents no allowance.
    let (refused, failed, _, receipt) = with_limit_usage_refusal(report, || {
        let mut owner = IdentityReservation::acquire().unwrap();
        pass.resolve_functions_paid(&program.path, &program, None, &mut owner)
            .unwrap()
            .err()
            .unwrap()
    });
    assert!(failed);
    let receipt = receipt.unwrap();
    assert_eq!(
        receipt.requested,
        crate::cleanup_plan::replay_btree_node_upper::<
            crate::hir::DeclarationId,
            crate::hir::NominalPayload<crate::hir::ResolvedTypeParameterDeclaration>,
        >()
        .expect("actual nominal parameter-map node layout")
    );
    assert_eq!(receipt.remaining, 0);
    assert_eq!(receipt.floor, 0);
    assert_eq!(receipt.stage, None);
    assert!(matches!(refused,
        paid_resolution::SelectiveFailure::Resolve(
            crate::hir::identity_resolution::Failure::Identity(Refused::Budget(first)))
        if first == receipt));
    // Checked body reuse and the first-checked-only borrowed gate are unchanged.
    let saved_helper_cost = owned.function_costs["helper"];
    let _owner = pass.resolved_module(
        &program.path,
        program.clone(),
        owned.resolved,
        0,
        0,
        owned.function_costs,
        owned.reused_functions,
    );
    pass.checked = std::mem::take(&mut pass.next_checked);
    let changed = crate::check(&source.replace("helper(1)", "helper(2)"), "cold.spx").unwrap();
    let legacy = pass
        .resolve_functions(&program.path, &changed, None)
        .unwrap()
        .unwrap();
    let (paid, failed, _) = with_limit_usage(1 << 20, || {
        let mut owner = IdentityReservation::acquire().unwrap();
        let paid = pass
            .resolve_functions_paid(&program.path, &changed, None, &mut owner)
            .unwrap()
            .unwrap();
        assert!(pass
            .resolve_borrowed_cold_source_paid(&program.path, &view, None, &mut owner)
            .is_none());
        paid
    });
    assert!(!failed);
    assert_eq!(paid.reused_functions, 1);
    assert_eq!(paid.reused_functions, legacy.reused_functions);
    assert_eq!(paid.resolved, legacy.resolved);
    // Same separate, equivalent 1 MiB active ledger for the legacy route; the
    // no-active `legacy` run above keeps its zero-cost compatibility inventory.
    let (legacy_active, legacy_failed, _) = with_limit_usage(1 << 20, || {
        pass.resolve_functions(&program.path, &changed, None)
            .unwrap()
            .unwrap()
    });
    assert!(!legacy_failed);
    assert_eq!(legacy_active.resolved, legacy.resolved);
    assert_eq!(legacy_active.reused_functions, legacy.reused_functions);
    assert_eq!(paid.function_costs, legacy_active.function_costs);
    assert_eq!(
        legacy.function_costs.keys().collect::<Vec<_>>(),
        legacy_active.function_costs.keys().collect::<Vec<_>>()
    );
    assert_eq!(legacy.function_costs["main"], 0);
    assert_eq!(legacy.function_costs["helper"], saved_helper_cost);
}
