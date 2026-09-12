# codegen/native_emit/expression/box_ops.rs

- owned_payload · module · L3-L3 — mod owned_payload;
- emit_box_op · function · L11-L101 — pub(super) fn emit_box_op(
- box_element_tag · function · L113-L129 — fn box_element_tag(ty: &ResolvedType) -> Result<i32, Diagnostic>
- box_scalar_to_bits · function · L131-L137 — fn box_scalar_to_bits(value: &CValue) -> String
- box_bits_to_scalar · function · L139-L157 — fn box_bits_to_scalar(bits: &str, ty: &ResolvedType) -> Result<String, Diagnostic>
- tests · module · L160-L179 — mod tests
- box_element_tag_refuses_an_unadmitted_element_without_panicking · function · L169-L172 — fn box_element_tag_refuses_an_unadmitted_element_without_panicking()
- box_bits_to_scalar_refuses_an_unadmitted_element_without_panicking · function · L175-L178 — fn box_bits_to_scalar_refuses_an_unadmitted_element_without_panicking()
