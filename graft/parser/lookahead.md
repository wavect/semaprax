# parser/lookahead.rs

- looks_like_generic_variant_qualifier · function · L4-L6 — pub(super) fn looks_like_generic_variant_qualifier(&self) -> bool
- looks_like_generic_record_qualifier · function · L8-L10 — pub(super) fn looks_like_generic_record_qualifier(&self) -> bool
- looks_like_generic_function_call · function · L12-L14 — pub(super) fn looks_like_generic_function_call(&self) -> bool
- looks_like_generic_qualifier · function · L16-L41 — fn looks_like_generic_qualifier(&self, terminator: TokenKind) -> bool
- looks_like_malformed_generic_qualifier · function · L43-L65 — fn looks_like_malformed_generic_qualifier(&self, terminator: &TokenKind) -> bool
- generic_type_end · function · L67-L94 — fn generic_type_end(&self, mut cursor: usize) -> Option<usize>
