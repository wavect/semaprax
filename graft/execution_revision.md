---
covers: []
---
# execution_revision.rs

- iterative · module · L2-L2 — pub mod iterative;
- typed · module · L3-L3 — pub mod typed;
- Result · type · L21-L21 — type Result<T> = std::result::Result<T, Vec<Diagnostic>>;
- ProgramRootRef · enum · L25-L29 — pub enum ProgramRootRef<'a>
- digest · function · L31-L37 — pub fn digest(self) -> &'a str
- segments · function · L38-L44 — fn segments(self) -> &'a [ProgramRootSegment]
- ExecutionRoot · struct · L49-L52 — pub struct ExecutionRoot
- digest · function · L54-L56 — pub fn digest(&self) -> &str
- canonical_json · function · L57-L59 — pub fn canonical_json(&self) -> &str
- root · function · L61-L80 — pub(crate) fn root(schema: &str, facts: serde_json::Value) -> ExecutionRoot
- input_digest · function · L81-L86 — fn input_digest(bytes: &[u8]) -> String
- refused · function · L87-L92 — fn refused(detail: &str) -> Vec<Diagnostic>
- ExecutionRevision · struct · L96-L105 — pub struct ExecutionRevision
- deployment_root · function · L107-L109 — pub fn deployment_root(&self) -> &ExecutionRoot
- instance_root · function · L110-L112 — pub fn instance_root(&self) -> &ExecutionRoot
- execution_revision · function · L113-L115 — pub fn execution_revision(&self) -> &ExecutionRoot
- project_revision · function · L116-L118 — pub fn project_revision(&self) -> &ProjectRevision
- run · function · L119-L136 — pub fn run(
- ExecutionEvidence · struct · L138-L142 — pub struct ExecutionEvidence
- run · function · L144-L146 — pub fn run(&self) -> &LifecycleRun
- evidence_root · function · L147-L149 — pub fn evidence_root(&self) -> &ExecutionRoot
- execution_revision · function · L150-L152 — pub fn execution_revision(&self) -> &ExecutionRoot
- bind_execution_revision · function · L158-L248 — pub fn bind_execution_revision(
