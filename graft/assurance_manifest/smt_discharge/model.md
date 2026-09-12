# assurance_manifest/smt_discharge/model.rs

- ModelValue · enum · L18-L21 — pub enum ModelValue
- Model · type · L28-L28 — pub type Model = BTreeMap<String, ModelValue>;
- SExpr · enum · L31-L34 — enum SExpr
- tokenize · function · L36-L59 — fn tokenize(text: &str) -> Vec<String>
- parse_one · function · L65-L92 — fn parse_one(tokens: &[String]) -> Result<(SExpr, &[String]), String>
- atom · function · L94-L99 — fn atom(expr: &SExpr) -> Option<&str>
- value_of · function · L105-L129 — fn value_of(expr: &SExpr) -> Result<ModelValue, String>
- parse_model · function · L133-L167 — pub fn parse_model(text: &str) -> Result<Model, String>
- tests · module · L170-L219 — mod tests
- parses_a_simple_int_and_bool_model · function · L174-L179 — fn parses_a_simple_int_and_bool_model()
- parses_a_negative_int_literal · function · L182-L186 — fn parses_a_negative_int_literal()
- rejects_an_uninterpreted_or_nonliteral_value · function · L189-L192 — fn rejects_an_uninterpreted_or_nonliteral_value()
- rejects_a_non_nullary_function_entry · function · L195-L198 — fn rejects_a_non_nullary_function_entry()
- rejects_trailing_garbage_after_the_outer_list · function · L201-L204 — fn rejects_trailing_garbage_after_the_outer_list()
- rejects_a_duplicate_name · function · L207-L210 — fn rejects_a_duplicate_name()
- rejects_a_missing_outer_wrapper_and_an_unterminated_list · function · L213-L218 — fn rejects_a_missing_outer_wrapper_and_an_unterminated_list()
