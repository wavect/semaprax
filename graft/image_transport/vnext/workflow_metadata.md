# image_transport/vnext/workflow_metadata.rs

- EVENTS · constant · L8-L16 — pub(in crate::image_transport::vnext) const EVENTS: &[&str] = &[
- OUTCOMES · constant · L17-L25 — pub(in crate::image_transport::vnext) const OUTCOMES: &[&str] = &[
- REPAIR_ACTIONS · constant · L26-L27 — pub(in crate::image_transport::vnext) const REPAIR_ACTIONS: &[&str] =
- PROFILE_DOMAIN · constant · L29-L29 — const PROFILE_DOMAIN: &[u8] = b"semaprax.supported-product-workflow.selected-profile-binding.v1\0";
- CONTRACT_SCHEMA · constant · L30-L30 — const CONTRACT_SCHEMA: &str = "semaprax.supported-product-workflow-response-contract.v1";
- profile_revision · function · L32-L38 — pub(super) fn profile_revision(profile: &Value) -> String
- effect · function · L40-L50 — fn effect(method: &str) -> &'static str
- response_contract · function · L52-L80 — fn response_contract(method: &Method) -> Value
- required_grants · function · L82-L96 — fn required_grants(method: &Method) -> Vec<&'static str>
- step · function · L98-L113 — fn step(methods: &[&Method], index: usize, id: Option<&str>, method: &str) -> Value
- supported · function · L115-L276 — pub(super) fn supported(methods: &[&Method], grants: &[&str], policy: &VNextPolicy) -> Vec<Value>
