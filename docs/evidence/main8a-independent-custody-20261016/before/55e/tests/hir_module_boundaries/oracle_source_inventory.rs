//! Complete independent cleanup-oracle tests, outside the production HIR scan.
//! These modules deliberately execute the complete original inventory builder.
const ORACLE_SOURCES: [(&str, &str); 2] = [
    (
        "src/cleanup_plan/replay/paid_type_facts/inventory_profile/tests.rs",
        include_str!("../../src/cleanup_plan/replay/paid_type_facts/inventory_profile/tests.rs"),
    ),
    (
        "src/cleanup_plan/replay/paid_type_facts/inventory_summary/tests.rs",
        include_str!("../../src/cleanup_plan/replay/paid_type_facts/inventory_summary/tests.rs"),
    ),
];

#[test]
fn complete_independent_cleanup_oracles_remain_registered_and_source_bound() {
    let parents = [
        include_str!("../../src/cleanup_plan/replay/paid_type_facts/inventory_profile.rs"),
        include_str!("../../src/cleanup_plan/replay/paid_type_facts/inventory_summary.rs"),
    ];
    let expected_paths = [
        "src/cleanup_plan/replay/paid_type_facts/inventory_profile/tests.rs",
        "src/cleanup_plan/replay/paid_type_facts/inventory_summary/tests.rs",
    ];
    let owning_controls = [
        "fn full_profile_receiver_preserves_canonical_shapes_limits_and_prepaid_drop()",
        "fn complete_old_inventory_and_generic_shape_sources_remain_independent_byte_exact()",
    ];
    let own_source = include_str!("oracle_source_inventory.rs");
    let root_source = include_str!("../hir_module_boundaries.rs");
    let registration = "#[cfg(test)]\n#[path = \"hir_module_boundaries/oracle_source_inventory.rs\"]\nmod oracle_source_inventory;\n";
    assert_eq!(root_source.matches(registration).count(), 1);
    assert_eq!(ORACLE_SOURCES.len(), expected_paths.len());
    for (ordinal, (path, complete_source)) in ORACLE_SOURCES.iter().enumerate() {
        assert_eq!(*path, expected_paths[ordinal]);
        assert_eq!(
            parents[ordinal]
                .matches("#[cfg(test)]\nmod tests;\n")
                .count(),
            1
        );
        // The compiler includes the complete file. Exact literal membership
        // prevents moving a test receiver back into the production scan or
        // silently losing an old oracle while retaining only its metadata.
        let literal = format!("include_str!(\"../../{path}\")");
        assert_eq!(own_source.matches(&literal).count(), 1);
        let production_literal = format!("include_str!(\"../{path}\")");
        assert!(!root_source.contains(&production_literal));
        assert!(complete_source.contains(owning_controls[ordinal]));
        assert!(complete_source.contains("crate::cleanup::build_inventory"));
    }
}
