---
covers: []
---
# agent_economics.rs

- task_comparison · module · L17-L17 — mod task_comparison;
- MANIFEST_SCHEMA · constant · L20-L20 — const MANIFEST_SCHEMA: &str = "semaprax.agent-context-benchmark.v1";
- OUTPUT_SCHEMA · constant · L21-L21 — const OUTPUT_SCHEMA: &str = "semaprax.agent-context-economics.v1";
- TOKEN_SCHEMA · constant · L22-L22 — const TOKEN_SCHEMA: &str = "semaprax.lexical-token.v1";
- Case · struct · L25-L36 — struct Case
- benchmark_manifest · function · L39-L219 — pub fn benchmark_manifest(path: &Path) -> Result<String, Diagnostic>
- lexical_tokens · function · L223-L240 — pub fn lexical_tokens(text: &str) -> usize
- Aggregate · struct · L243-L255 — struct Aggregate
- parse_manifest · function · L257-L317 — fn parse_manifest(manifest: &str) -> Result<Vec<Case>, Diagnostic>
- validate_relative_source · function · L319-L350 — fn validate_relative_source(source: &str, line: usize) -> Result<(), Diagnostic>
- portable_windows_component · function · L352-L381 — fn portable_windows_component(component: &str) -> bool
- canonical_source · function · L383-L445 — fn canonical_source(parent: &Path, source: &str) -> Result<PathBuf, Diagnostic>
- parse_filters · function · L447-L475 — fn parse_filters(value: &str, line: usize) -> Result<Vec<AgentContextFilter>, Diagnostic>
- parse_ids · function · L477-L489 — fn parse_ids(value: &str, label: &str, line: usize) -> Result<Vec<String>, Diagnostic>
- canonical_usize · function · L491-L505 — fn canonical_usize(value: &str, label: &str, line: usize) -> Result<usize, Diagnostic>
- json_number · function · L507-L518 — fn json_number(json: &str, marker: &str) -> Result<usize, Diagnostic>
- fact_is_emitted · function · L520-L522 — fn fact_is_emitted(facts: &str, id: &str) -> bool
- ratio · function · L524-L530 — fn ratio(numerator: usize, denominator: usize) -> String
- gcd · function · L532-L537 — fn gcd(mut left: usize, mut right: usize) -> usize
- sha256 · function · L539-L546 — fn sha256(bytes: &[u8]) -> String
- benchmark_error · function · L548-L550 — fn benchmark_error(message: impl Into<String>) -> Diagnostic
- tests · module · L553-L561 — mod tests
- lexical_unit_is_closed_and_deterministic · function · L557-L560 — fn lexical_unit_is_closed_and_deterministic()
