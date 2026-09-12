# codegen/native_vec.rs

- owned_payload · module · L3-L3 — mod owned_payload;
- emit_runtime · function · L5-L20 — pub(super) fn emit_runtime(
- program_uses_vec · function · L22-L53 — pub(super) fn program_uses_vec(program: &crate::hir::ResolvedProgram) -> bool
- program_uses_extended_ops · function · L55-L87 — fn program_uses_extended_ops(program: &crate::hir::ResolvedProgram) -> bool
- NATIVE_VEC_RUNTIME_C · constant · L89-L214 — const NATIVE_VEC_RUNTIME_C: &str = r#"#include <stddef.h>
- NATIVE_VEC_EXTENDED_RUNTIME_C · constant · L216-L274 — const NATIVE_VEC_EXTENDED_RUNTIME_C: &str = r#"#ifndef SPX_VEC_REALLOC
- NATIVE_VEC_RUNTIME_SUFFIX_C · constant · L276-L303 — const NATIVE_VEC_RUNTIME_SUFFIX_C: &str = r#"static __attribute__((unused)) uint64_t spx_vec_len(struct spx_context *spx_ctx, const spx_vec_v1 *value, uint32_t tag)
