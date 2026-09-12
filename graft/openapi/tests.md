# openapi/tests.rs

- COUNTER · constant · L6-L6 — static COUNTER: AtomicUsize = AtomicUsize::new(0);
- write_temp · function · L8-L16 — fn write_temp(source: &str) -> PathBuf
- cleanup · function · L18-L20 — fn cleanup(path: &Path)
- emit · function · L22-L25 — fn emit(path: &Path, selections: &[&str]) -> String
- errors · function · L27-L30 — fn errors(path: &Path, selections: &[&str]) -> Vec<Diagnostic>
- REVERSED · constant · L35-L51 — const REVERSED: &str = r#"module test.openapi.order;
- offset · function · L53-L57 — fn offset(envelope: &str, needle: &str) -> usize
- the_envelope_depends_on_the_selected_set_and_not_on_selection_order · function · L67-L95 — fn the_envelope_depends_on_the_selected_set_and_not_on_selection_order()
- object_keys_are_sorted_while_parameter_arrays_stay_in_declared_order · function · L102-L146 — fn object_keys_are_sorted_while_parameter_arrays_stay_in_declared_order()
- identities_that_derive_one_component_name_fail_closed · function · L153-L189 — fn identities_that_derive_one_component_name_fail_closed()
- unselected_declarations_never_reach_the_document · function · L196-L249 — fn unselected_declarations_never_reach_the_document()
- identities_needing_escaping_round_trip_and_derive_ascii_component_names · function · L257-L297 — fn identities_needing_escaping_round_trip_and_derive_ascii_component_names()
