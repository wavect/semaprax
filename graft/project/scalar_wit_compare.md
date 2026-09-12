# project/scalar_wit_compare.rs

- SCALAR_WIT_COMPATIBILITY_SCHEMA · constant · L21-L21 — pub const SCALAR_WIT_COMPATIBILITY_SCHEMA: &str = "semaprax.project-scalar-wit-compatibility.v1";
- CODE_FOREIGN · constant · L22-L22 — const CODE_FOREIGN: &str = "SPX-J124";
- ScalarWitCompatibility · struct · L28-L31 — pub struct ScalarWitCompatibility
- breaking · function · L34-L36 — pub fn breaking(&self) -> bool
- report · function · L38-L40 — pub fn report(&self) -> &str
- Signature · struct · L43-L46 — struct Signature
- classify_scalar_wit_change · function · L54-L142 — pub fn classify_scalar_wit_change(
- parse_descriptor · function · L144-L195 — fn parse_descriptor(
- json_string · function · L197-L199 — fn json_string(value: &str) -> String
- foreign · function · L201-L203 — fn foreign(message: String) -> Vec<Diagnostic>
- tests · module · L206-L340 — mod tests
- descriptor · function · L209-L227 — fn descriptor(exports: &[(&str, &[&str], &str)]) -> String
- classify · function · L229-L241 — fn classify(base: &str, candidate: &str) -> (bool, Value)
- identical_interfaces_are_compatible_regardless_of_revision · function · L244-L251 — fn identical_interfaces_are_compatible_regardless_of_revision()
- per_export_signature_changes_are_classified · function · L254-L329 — fn per_export_signature_changes_are_classified()
- a_foreign_descriptor_is_rejected · function · L332-L339 — fn a_foreign_descriptor_is_rejected()
