# project/public_utf8_api.rs

- PUBLIC_OWNED_UTF8_API_SCHEMA · constant · L6-L6 — pub const PUBLIC_OWNED_UTF8_API_SCHEMA: &str = "semaprax.public-owned-utf8-api.v1";
- PUBLIC_OWNED_UTF8_PROJECT_SCHEMA · constant · L7-L7 — pub const PUBLIC_OWNED_UTF8_PROJECT_SCHEMA: &str = "semaprax.project.v10";
- UTF8_DESCRIPTOR_DIGEST_DOMAIN · constant · L8-L9 — pub(super) const UTF8_DESCRIPTOR_DIGEST_DOMAIN: &[u8] =
- validate_closure_shape · function · L11-L33 — pub(super) fn validate_closure_shape(function: &ResolvedFunction) -> Result<(), String>
- is_direct_string_carrier · function · L35-L43 — fn is_direct_string_carrier(expression: &ResolvedExpr) -> bool
- expression_reaches_string_intrinsic · function · L45-L56 — fn expression_reaches_string_intrinsic(root: &ResolvedExpr) -> bool
- expression_reaches_owned_string · function · L58-L67 — fn expression_reaches_owned_string(root: &ResolvedExpr) -> bool
