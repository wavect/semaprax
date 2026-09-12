# hir/resolve_program/tests.rs

- parsed · function · L15-L17 — fn parsed(source: &str, path: &str) -> crate::ast::Program
- resolver · function · L19-L26 — fn resolver(program: &crate::ast::Program) -> Resolver<'_>
- named · function · L28-L33 — fn named(name: &str, arguments: Vec<crate::ast::Type>) -> crate::ast::Type
- lowering_a_module_without_an_entry_point_is_spx_h005 · function · L36-L55 — fn lowering_a_module_without_an_entry_point_is_spx_h005()
- a_mutually_recursive_record_layout_never_reaches_lowering · function · L58-L101 — fn a_mutually_recursive_record_layout_never_reaches_lowering()
- shared_record_fields_are_a_diamond_and_not_a_recursive_layout · function · L104-L135 — fn shared_record_fields_are_a_diamond_and_not_a_recursive_layout()
- TYPES · constant · L137-L151 — const TYPES: &str = r#"
- resolving_an_undeclared_named_type_is_spx_h001 · function · L154-L172 — fn resolving_an_undeclared_named_type_is_spx_h001()
- generic_type_arguments_are_limited_to_copy_scalars_and_the_owned_byte_prelude · function · L175-L213 — fn generic_type_arguments_are_limited_to_copy_scalars_and_the_owned_byte_prelude()
- function_type_parameters_resolve_to_their_declaration_index · function · L216-L267 — fn function_type_parameters_resolve_to_their_declaration_index()
- discovered_instances_follow_call_order_and_collapse_repeats · function · L270-L336 — fn discovered_instances_follow_call_order_and_collapse_repeats()
- resolved_functions_keep_source_order_and_place_class_methods_last · function · L339-L392 — fn resolved_functions_keep_source_order_and_place_class_methods_last()
