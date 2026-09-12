# image_transport/vnext/candidate_archive_store.rs

- schemas · module · L25-L25 — mod schemas;
- schema_documents · function · L27-L29 — pub(super) fn schema_documents(capabilities: &Value) -> std::collections::BTreeMap<String, Value>
- IMAGE_CANDIDATE_ARCHIVE_STORE_SCHEMA · constant · L31-L31 — pub const IMAGE_CANDIDATE_ARCHIVE_STORE_SCHEMA: &str = "semaprax.image-candidate-archive-store.v1";
- IMAGE_DRAFT_ARCHIVE_STORE_SCHEMA · constant · L32-L32 — pub const IMAGE_DRAFT_ARCHIVE_STORE_SCHEMA: &str = "semaprax.image-draft-archive-store.v1";
- CANDIDATE_METHOD · constant · L34-L47 — const CANDIDATE_METHOD: Method = Method
- DRAFT_METHOD · constant · L48-L61 — const DRAFT_METHOD: Method = Method
- methods · function · L63-L65 — pub(super) fn methods() -> [&'static Method; 2]
- with_candidate_archive_store · function · L70-L95 — pub fn with_candidate_archive_store(mut self, root: &Path) -> Result<Self, Vec<Diagnostic>>
- candidate_archive_store_request · function · L97-L145 — pub(super) fn candidate_archive_store_request(
- draft_archive_store_request · function · L147-L197 — pub(super) fn draft_archive_store_request(
- checkpoint_candidate_archive_receipt · function · L199-L223 — fn checkpoint_candidate_archive_receipt(
- checkpoint_draft_archive_receipt · function · L225-L249 — fn checkpoint_draft_archive_receipt(
