//! Full new owner membership in every existing source audit union.
#[test]
fn held_authentication_complete_source_owners_and_ordinary_receivers_stay_independent() {
    let audits = [
        (include_str!("../hir_module_boundaries.rs"),"../"),
        (include_str!("../workspace/semantic_graph/source_inventory.rs"),"../../"),
        (include_str!("../../crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs"),"../../../../../"),
    ];
    let owners = [
        ("src/cleanup_plan/replay/allocation/oracle.rs", include_str!("../../src/cleanup_plan/replay/allocation/oracle.rs")),
        ("src/cleanup_plan/replay/allocation/oracle/zero_tests.rs", include_str!("../../src/cleanup_plan/replay/allocation/oracle/zero_tests.rs")),
        ("src/cleanup_plan/replay/factored/canonical_shared.rs", include_str!("../../src/cleanup_plan/replay/factored/canonical_shared.rs")),
        ("src/cleanup_plan/replay/factored/canonical_shared/before_held.rs", include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/before_held.rs")),
        ("src/cleanup_plan/replay/factored/canonical_shared/before_held_memo.rs", include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/before_held_memo.rs")),
        ("src/cleanup_plan/replay/factored/canonical_shared/before_held_retained.rs", include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/before_held_retained.rs")),
        ("src/cleanup_plan/replay/factored/canonical_shared/before_ordered.rs", include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/before_ordered.rs")),
        ("src/cleanup_plan/replay/factored/canonical_shared/held_auth_tests.rs", include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/held_auth_tests.rs")),
        ("src/cleanup_plan/replay/factored/canonical_shared/memo/retained.rs", include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/memo/retained.rs")),
        ("src/cleanup_plan/replay/factored/canonical_shared/memo/retained/held_tests.rs", include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/memo/retained/held_tests.rs")),
        ("src/cleanup_plan/replay/factored/canonical_shared/ordered_weights/amortized_source.rs", include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/ordered_weights/amortized_source.rs")),
        ("src/cleanup_plan/replay/factored/canonical_shared/ordered_weights/source.rs", include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/ordered_weights/source.rs")),
        ("src/cleanup_plan/replay/factored/canonical_shared/scratch_tests.rs", include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/scratch_tests.rs")),
        ("src/cleanup_plan/replay/live_oracle/source_custody/current_view.rs", include_str!("../../src/cleanup_plan/replay/live_oracle/source_custody/current_view.rs")),
        ("src/cleanup_plan/replay/physical_budget.rs", include_str!("../../src/cleanup_plan/replay/physical_budget.rs")),
        ("src/cleanup_plan/replay/physical_budget/authentication.rs", include_str!("../../src/cleanup_plan/replay/physical_budget/authentication.rs")),
        ("src/cleanup_plan/replay/physical_budget/authentication/source.rs", include_str!("../../src/cleanup_plan/replay/physical_budget/authentication/source.rs")),
        ("src/cleanup_plan/replay/physical_budget/authentication/tests.rs", include_str!("../../src/cleanup_plan/replay/physical_budget/authentication/tests.rs")),
        ("src/cleanup_plan/replay/physical_storage.rs", include_str!("../../src/cleanup_plan/replay/physical_storage.rs")),
        ("src/cleanup_plan/replay/physical_storage/zero_tests.rs", include_str!("../../src/cleanup_plan/replay/physical_storage/zero_tests.rs")),
    ];
    for (path, complete) in owners {
        assert!(!complete.is_empty());
        for (audit,prefix) in audits {
            let declaration=format!("include_str!(\"{prefix}{path}\")");
            assert_eq!(audit.matches(&declaration).count(),1,"unique full owner {path}");
        }
    }
    assert_eq!(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/memo.rs"),
        include_str!("../../docs/evidence/held-auth-839/src__cleanup_plan__replay__factored__canonical_shared__memo.rs.before.txt"));
    assert_eq!(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/before_held_retained.rs"),
        include_str!("../../docs/evidence/held-auth-839/src__cleanup_plan__replay__factored__canonical_shared__memo__retained.rs.before.txt"));
}
