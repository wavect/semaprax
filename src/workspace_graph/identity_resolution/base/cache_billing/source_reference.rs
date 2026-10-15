//! Named whole-file proof of the shared-prelude cache-billing integration.
//! This is separate from formatting-only CI inverses. Every current root byte
//! is authenticated before undoing the declared failure-call-site hooks
//! and the three shared-billing hooks.
use sha2::{Digest, Sha256};
use std::sync::OnceLock;
fn digest(source: &str) -> String {
    format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(source.as_bytes()))
    )
}
fn restore(current: &str) -> String {
    let merged = crate::source_reflows_merge::main927_input("src/workspace_graph.rs", current);
    let current = merged.as_str();
    assert_eq!(
        digest(current),
        "811b1dae18de462dedf10c6b683ef6f3ea1c835cc0d92964baa39476c97936e4"
    );
    let mut original = current.to_owned();
    // Diagnostic metadata does not alter any resolver or billing receiver.
    const SITE_AFTER_0: &str = r###"            })
            .transpose()?;
        let (core, overflowed, consumed, refusal, site) =
            crate::bounded_output::with_limit_usage_refusal_site(active_builder_limit(), || {
                #[cfg(test)]
                let _trace = crate::bounded_output::stage_debits::start(
"###;
    const SITE_BEFORE_0: &str = r###"            })
            .transpose()?;
        let (core, overflowed, consumed, refusal) =
            crate::bounded_output::with_limit_usage_refusal(active_builder_limit(), || {
                #[cfg(test)]
                let _trace = crate::bounded_output::stage_debits::start(
"###;
    assert_eq!(original.matches(SITE_AFTER_0).count(), 1);
    original = original.replacen(SITE_AFTER_0, SITE_BEFORE_0, 1);
    const SITE_AFTER_1: &str = r###"            || !retry_allowed
        {
            return Err(vec![diagnostics::live_builder_refusal_at(
                active_builder_limit(),
                refusal,
                site,
            )]);
        }
"###;
    const SITE_BEFORE_1: &str = r###"            || !retry_allowed
        {
            return Err(vec![diagnostics::live_builder_refusal(
                active_builder_limit(),
                refusal,
            )]);
        }
"###;
    assert_eq!(original.matches(SITE_AFTER_1).count(), 1);
    original = original.replacen(SITE_AFTER_1, SITE_BEFORE_1, 1);
    assert_eq!(
        digest(&original),
        "3fcd39c7b442be79e1200493893a14f7ad9a00c242e54b40b090c102e3e44561"
    );
    const AFTER_0: &str = r###"            })
        );
        let shared_before = core_proofs.base_prelude.selected_bytes();
        let (resolved, static_text) = if let Some((resolved, resolver_bytes, original_loan_bytes)) =
            cached
"###;
    const BEFORE_0: &str = r###"            })
        );
        let (resolved, static_text) = if let Some((resolved, resolver_bytes, original_loan_bytes)) =
            cached
"###;
    assert_eq!(original.matches(AFTER_0).count(), 1);
    original = original.replacen(AFTER_0, BEFORE_0, 1);
    const AFTER_1: &str = r###"            let static_text = staged!(CachedValidation, hir::validate_with_static_text(&resolved))
                .map_err(|error| vec![error])?;
            identity_resolution::replay_cached_prelude(&mut core_proofs, &synthetic)
                .map_err(identity_resolution::ModuleFailure::into_legacy)?;
            let validation_bytes =
                identity_resolution::local_resolution_bytes(before, shared_before, &core_proofs)?;
            let residual = resolver_bytes
                .checked_sub(validation_bytes)
"###;
    const BEFORE_1: &str = r###"            let static_text = staged!(CachedValidation, hir::validate_with_static_text(&resolved))
                .map_err(|error| vec![error])?;
            let validation_bytes = before.saturating_sub(
                crate::bounded_output::active_remaining()
                    .expect("resolved core has a builder budget"),
            );
            let residual = resolver_bytes
                .checked_sub(validation_bytes)
"###;
    assert_eq!(original.matches(AFTER_1).count(), 1);
    original = original.replacen(AFTER_1, BEFORE_1, 1);
    const AFTER_2: &str = r###"                .map_err(identity_resolution::ModuleFailure::into_legacy)?;
            drop(held);
            let resolver_bytes =
                identity_resolution::local_resolution_bytes(before, shared_before, &core_proofs)?;
            let resolved = if let Some(cache) = frontend.as_deref_mut() {
                staged!(FrontendRetention, {
"###;
    const BEFORE_2: &str = r###"                .map_err(identity_resolution::ModuleFailure::into_legacy)?;
            drop(held);
            let resolver_bytes = before.saturating_sub(
                crate::bounded_output::active_remaining()
                    .expect("resolved core has a builder budget"),
            );
            let resolved = if let Some(cache) = frontend.as_deref_mut() {
                staged!(FrontendRetention, {
"###;
    assert_eq!(original.matches(AFTER_2).count(), 1);
    original = original.replacen(AFTER_2, BEFORE_2, 1);
    assert_eq!(
        digest(&original),
        "de94863e39460dd9270b2feea2fc7a4986d25417e20ee2cfa4b38412e9a13f55"
    );
    const AFTER_3: &str = r###"            // until this legacy outer adapter.
            let (resolved, function_costs, reused_functions, static_text) =
                identity_resolution::resolve_module_with_core(
                    &mut core_proofs,
                    frontend.as_deref(),
                    &program.path,
                    &synthetic,
                    private,
"###;
    const BEFORE_3: &str = r###"            // until this legacy outer adapter.
            let (resolved, function_costs, reused_functions, static_text) =
                identity_resolution::resolve_module_with_core(
                    &mut core_proofs,frontend.as_deref(),
                    &program.path,
                    &synthetic,
                    private,
"###;
    assert_eq!(original.matches(AFTER_3).count(), 1);
    original = original.replacen(AFTER_3, BEFORE_3, 1);
    assert_eq!(
        digest(&original),
        "c82d3bb6176c847fd9ff171f02d9c5aedd113e72b1eb9b1739ffee062319c11b"
    );
    original
}
pub(in crate::workspace_graph) fn historical_root() -> &'static str {
    static ROOT: OnceLock<&'static str> = OnceLock::new();
    ROOT.get_or_init(|| {
        Box::leak(restore(include_str!("../../../../workspace_graph.rs")).into_boxed_str())
    })
}
#[test]
fn complete_root_has_only_the_declared_shared_billing_hooks_and_prior_formatting() {
    assert!(!historical_root().is_empty());
    let mut changed = include_str!("../../../../workspace_graph.rs").to_owned();
    changed.push(' ');
    assert!(std::panic::catch_unwind(|| restore(&changed)).is_err());
}

/// The earlier inline-error proof predates the invocation-owned prelude route.
/// Its complete old `resolve_module` receiver remains independently compiled.
pub(in crate::workspace_graph) fn historical_inline_root() -> &'static str {
    static ROOT: OnceLock<&'static str> = OnceLock::new();
    ROOT.get_or_init(|| {
        const CURRENT_CALL: &str = r###"identity_resolution::resolve_module_with_core(
                    &mut core_proofs,frontend.as_deref(),"###;
        const ORIGINAL_CALL: &str = r###"identity_resolution::resolve_module(
                    frontend.as_deref(),"###;
        let current = historical_root();
        assert_eq!(current.matches(CURRENT_CALL).count(), 1);
        let original = current.replacen(CURRENT_CALL, ORIGINAL_CALL, 1);
        assert_eq!(
            digest(&original),
            "fe644b87c264e23b9afa0660d4bcd8149583f19438146734d258025c27087771"
        );
        Box::leak(original.into_boxed_str())
    })
}

/// Later additive audit registrations stay in the actual compiled source union.
/// Bind that complete current inventory before restoring its older snapshot.
pub(in crate::workspace_graph) fn historical_inline_inventory(current: &str) -> String {
    let merged = crate::source_reflows_merge::main927_input(
        "tests/workspace/semantic_graph/source_inventory.rs",
        current,
    );
    let current = merged.as_str();
    assert_eq!(
        digest(current),
        "e55bd484d5c38c005501cbb3b48493907cd7fc370ab083b810cc71a4b3b0ce0b"
    );
    let mut original = current.to_owned();
    const ADDITIONS_0: &str = r###"        include_str!("../../src/hir/validation/byte_traversal/before_nested.rs"),
        include_str!("../../src/hir/validation/byte_traversal/nested.rs"),
        include_str!("../../src/hir/validation/byte_traversal/nested/slot.rs"),
        include_str!("../../src/hir/validation/byte_traversal/nested/check.rs"),
        include_str!("../../src/hir/validation/byte_traversal/nested/index.rs"),
        include_str!("../../src/hir/validation/byte_traversal/nested/intervals.rs"),
        include_str!("../../src/hir/validation/byte_traversal/nested/shape.rs"),
        include_str!("../../src/hir/validation/byte_traversal/nested/storage.rs"),
        include_str!("../../src/hir/validation/byte_traversal/nested/tests.rs"),
        include_str!("../../src/hir/validation/byte_traversal/nested/walk.rs"),
        include_str!("../../src/source_verify/byte_traversal/nested.rs"),
        include_str!("../../src/source_verify/byte_traversal/nested/tests.rs"),
        include_str!("../../src/source_verify/iterative/while_rules/before_nested.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/tests.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/memo.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/memo/tests.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/epsilon_alias.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/epsilon_alias/tests.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/before_alias.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/before_index.rs"),
        include_str!("../../src/cleanup_plan/replay/selected_context/paid.rs"),
        include_str!("../../src/cleanup_plan/replay/selected_owner/paid.rs"),
        include_str!("../../src/cleanup_plan/replay/selected_profile/paid.rs"),
        include_str!("../../src/cleanup_plan/replay/selected_entry/tests/session_errors/source.rs"),
        include_str!("../../src/cleanup_plan/replay/selected_entry/tests/session_errors.rs"),
"###;
    assert_eq!(original.matches(ADDITIONS_0).count(), 1);
    original = original.replacen(ADDITIONS_0, "", 1);
    const ADDITIONS_1: &str = r###"        include_str!("../../src/loan_plan/paid_plan/retained_before_dense.rs"),
        include_str!("../../src/loan_plan/funded_flat/before_dense.rs"),
        include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix.rs"),
        include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix/tests.rs"),
        include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/retained/status_before_prefix.rs"),
        include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/retained/component_before_prefix.rs"),
"###;
    assert_eq!(original.matches(ADDITIONS_1).count(), 1);
    original = original.replacen(ADDITIONS_1, "", 1);
    const ADDITIONS_2: &str = r###"        include_str!("../../src/hir/nominal_payload/legacy.rs"),
        include_str!("../../src/hir/nominal_payload/legacy/clone.rs"),
        include_str!("../../src/hir/nominal_payload/legacy/tests.rs"),
        include_str!("../../src/hir/declaration_index/nominal/legacy.rs"),
        include_str!("../../src/hir/identity_resolution/nominal_tests/legacy.rs"),
        include_str!("../../src/hir/nominal_payload/legacy/source_tests.rs"),
"###;
    assert_eq!(original.matches(ADDITIONS_2).count(), 1);
    original = original.replacen(ADDITIONS_2, "", 1);
    assert_eq!(
        digest(&original),
        "d4158fd988538e77c0f39a2f05cfc50aab15679744bba14ae1f005fc32c805fb"
    );
    original
}
