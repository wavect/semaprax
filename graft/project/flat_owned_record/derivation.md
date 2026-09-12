# project/flat_owned_record/derivation.rs

- derive_flat_owned_record_api_descriptor · function · L20-L96 — pub fn derive_flat_owned_record_api_descriptor(
- validate_descriptor_size · function · L98-L103 — fn validate_descriptor_size(descriptor: &FlatOwnedRecordApiDescriptor) -> Result<(), Diagnostic>
- derive_export · function · L105-L227 — fn derive_export(
- charge_content · function · L234-L243 — fn charge_content(remaining: &mut usize, value: &str, copies: usize) -> Result<(), Diagnostic>
- stable_host_name · function · L245-L256 — fn stable_host_name(prefix: &str, stable_id: &str) -> String
- host_record_name · function · L258-L260 — fn host_record_name(stable_id: &str) -> String
- host_field_name · function · L262-L264 — fn host_field_name(stable_id: &str) -> String
- tests · module · L267-L296 — mod tests
- descriptor_size_and_content_precheck_preserve_exact_bound · function · L271-L295 — fn descriptor_size_and_content_precheck_preserve_exact_bound()
