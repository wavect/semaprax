# workspace_graph/expected_projection/call_identity.rs

- MAX_NAMES · constant · L14-L14 — const MAX_NAMES: usize = 4096;
- discount · function · L16-L152 — pub(super) fn discount(
- primitive · function · L154-L166 — pub(super) fn primitive(ty: &Type) -> bool
- scalar_callee · function · L168-L191 — pub(super) fn scalar_callee(
- roots · function · L193-L199 — pub(super) fn roots(function: &Function) -> impl Iterator<Item = &Expr>
- walk · function · L204-L225 — pub(super) fn walk<'a>(root: &'a Expr, visit: &mut impl FnMut(&'a Expr) -> bool) -> bool
- complete_pattern · function · L227-L264 — pub(super) fn complete_pattern<'a>(
- record_pattern · function · L266-L288 — fn record_pattern<'a>(
- tests · module · L291-L416 — mod tests
- count · function · L293-L298 — fn count(source: &str) -> usize
- identity_prebound_scalar_calls_keep_callee_and_opaque_results · function · L300-L311 — fn identity_prebound_scalar_calls_keep_callee_and_opaque_results()
- identity_prebound_scalar_call_binding_shadowing_is_conservative · function · L313-L322 — fn identity_prebound_scalar_call_binding_shadowing_is_conservative()
- identity_prebound_scalar_call_import_requires_exact_provider_header · function · L324-L337 — fn identity_prebound_scalar_call_import_requires_exact_provider_header()
- identity_prebound_scalar_parameter_reads_keep_both_place_identities · function · L339-L360 — fn identity_prebound_scalar_parameter_reads_keep_both_place_identities()
- identity_prebound_scalar_parameter_redeclarations_disable_discount · function · L363-L374 — fn identity_prebound_scalar_parameter_redeclarations_disable_discount()
- identity_prebound_unique_scalar_locals_keep_place_identities · function · L376-L393 — fn identity_prebound_unique_scalar_locals_keep_place_identities()
- identity_prebound_local_scope_and_opaque_values_remain_charged · function · L396-L415 — fn identity_prebound_local_scope_and_opaque_values_remain_charged()
