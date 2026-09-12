# image_transport/vnext/response_types_script.rs

- Result · type · L9-L9 — type Result<T> = std::result::Result<T, Vec<Diagnostic>>;
- MAX_SCRIPT_TYPES_BYTES · constant · L10-L10 — const MAX_SCRIPT_TYPES_BYTES: usize = 900 * 1024;
- MAX_SAFE_INTEGER · constant · L11-L11 — const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
- typescript · function · L13-L62 — pub(super) fn typescript(model: &Model) -> Result<String>
- python · function · L64-L171 — pub(super) fn python(model: &Model) -> Result<String>
- py_reference · function · L173-L179 — fn py_reference(name: &str, emitted: &BTreeSet<String>) -> Result<String>
- terminal_alias · function · L181-L214 — fn terminal_alias(
- py_union · function · L216-L222 — fn py_union(items: &[String], empty: &str) -> String
- ts_literal · function · L224-L246 — fn ts_literal(value: &Value) -> Result<String>
- py_literal · function · L248-L264 — fn py_literal(value: &Value) -> Result<String>
- quoted · function · L266-L268 — fn quoted(value: &str) -> Result<String>
- quoted_value · function · L269-L271 — fn quoted_value(value: &Value) -> Result<String>
- bound · function · L272-L280 — fn bound(source: &str) -> Result<()>
- invalid · function · L281-L283 — fn invalid(message: &str) -> Vec<Diagnostic>
- tests · module · L286-L304 — mod tests
- script_literals_preserve_each_language_representation · function · L291-L303 — fn script_literals_preserve_each_language_representation()
