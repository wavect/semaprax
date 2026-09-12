---
covers: []
---
# agent_skill_bundle.rs

- bformat · function · L42-L46 — macro_rules! bformat
- AGENT_SKILL_SCHEMA · constant · L49-L49 — pub const AGENT_SKILL_SCHEMA: &str = "semaprax.agent-skill.v1";
- MAX_AGENT_SKILL_BUNDLE_BYTES · constant · L54-L54 — pub const MAX_AGENT_SKILL_BUNDLE_BYTES: usize = 64 * 1024;
- AGENT_SKILL_DOMAIN · constant · L56-L56 — const AGENT_SKILL_DOMAIN: &[u8] = b"semaprax.agent-skill.payload.digest.v1\0";
- STDLIB_CATALOG · constant · L58-L58 — const STDLIB_CATALOG: &str = include_str!("../std/catalog.json");
- STDLIB_CATALOG_DOMAIN · constant · L59-L59 — const STDLIB_CATALOG_DOMAIN: &[u8] = b"semaprax.agent-skill.package-status.digest.v1\0";
- capacity · function · L61-L63 — fn capacity(message: &'static str) -> Vec<Diagnostic>
- invalid · function · L65-L67 — fn invalid(message: String) -> Vec<Diagnostic>
- incompatible_schema · function · L69-L78 — fn incompatible_schema(requested: &str) -> Diagnostic
- AUTHORITY_CLASSES · constant · L84-L90 — pub const AUTHORITY_CLASSES: &[&str] = &[
- TARGET_PROFILES · constant · L94-L94 — pub const TARGET_PROFILES: &[&str] = &["core-wasm", "interpreter", "native-c11"];
- PublicWorkflowVerb · struct · L99-L105 — pub struct PublicWorkflowVerb
- PUBLIC_WORKFLOW · constant · L116-L187 — pub const PUBLIC_WORKFLOW: &[PublicWorkflowVerb] = &[
- package_status · function · L189-L205 — fn package_status() -> Result<Value, Vec<Diagnostic>>
- render_public_workflow · function · L207-L220 — fn render_public_workflow() -> String
- quoted_list · function · L222-L231 — fn quoted_list(items: &[&str]) -> String
- domain_digest · function · L233-L242 — fn domain_digest(domain: &[u8], bytes: &[u8]) -> String
- KNOWN_LIMITATIONS · constant · L244-L250 — const KNOWN_LIMITATIONS: &[&str] = &[
- generate_agent_skill_bundle · function · L259-L304 — pub fn generate_agent_skill_bundle() -> Result<String, Vec<Diagnostic>>
- negotiate_agent_skill_schema · function · L311-L317 — pub fn negotiate_agent_skill_schema(requested: &str) -> Result<(), Diagnostic>
- tests · module · L320-L320 — mod tests;
