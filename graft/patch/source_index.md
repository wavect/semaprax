# patch/source_index.rs

- MemberSite · struct · L15-L18 — pub(super) struct MemberSite
- CallSite · struct · L21-L27 — pub(super) struct CallSite
- SemanticSourceIndex · struct · L30-L40 — pub(super) struct SemanticSourceIndex
- build · function · L43-L97 — pub fn build(program: &Program, resolved: &ResolvedProgram, tokens: &[Token]) -> Option<Self>
- member · function · L99-L117 — fn member(&mut self, owner: &str, field: &str, span: Span, shorthand_binding: Option<String>)
- case · function · L119-L124 — fn case(&mut self, owner: &str, case: &str, span: Span)
- expr · function · L126-L370 — fn expr(&mut self, source: &Expr, resolved: &ResolvedExpr, tokens: &[Token]) -> Option<()>
- expr_pairs · function · L372-L385 — fn expr_pairs(
- pattern · function · L387-L426 — fn pattern(&mut self, source: &MatchPattern, resolved: &ResolvedMatchPattern) -> Option<()>
- record_pattern · function · L428-L463 — fn record_pattern(
- collect_project_spans · function · L466-L474 — fn collect_project_spans(expression: &Expr, spans: &mut Vec<Span>)
- call_type_argument_spans · function · L476-L503 — fn call_type_argument_spans(span: Span, count: usize, tokens: &[Token]) -> Option<Vec<Span>>
