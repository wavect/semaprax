# project/candidate/type_declaration.rs

- MAX_TYPE_FIELDS · constant · L16-L16 — pub(super) const MAX_TYPE_FIELDS: usize = 64;
- MAX_TYPE_CASES · constant · L17-L17 — pub(super) const MAX_TYPE_CASES: usize = 64;
- MAX_TYPE_IDENTITIES · constant · L18-L18 — pub(super) const MAX_TYPE_IDENTITIES: usize = 4096;
- Result · type · L19-L19 — type Result<T> = std::result::Result<T, Vec<Diagnostic>>;
- TypeAddition · struct · L21-L28 — pub(super) struct TypeAddition
- Inventory · struct · L30-L35 — struct Inventory<'a>
- add · function · L38-L52 — fn add(&mut self, id: &str, kind: &str, owner: Option<&str>) -> Result<()>
- fields · function · L54-L84 — fn fields(
- field_type · function · L87-L115 — fn field_type(revision: &ProjectRevision, program: &Program, value: &Value) -> Result<Type>
- apply · function · L117-L250 — pub(super) fn apply(
- validate · function · L252-L277 — pub(super) fn validate(
- validate_checked_data · function · L279-L342 — fn validate_checked_data(revision: &ProjectRevision, addition: &TypeAddition) -> Result<()>
- array · function · L344-L352 — fn array(value: &Value, max: usize) -> Result<&[Value]>
- object · function · L353-L361 — fn object(value: &Value, keys: &[&str]) -> Result<()>
- text · function · L362-L366 — fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str>
- grammar · function · L367-L369 — fn grammar(message: &'static str) -> Vec<Diagnostic>
- capacity · function · L370-L372 — fn capacity(message: &'static str) -> Vec<Diagnostic>
