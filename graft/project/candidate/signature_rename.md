# project/candidate/signature_rename.rs

- Scope · type · L5-L5 — type Scope = BTreeMap<String, Option<String>>;
- invalid · function · L7-L9 — pub(super) fn invalid(message: &'static str) -> Vec<Diagnostic>
- apply · function · L11-L37 — pub(super) fn apply(
- Rename · struct · L39-L44 — struct Rename<'a>
- budget · function · L47-L56 — fn budget(&mut self, depth: usize) -> Result<()>
- reference · function · L58-L69 — fn reference(name: &mut String, scope: &Scope) -> Result<()>
- binding · function · L71-L91 — fn binding(&mut self, name: &mut String, scope: &mut Scope) -> Result<()>
- record_pattern · function · L93-L110 — fn record_pattern(
- pattern · function · L112-L147 — fn pattern(
- expression · function · L149-L262 — fn expression(&mut self, expression: &mut Expr, scope: &Scope, depth: usize) -> Result<()>
