# image_transport/candidates/diagnostics.rs

- reads · module · L6-L6 — mod reads;
- read_payload · function · L8-L15 — pub(in crate::image_transport) fn read_payload(
- DIAGNOSTIC_PROTOCOL_SCHEMA · constant · L17-L17 — pub const DIAGNOSTIC_PROTOCOL_SCHEMA: &str = "semaprax.image-agent-protocol.v4";
- DIAGNOSTIC_RESULT_SCHEMA · constant · L18-L18 — pub const DIAGNOSTIC_RESULT_SCHEMA: &str = "semaprax.image-agent-result.v4";
- ATTEMPT · constant · L19-L23 — const ATTEMPT: Parameter = Parameter
- Action · enum · L25-L37 — pub(in crate::image_transport) enum Action
- method · function · L38-L48 — macro_rules! method
- METHODS_V4 · constant · L49-L167 — const METHODS_V4: &[Method] = &[
- methods · function · L168-L173 — pub(in crate::image_transport) fn methods(test_enabled: bool) -> Vec<&'static Method>
- payload_schema · function · L174-L193 — fn payload_schema(method: &Method, test_enabled: bool) -> String
- descriptor · function · L194-L232 — fn descriptor(method: &Method, test_enabled: bool) -> Value
- handle · function · L233-L282 — pub(super) fn handle(
- prepare · function · L283-L350 — pub(in crate::image_transport) fn prepare(
- attempt · function · L351-L359 — fn attempt<'a>(
- action_payload · function · L360-L447 — fn action_payload(
