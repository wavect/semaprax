# source_verify/function_value_inventory.rs

- Binding · enum · L13-L19 — enum Binding
- Scope · type · L21-L21 — type Scope<'a> = HashMap<&'a str, Binding>;
- function_value_targets · function · L23-L46 — pub(crate) fn function_value_targets(
- calls · function · L48-L70 — pub(crate) fn calls(
- initial_scope · function · L72-L89 — fn initial_scope(function: &Function) -> Scope<'_>
- visit · function · L91-L195 — fn visit(
- binding_for_let · function · L197-L214 — fn binding_for_let(
- reference_signature · function · L216-L240 — fn reference_signature(
- reference_target · function · L242-L258 — fn reference_target(
- function_value_signature_for · function · L260-L264 — fn function_value_signature_for(name: &str, functions: &HashMap<&str, &Function>) -> Option<Type>
- candidates · function · L266-L279 — fn candidates(
- shadow_pattern · function · L281-L300 — fn shadow_pattern<'a>(pattern: &'a MatchPattern, scope: &mut Scope<'a>)
- shadow_field · function · L301-L313 — fn shadow_field<'a>(pattern: &'a RecordMatchFieldPattern, scope: &mut Scope<'a>)
