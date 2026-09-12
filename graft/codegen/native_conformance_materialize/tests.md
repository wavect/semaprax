# codegen/native_conformance_materialize/tests.rs

- SOURCE · constant · L11-L31 — const SOURCE: &str = r#"module test.native_materialize;
- program · function · L33-L36 — fn program() -> ResolvedProgram
- function · function · L38-L44 — fn function<'a>(program: &'a ResolvedProgram, id: &str) -> &'a ResolvedFunction
- wire_place · function · L46-L62 — fn wire_place(place: &CleanupPlace) -> WirePlace
- contract_wire_status · function · L64-L72 — fn contract_wire_status() -> WireStatus
- successful_consume_wire · function · L74-L123 — fn successful_consume_wire(function: &ResolvedFunction) -> WireTrace
- materializes_success_by_cloning_validated_plan_identities · function · L126-L147 — fn materializes_success_by_cloning_validated_plan_identities()
- materializes_owned_transfer_and_result_source · function · L150-L217 — fn materializes_owned_transfer_and_result_source()
- materializes_only_the_trusted_contract_status · function · L220-L264 — fn materializes_only_the_trusted_contract_status()
- rejects_hostile_function_invocation_identity_and_status_fields · function · L267-L321 — fn rejects_hostile_function_invocation_identity_and_status_fields()
- rejects_unknown_places_result_sources_and_unsupported_shapes · function · L324-L373 — fn rejects_unknown_places_result_sources_and_unsupported_shapes()
