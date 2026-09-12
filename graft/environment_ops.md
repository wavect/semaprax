---
covers: []
---
# environment_ops.rs

- EFFECT · constant · L4-L4 — pub(crate) const EFFECT: &str = "process.environment.read";
- STATUS_DOMAIN · constant · L5-L5 — pub(crate) const STATUS_DOMAIN: &str = "semaprax.environment-input.v1";
- INDEX_OUT_OF_BOUNDS · constant · L6-L6 — pub(crate) const INDEX_OUT_OF_BOUNDS: u32 = 1;
- INVALID_INPUT · constant · L7-L7 — pub(crate) const INVALID_INPUT: u32 = 2;
- CAPACITY_EXCEEDED · constant · L8-L8 — pub(crate) const CAPACITY_EXCEEDED: u32 = 3;
- AUTHORITY_DENIED · constant · L9-L9 — pub(crate) const AUTHORITY_DENIED: u32 = 4;
- STATUS_CODES · constant · L10-L15 — pub(crate) const STATUS_CODES: [u32; 4] = [
- MAX_ENTRIES · constant · L16-L16 — pub(crate) const MAX_ENTRIES: u64 = 256;
- MAX_INPUT_BYTES · constant · L17-L17 — pub(crate) const MAX_INPUT_BYTES: u64 = 65_536;
- ARENA_ID · constant · L18-L18 — pub(crate) const ARENA_ID: &str = "core.host.environment-arena";
- OPERATIONS · constant · L19-L19 — pub(crate) const OPERATIONS: [Op; 3] = [Op::EnvLen, Op::EnvNameUtf8, Op::EnvValueUtf8];
- is_environment · function · L20-L22 — pub(crate) const fn is_environment(op: Op) -> bool
- is_lookup · function · L23-L25 — pub(crate) const fn is_lookup(op: Op) -> bool
- by_name · function · L26-L28 — pub(crate) fn by_name(value: &str) -> Option<Op>
- by_id · function · L29-L31 — pub(crate) fn by_id(value: &str) -> Option<Op>
- name · function · L32-L39 — pub(crate) const fn name(op: Op) -> &'static str
- id · function · L40-L47 — pub(crate) const fn id(op: Op) -> &'static str
- arity · function · L48-L54 — pub(crate) const fn arity(op: Op) -> usize
- ast_return_type · function · L55-L61 — pub(crate) const fn ast_return_type(op: Op) -> Type
- return_type · function · L62-L68 — pub(crate) const fn return_type(op: Op) -> ResolvedType
- result_ownership · function · L69-L75 — pub(crate) const fn result_ownership(op: Op) -> OwnershipMode
- accepts_ast · function · L76-L78 — pub(crate) fn accepts_ast(op: Op, index: usize, ty: &Type) -> bool
- accepts_resolved · function · L79-L81 — pub(crate) fn accepts_resolved(op: Op, index: usize, ty: &ResolvedType) -> bool
- ast_params · function · L82-L93 — pub(crate) fn ast_params(op: Op) -> Vec<Param>
- program_uses_environment · function · L96-L115 — pub(crate) fn program_uses_environment(program: &crate::hir::ResolvedProgram) -> bool
