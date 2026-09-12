---
covers: []
---
# query.rs

- SCHEMA_V1 · constant · L25-L25 — pub const SCHEMA_V1: &str = "semaprax.query.v1";
- PROJECT_SCHEMA_V1 · constant · L27-L27 — pub const PROJECT_SCHEMA_V1: &str = "semaprax.project-query.v1";
- KINDS · constant · L30-L40 — pub const KINDS: &[&str] = &[
- QueryFilters · struct · L44-L57 — pub struct QueryFilters
- Match · struct · L61-L67 — pub struct Match
- QueryResult · struct · L71-L76 — pub struct QueryResult
- ProjectMatch · struct · L80-L85 — pub struct ProjectMatch
- ProjectQueryResult · struct · L89-L95 — pub struct ProjectQueryResult
- effects · function · L97-L103 — fn effects(entry: &Entry) -> &[String]
- ids · function · L105-L108 — fn ids(set: Option<&BTreeSet<DeclarationId>>) -> Vec<String>
- validate_kinds · function · L110-L123 — fn validate_kinds(filters: &QueryFilters) -> Result<(), Vec<Diagnostic>>
- validate_call_targets · function · L125-L139 — fn validate_call_targets(
- admitted · function · L141-L163 — fn admitted(entry: &Entry, calls: &[String], called_by: &[String], filters: &QueryFilters) -> bool
- run · function · L166-L203 — pub fn run(
- run_project · function · L208-L306 — pub fn run_project(
- header · function · L309-L315 — fn header(entry: &Entry) -> &str
- text · function · L319-L332 — pub fn text(result: &QueryResult) -> String
- json_strings · function · L334-L344 — fn json_strings(values: &[String]) -> String
- json_option · function · L346-L348 — fn json_option(value: Option<&String>) -> String
- json · function · L352-L390 — pub fn json(result: &QueryResult) -> String
- project_text · function · L394-L408 — pub fn project_text(result: &ProjectQueryResult) -> String
- project_json · function · L412-L455 — pub fn project_json(result: &ProjectQueryResult) -> String
