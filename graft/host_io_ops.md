---
covers: []
---
# host_io_ops.rs

- STDOUT_WRITE_NAME · constant · L9-L9 — pub(crate) const STDOUT_WRITE_NAME: &str = "stdout_write";
- STDOUT_WRITE_ID · constant · L10-L10 — pub(crate) const STDOUT_WRITE_ID: &str = "core.host.stdout-write";
- STDOUT_WRITE_EFFECT · constant · L11-L11 — pub(crate) const STDOUT_WRITE_EFFECT: &str = "process.stdout.write";
- MAX_STDOUT_TRANSCRIPT_BYTES · constant · L12-L13 — pub(crate) const MAX_STDOUT_TRANSCRIPT_BYTES: u64 =
- MAX_STDOUT_WRITES_PER_PATH · constant · L14-L15 — pub(crate) const MAX_STDOUT_WRITES_PER_PATH: u64 =
- HostIoOp · enum · L18-L20 — pub(crate) enum HostIoOp
- name · function · L23-L25 — pub(crate) const fn name(self) -> &'static str
- id · function · L27-L29 — pub(crate) const fn id(self) -> &'static str
- effect · function · L31-L33 — pub(crate) const fn effect(self) -> &'static str
- arity · function · L35-L37 — pub(crate) const fn arity(self) -> usize
- return_type · function · L39-L41 — pub(crate) fn return_type(self) -> ResolvedType
- ast_return_type · function · L43-L45 — pub(crate) fn ast_return_type(self) -> Type
- accepts_resolved · function · L47-L49 — pub(crate) fn accepts_resolved(self, index: usize, ty: &ResolvedType) -> bool
- accepts_ast · function · L51-L53 — pub(crate) fn accepts_ast(self, index: usize, ty: &Type) -> bool
- by_name · function · L56-L58 — pub(crate) fn by_name(name: &str) -> Option<HostIoOp>
- by_id · function · L60-L62 — pub(crate) fn by_id(id: &str) -> Option<HostIoOp>
- ast_params · function · L64-L71 — pub(crate) fn ast_params(_op: HostIoOp) -> Vec<Param>
- resolved_params · function · L73-L81 — pub(crate) fn resolved_params(op: HostIoOp) -> Vec<ResolvedParam>
- validate_stdout_profile_authority · function · L89-L126 — pub(crate) fn validate_stdout_profile_authority(
- profile_authority_error · function · L128-L136 — fn profile_authority_error(message: impl Into<String>) -> Diagnostic
