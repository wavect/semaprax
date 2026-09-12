---
covers: []
---
# str_ops.rs

- LEN_BYTES_NAME · constant · L12-L12 — pub(crate) const LEN_BYTES_NAME: &str = "str_len_bytes";
- IS_EMPTY_NAME · constant · L13-L13 — pub(crate) const IS_EMPTY_NAME: &str = "str_is_empty";
- STARTS_WITH_NAME · constant · L14-L14 — pub(crate) const STARTS_WITH_NAME: &str = "str_starts_with";
- CONTAINS_NAME · constant · L15-L15 — pub(crate) const CONTAINS_NAME: &str = "str_contains";
- LEN_BYTES_ID · constant · L17-L17 — pub(crate) const LEN_BYTES_ID: &str = "core.str.len_bytes";
- IS_EMPTY_ID · constant · L18-L18 — pub(crate) const IS_EMPTY_ID: &str = "core.str.is_empty";
- STARTS_WITH_ID · constant · L19-L19 — pub(crate) const STARTS_WITH_ID: &str = "core.str.starts_with";
- CONTAINS_ID · constant · L20-L20 — pub(crate) const CONTAINS_ID: &str = "core.str.contains";
- MAX_BORROWED_STR_BYTES · constant · L25-L25 — pub(crate) const MAX_BORROWED_STR_BYTES: usize = 65_536;
- contains · function · L30-L69 — pub(crate) fn contains(value: &str, needle: &str) -> Option<bool>
- StrOp · enum · L72-L77 — pub(crate) enum StrOp
- name · function · L80-L87 — pub(crate) const fn name(self) -> &'static str
- id · function · L89-L96 — pub(crate) const fn id(self) -> &'static str
- arity · function · L98-L103 — pub(crate) const fn arity(self) -> usize
- param_names · function · L105-L111 — pub(crate) const fn param_names(self) -> &'static [&'static str]
- param_types · function · L113-L118 — pub(crate) fn param_types(self) -> &'static [ResolvedType]
- return_type · function · L120-L125 — pub(crate) fn return_type(self) -> ResolvedType
- ast_return_type · function · L127-L132 — pub(crate) fn ast_return_type(self) -> Type
- by_name · function · L135-L143 — pub(crate) fn by_name(name: &str) -> Option<StrOp>
- by_id · function · L145-L153 — pub(crate) fn by_id(id: &str) -> Option<StrOp>
- resolved_params · function · L155-L167 — pub(crate) fn resolved_params(op: StrOp) -> Vec<ResolvedParam>
- ast_params · function · L169-L179 — pub(crate) fn ast_params(op: StrOp) -> Vec<Param>
