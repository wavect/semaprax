# codegen/native_emit/expression/vec_ops.rs

- owned_payload · module · L3-L3 — mod owned_payload;
- record_payload · module · L4-L4 — mod record_payload;
- emit_vec_op · function · L14-L239 — pub(super) fn emit_vec_op(
- vec_element_tag · function · L251-L267 — fn vec_element_tag(ty: &ResolvedType) -> Result<i32, Diagnostic>
- vec_scalar_to_bits · function · L269-L275 — fn vec_scalar_to_bits(value: &CValue) -> String
- vec_bits_to_scalar · function · L277-L295 — fn vec_bits_to_scalar(bits: &str, ty: &ResolvedType) -> Result<String, Diagnostic>
- tests · module · L298-L323 — mod tests
- vec_element_tag_refuses_an_unadmitted_element_without_panicking · function · L313-L316 — fn vec_element_tag_refuses_an_unadmitted_element_without_panicking()
- vec_bits_to_scalar_refuses_an_unadmitted_element_without_panicking · function · L319-L322 — fn vec_bits_to_scalar_refuses_an_unadmitted_element_without_panicking()
