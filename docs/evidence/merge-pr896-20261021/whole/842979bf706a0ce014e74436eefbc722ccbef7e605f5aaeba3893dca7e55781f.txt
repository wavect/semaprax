macro_rules! substitution_leaf_source_inventory {
    () => {
        audit_source_join!(
            include_str!("../../../../../src/hir/monomorphize.rs"),
            include_str!("../../../../../src/hir/monomorphize/substitution_saved.rs"),
            include_str!("../../../../../src/hir/monomorphize/substitution_leaf_tests.rs"),
        )
    };
}
macro_rules! reachability_leaf_source_inventory {
    () => {
        audit_source_join!(
            include_str!("../../../../../src/hir/type_reachability.rs"),
            include_str!("../../../../../src/hir/type_reachability/inventory.rs"),
            include_str!("../../../../../src/hir/type_reachability/nested_generic_function.rs"),
            include_str!("../../../../../src/hir/type_reachability/owned_oracle.rs"),
            include_str!("../../../../../src/hir/type_reachability/owned_oracle/native_owner_old.rs"),
            include_str!("../../../../../src/hir/type_reachability/owned_oracle/native_owner_old/url_view.rs"),
            include_str!("../../../../../src/hir/type_reachability/leaf_saved.rs"),
            include_str!("../../../../../src/hir/type_reachability/leaf_tests.rs"),
            include_str!("../../../../../src/hir/type_reachability/nominal_saved.rs"),
            include_str!("../../../../../src/hir/type_reachability/nominal_tests.rs"),
        )
    };
}
// Complete borrowed-root oracle owners and their relocated children.
macro_rules! hir_borrowed_root_oracle_source_inventory {
    () => {
        audit_source_join!(
            include_str!("../../../../../src/hir/borrowed_root_oracle.rs"),
            include_str!("../../../../../src/hir/borrowed_root_oracle/resolve_box_call.rs"),
            include_str!(
                "../../../../../src/hir/borrowed_root_oracle/resolve_box_call/iterator.rs"
            ),
            include_str!("../../../../../src/hir/borrowed_root_oracle/resolve_box_call/list.rs"),
            include_str!("../../../../../src/hir/borrowed_root_oracle/resolve_box_call/map.rs"),
            include_str!("../../../../../src/hir/borrowed_root_oracle/resolve_expr.rs"),
            include_str!("../../../../../src/hir/borrowed_root_oracle/resolve_expr/byte_view.rs"),
            include_str!("../../../../../src/hir/borrowed_root_oracle/resolve_expr/capacity.rs"),
            include_str!("../../../../../src/hir/borrowed_root_oracle/resolve_expr/finish.rs"),
            include_str!(
                "../../../../../src/hir/borrowed_root_oracle/resolve_expr/lexical_exit.rs"
            ),
            include_str!(
                "../../../../../src/hir/borrowed_root_oracle/resolve_expr/literal_format.rs"
            ),
            include_str!("../../../../../src/hir/borrowed_root_oracle/resolve_expr/sort_keys.rs"),
            include_str!(
                "../../../../../src/hir/borrowed_root_oracle/resolve_expr/super_method.rs"
            ),
            include_str!("../../../../../src/hir/borrowed_root_oracle/resolve_expr/variable.rs"),
            include_str!(
                "../../../../../src/hir/borrowed_root_oracle/resolve_expr/vec_constructor.rs"
            ),
            include_str!("../../../../../src/hir/borrowed_root_oracle/resolve_expr/vec_field.rs"),
            include_str!("../../../../../src/hir/borrowed_root_oracle/resolve_expr_frame.rs"),
            include_str!(
                "../../../../../src/hir/borrowed_root_oracle/resolve_expr_frame/fixed_results.rs"
            ),
            include_str!(
                "../../../../../src/hir/borrowed_root_oracle/resolve_expr_frame/capacity_old.rs"
            ),
            include_str!("../../../../../src/hir/borrowed_root_oracle/resolve_for.rs"),
            include_str!("../../../../../src/hir/borrowed_root_oracle/resolve_for/bytes.rs"),
            include_str!("../../../../../src/hir/borrowed_root_oracle/resolve_for/record.rs"),
            include_str!("../../../../../src/hir/borrowed_root_oracle/resolve_for_own.rs"),
            include_str!("../../../../../src/hir/borrowed_root_oracle/resolve_pattern.rs"),
            include_str!("../../../../../src/hir/borrowed_root_oracle/resolve_statement.rs"),
            include_str!("../../../../../src/hir/borrowed_root_oracle/resolve_vec_call.rs"),
        )
    };
}
