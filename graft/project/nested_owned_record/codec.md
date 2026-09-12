# project/nested_owned_record/codec.rs

- validate_closed_descriptor · function · L7-L247 — pub(super) fn validate_closed_descriptor(root: &Map<String, Value>) -> Result<(), Diagnostic>
- exact_keys · function · L249-L255 — fn exact_keys(object: &Map<String, Value>, expected: &[&str]) -> Result<(), Diagnostic>
- object · function · L256-L260 — fn object(value: &Value) -> Result<&Map<String, Value>, Diagnostic>
- array · function · L261-L266 — fn array<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a Vec<Value>, Diagnostic>
- require_string · function · L267-L272 — fn require_string<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, Diagnostic>
- require_u64 · function · L273-L278 — fn require_u64(object: &Map<String, Value>, key: &str) -> Result<u64, Diagnostic>
- require_tag · function · L279-L286 — fn require_tag(object: &Map<String, Value>, key: &str, tags: &[&str]) -> Result<(), Diagnostic>
