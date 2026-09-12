# project/candidate/type_rename.rs

- Result · type · L11-L11 — type Result<T> = std::result::Result<T, Vec<Diagnostic>>;
- NominalRename · struct · L14-L16 — pub(super) struct NominalRename
- eligible · function · L20-L23 — pub(super) fn eligible(revision: &ProjectRevision, target: &str) -> Result<bool>
- member_kind · function · L26-L32 — pub(super) fn member_kind(
- Selection · struct · L34-L37 — struct Selection<'a>
- selection · function · L39-L89 — fn selection<'a>(programs: &'a [Program], target: &str) -> Result<Option<Selection<'a>>>
- apply · function · L91-L167 — pub(super) fn apply(
- validate · function · L169-L184 — pub(super) fn validate(after: &ProjectRevision, rename: &NominalRename) -> Result<()>
- invalid · function · L186-L188 — fn invalid(message: &'static str) -> Vec<Diagnostic>
