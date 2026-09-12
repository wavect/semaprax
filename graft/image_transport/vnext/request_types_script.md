# image_transport/vnext/request_types_script.rs

- Result · type · L10-L10 — type Result<T> = std::result::Result<T, Vec<Diagnostic>>;
- MAX_SOURCE_BYTES · constant · L11-L11 — const MAX_SOURCE_BYTES: usize = 900 * 1024;
- MAX_SAFE_INTEGER · constant · L12-L12 — const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
- typescript · function · L14-L60 — pub(super) fn typescript(model: &Model) -> Result<String>
- python · function · L62-L160 — pub(super) fn python(model: &Model) -> Result<String>
- terminal_alias · function · L162-L175 — fn terminal_alias(start: &str, definitions: &BTreeMap<String, &Shape>) -> Result<String>
- py_forward_union · function · L177-L187 — fn py_forward_union(items: &[String]) -> Result<String>
- ts_literal · function · L189-L209 — fn ts_literal(value: &Value) -> Result<String>
- py_literal · function · L210-L226 — fn py_literal(value: &Value) -> Result<String>
- quoted · function · L227-L229 — fn quoted(value: &str) -> Result<String>
- encode · function · L230-L232 — fn encode(value: &Value) -> Result<String>
- bound · function · L233-L241 — fn bound(source: &str) -> Result<()>
- invalid · function · L242-L244 — fn invalid(message: &str) -> Vec<Diagnostic>
- tests · module · L247-L306 — mod tests
- recursive_fields_are_forward_references_and_aliases_bind_concrete_objects · function · L253-L296 — fn recursive_fields_are_forward_references_and_aliases_bind_concrete_objects()
- unproductive_alias_cycles_and_missing_names_fail_closed · function · L299-L305 — fn unproductive_alias_cycles_and_missing_names_fail_closed()
