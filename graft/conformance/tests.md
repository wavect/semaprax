# conformance/tests.rs

- SOURCE · constant · L8-L12 — const SOURCE: &str = r#"module test.trace;
- fixture_ids · function · L14-L22 — fn fixture_ids() -> (DeclarationId, ExpressionId)
- compiler_status_codes_and_canonical_json_are_stable · function · L25-L84 — fn compiler_status_codes_and_canonical_json_are_stable()
- external_statuses_cannot_forge_compiler_owned_mappings · function · L87-L138 — fn external_statuses_cannot_forge_compiler_owned_mappings()
- scalar_success_trace_has_an_exact_canonical_projection · function · L141-L175 — fn scalar_success_trace_has_an_exact_canonical_projection()
- nested_failure_selection_and_callable_import_failure_have_exact_json · function · L178-L232 — fn nested_failure_selection_and_callable_import_failure_have_exact_json()
- event_order_and_json_escaping_are_preserved_without_sorting · function · L235-L307 — fn event_order_and_json_escaping_are_preserved_without_sorting()
