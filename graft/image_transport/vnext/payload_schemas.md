# image_transport/vnext/payload_schemas.rs

- candidate_function_schemas · module · L7-L7 — mod candidate_function_schemas;
- candidate_schemas · module · L9-L9 — mod candidate_schemas;
- function_instance_schemas · module · L11-L11 — mod function_instance_schemas;
- function_reference_schemas · module · L13-L13 — mod function_reference_schemas;
- hole_navigation_schemas · module · L15-L15 — mod hole_navigation_schemas;
- merge_preview_schemas · module · L17-L17 — mod merge_preview_schemas;
- package_schemas · module · L19-L19 — mod package_schemas;
- digest · function · L21-L23 — pub(super) fn digest() -> Value
- text · function · L24-L26 — pub(super) fn text() -> Value
- uint · function · L27-L29 — pub(super) fn uint() -> Value
- nullable · function · L30-L32 — pub(super) fn nullable(value: Value) -> Value
- array · function · L33-L35 — pub(super) fn array(value: Value) -> Value
- blind_spot_ledger · function · L36-L43 — fn blind_spot_ledger() -> Value
- blind_spot · function · L44-L68 — fn blind_spot() -> Value
- object · function · L69-L76 — pub(super) fn object(fields: Vec<(&str, Value)>) -> Value
- document · function · L77-L84 — pub(super) fn document(id: &str, fields: Vec<(&str, Value)>) -> Value
- documents · function · L85-L1500 — pub(super) fn documents(capabilities: &Value) -> BTreeMap<String, Value>
