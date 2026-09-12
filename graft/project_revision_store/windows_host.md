# project_revision_store/windows_host.rs

- path · function · L6-L8 — pub fn path(&self) -> &str
- source · function · L9-L11 — pub fn source(&self) -> &[u8]
- entry_json · function · L15-L17 — pub fn entry_json(&self) -> &[u8]
- manifest · function · L18-L20 — pub fn manifest(&self) -> &[u8]
- workspace_manifest · function · L21-L23 — pub fn workspace_manifest(&self) -> &[u8]
- sources · function · L24-L26 — pub fn sources(&self) -> &[StoredSource]
- entry_digest · function · L27-L29 — pub fn entry_digest(&self) -> &str
- project_revision · function · L30-L32 — pub fn project_revision(&self) -> &str
- workspace_revision · function · L33-L35 — pub fn workspace_revision(&self) -> &str
- project_graph_digest · function · L36-L38 — pub fn project_graph_digest(&self) -> &str
- persist · function · L41-L50 — pub fn persist(
- load · function · L52-L61 — pub fn load(
- replay · function · L63-L95 — pub fn replay(
- Metadata · struct · L97-L101 — pub struct Metadata
- inspect · function · L104-L158 — pub fn inspect(entry_json: &[u8], expected_hex: Option<&str>) -> Result<Metadata, Vec<Diagnostic>>
