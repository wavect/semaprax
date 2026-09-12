# public_generic_abi/native/template.rs

- HEADER_V1 · constant · L19-L19 — pub const HEADER_V1: &str = include_str!("spx_pg_v1.h");
- BODY_V1 · constant · L21-L21 — const BODY_V1: &str = include_str!("provider_body.c");
- render_reference_provider · function · L27-L60 — pub fn render_reference_provider(
- write_byte_array · function · L62-L78 — fn write_byte_array(source: &mut String, bytes_name: &str, len_name: &str, bytes: &[u8])
- tests · module · L81-L81 — mod tests;
