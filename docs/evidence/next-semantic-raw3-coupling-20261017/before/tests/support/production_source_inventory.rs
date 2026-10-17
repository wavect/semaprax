//! Explicit boundary membership is independent of complete source-role custody.
//! The full predecessor helper remains byte-exact and independently compiled.
#[path = "new_boundary_module.rs"]
mod boundary;
#[path = "production_source_inventory_original.rs"]
mod original;

pub(super) fn production(raw: &str, manifest: &str) -> String {
    assert_current_relocated_prefix(raw);
    // Authenticate ALL raw bytes, sources, registrations and production/test
    // roles first. No outside-boundary source is relabelled or left unaudited.
    let complete_authenticated_production = original::production(raw, manifest);
    boundary::production(raw, manifest, &complete_authenticated_production)
}

// Authenticate placement on the actual HIR/projection payload received by the
// full source owner, not by counting include tokens in the carrier's text.
fn relocated_prefix() -> &'static str {
    concat!(
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/incoming_flat_targets.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/moderate_tests.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/moderate_tests/before.rs"),
    )
}
fn assert_relocated_prefix(raw: &str) {
    let prefix = relocated_prefix();
    assert!(!prefix.is_empty());
    let rest = raw.strip_prefix(prefix).expect("all relocated subjects must enter the actual raw inventory");
    assert!(!rest.starts_with(prefix), "duplicate relocated inventory prefix");
}

#[test]
fn relocated_subjects_must_reach_the_actual_owner_payload() {
    let prefix = relocated_prefix();
    assert_relocated_prefix(&format!("{prefix}remaining original inventory"));
    for wrong in [String::new(), prefix[1..].to_owned(), format!("sentinel{prefix}"), format!("{prefix}{prefix}")] {
        assert!(std::panic::catch_unwind(|| assert_relocated_prefix(&wrong)).is_err());
    }
}

// New census generation: original three subjects remain, with two actual
// additions between first and second. The original guard/tests stay compiled.
fn current_relocated_prefix() -> &'static str {
    concat!(
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/incoming_flat_targets.rs"),
        include_str!("../../src/loop_calls/byte_writer/tests/string_slots.rs"),
        include_str!("../../tests/wasm/scalar_browser_ci_contract/before_21abc.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/moderate_tests.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/moderate_tests/before.rs"),
    )
}
fn assert_current_relocated_prefix(raw: &str) {
    let prefix = current_relocated_prefix();
    assert!(!prefix.is_empty());
    let rest = raw.strip_prefix(prefix).expect("all five relocated subjects must enter the actual raw inventory");
    assert!(!rest.starts_with(prefix), "duplicate current relocated inventory prefix");
}
#[test]
fn current_relocated_subjects_require_all_five_in_order_and_reject_old_or_duplicate_prefixes() {
    let prefix = current_relocated_prefix();
    assert_current_relocated_prefix(&format!("{prefix}remaining original inventory"));
    for wrong in [String::new(), relocated_prefix().to_owned(), prefix[1..].to_owned(),
        format!("sentinel{prefix}"), format!("{prefix}{prefix}"),
        prefix.replacen(include_str!("../../src/loop_calls/byte_writer/tests/string_slots.rs"), "", 1),
        prefix.replacen(include_str!("../../tests/wasm/scalar_browser_ci_contract/before_21abc.rs"), "", 1)] {
        assert!(std::panic::catch_unwind(|| assert_current_relocated_prefix(&wrong)).is_err());
    }
}
