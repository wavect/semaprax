# project/candidate/generic_rename.rs

- Result · type · L20-L20 — type Result<T> = std::result::Result<T, Vec<Diagnostic>>;
- MAX_RETAINED_INSTANCES · constant · L22-L22 — const MAX_RETAINED_INSTANCES: usize = 4096;
- GenericRenamePlan · struct · L25-L31 — pub(super) struct GenericRenamePlan
- plan · function · L33-L98 — pub(super) fn plan(
- validate · function · L100-L128 — pub(super) fn validate(revision: &ProjectRevision, plan: &GenericRenamePlan) -> Result<()>
- normalized · function · L130-L175 — fn normalized(
- normalize_template_spans · function · L177-L192 — fn normalize_template_spans(template: &mut ResolvedFunctionTemplate)
- normalize_expression_spans · function · L194-L293 — fn normalize_expression_spans(expression: &mut ResolvedExpr)
- normalize_statement_spans · function · L295-L327 — fn normalize_statement_spans(statement: &mut ResolvedStatement)
- normalize_pattern_spans · function · L329-L349 — fn normalize_pattern_spans(pattern: &mut ResolvedMatchPattern)
- normalize_record_pattern_spans · function · L351-L361 — fn normalize_record_pattern_spans(pattern: &mut ResolvedRecordMatchFieldPattern)
- normalize_binding_span · function · L363-L365 — fn normalize_binding_span(binding: &mut ResolvedBinding)
- invalid · function · L367-L369 — fn invalid(message: &'static str) -> Vec<Diagnostic>
- capacity · function · L371-L373 — fn capacity(message: &'static str) -> Vec<Diagnostic>
- stale · function · L375-L377 — fn stale(message: &'static str) -> Vec<Diagnostic>
