# image_transport/vnext/task_economics.rs

- PAYLOAD_SCHEMA · constant · L8-L8 — pub(super) const PAYLOAD_SCHEMA: &str = "semaprax.image-agent-task-comparison-report.v1";
- MAX_TRANSPORT_INPUT_BYTES · constant · L9-L9 — pub(super) const MAX_TRANSPORT_INPUT_BYTES: usize = 28 * 1024;
- MAX_TRANSPORT_REPORT_BYTES · constant · L10-L10 — pub(super) const MAX_TRANSPORT_REPORT_BYTES: usize = 384 * 1024;
- METHOD · constant · L12-L30 — const METHOD: Method = Method
- method · function · L32-L34 — pub(super) fn method() -> &'static Method
- prepare · function · L36-L73 — pub(super) fn prepare(
- sha256 · function · L75-L81 — fn sha256(bytes: &[u8]) -> String
