# project/candidate/intent/tests.rs

- parse · function · L6-L11 — fn parse(source: &str) -> Program
- programs · function · L13-L35 — fn programs() -> Vec<Program>
- append · function · L37-L41 — fn append() -> Value
- data_expression · function · L43-L51 — fn data_expression(request: &Value) -> Result<Expr>
- string_literals_preserve_decoded_data_and_bound_utf8_before_rendering · function · L54-L74 — fn string_literals_preserve_decoded_data_and_bound_utf8_before_rendering()
- byte_array_literals_charge_payloads_to_the_shared_expression_budget · function · L77-L95 — fn byte_array_literals_charge_payloads_to_the_shared_expression_budget()
- data_literal_grammar_does_not_expand_scalar_migration_defaults · function · L98-L120 — fn data_literal_grammar_does_not_expand_scalar_migration_defaults()
- widened_copy_literals_preserve_scalar_bits_and_signed_node_shape · function · L123-L147 — fn widened_copy_literals_preserve_scalar_bits_and_signed_node_shape()
- append_migrates_nested_contract_loop_and_import_calls_without_reordering · function · L150-L169 — fn append_migrates_nested_contract_loop_and_import_calls_without_reordering()
- rename_keeps_import_alias_and_identity_and_body_uses_stable_id_calls · function · L172-L198 — fn rename_keeps_import_alias_and_identity_and_body_uses_stable_id_calls()
- code · function · L200-L208 — fn code(result: Result<IntentSummary>, expected: &str)
- unsupported_or_effectful_migrations_and_unbound_body_nodes_fail_closed · function · L211-L247 — fn unsupported_or_effectful_migrations_and_unbound_body_nodes_fail_closed()
