# project/candidate/builtin.rs

- BuiltinOp · enum · L14-L17 — pub(in crate::project::candidate) enum BuiltinOp
- id · function · L20-L25 — pub(in crate::project::candidate) fn id(self) -> &'static str
- name · function · L27-L32 — pub(in crate::project::candidate) fn name(self) -> &'static str
- arity · function · L34-L39 — pub(in crate::project::candidate) fn arity(self) -> usize
- by_id · function · L42-L46 — pub(in crate::project::candidate) fn by_id(target: &str) -> Option<BuiltinOp>
- validate_builtin_namespace · function · L51-L121 — pub(in crate::project::candidate) fn validate_builtin_namespace<'a>(
- plan · function · L123-L137 — pub(super) fn plan(
- source_identities · function · L139-L142 — pub(super) fn source_identities(revision: &ProjectRevision) -> Result<BTreeSet<String>>
- selected · function · L144-L149 — fn selected(identities: &BTreeSet<String>, target: &str) -> Result<Option<BuiltinOp>>
- binding_available · function · L151-L169 — fn binding_available(program: &Program, op: BuiltinOp) -> bool
- builtin_constructors · function · L174-L190 — pub(in crate::project::candidate) fn builtin_constructors(
- builtin_dependency_fingerprint · function · L194-L202 — pub(in crate::project::candidate) fn builtin_dependency_fingerprint(
- implicit_dependency_fingerprint · function · L207-L225 — pub(in crate::project::candidate) fn implicit_dependency_fingerprint(
- descriptor · function · L227-L265 — fn descriptor(op: BuiltinOp) -> Value
- tests · module · L268-L341 — mod tests
- string_descriptors_preserve_exact_owner_signatures_and_byte_family_shape · function · L272-L307 — fn string_descriptors_preserve_exact_owner_signatures_and_byte_family_shape()
- string_selectors_reject_authored_identity_and_import_spelling_collisions · function · L310-L340 — fn string_selectors_reject_authored_identity_and_import_spelling_collisions()
