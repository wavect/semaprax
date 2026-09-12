# wasm/vec_ops.rs

- is_wasm_owned_vec_type · function · L16-L19 — pub(crate) fn is_wasm_owned_vec_type(program: &ResolvedProgram, ty: &ResolvedType) -> bool
- RECORD_ELEMENT_TAG · constant · L27-L27 — pub(crate) const RECORD_ELEMENT_TAG: i32 = 10;
- RECORD_ELEMENT_MAX_CAPACITY · constant · L39-L40 — pub(crate) const RECORD_ELEMENT_MAX_CAPACITY: u64 = crate::vec_ops::MAX_OWNED_PAYLOAD_BYTES
- _ · constant · L42-L42 — const _: () = assert!(RECORD_ELEMENT_MAX_CAPACITY == 4_096);
- program_uses_vec · function · L44-L83 — pub(crate) fn program_uses_vec(program: &ResolvedProgram) -> bool
- program_uses_record_vec · function · L90-L92 — pub(crate) fn program_uses_record_vec(program: &ResolvedProgram) -> bool
- program_uses_extended_vec · function · L94-L128 — pub(crate) fn program_uses_extended_vec(program: &ResolvedProgram) -> bool
