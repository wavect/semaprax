# assurance_manifest/smt_discharge/replay.rs

- Value · enum · L22-L25 — enum Value
- checked_range · function · L27-L33 — fn checked_range(mode: NumericMode, raw: i128) -> Option<i128>
- ReplayOutcome · enum · L38-L53 — pub enum ReplayOutcome
- model_value_to_runtime · function · L55-L73 — fn model_value_to_runtime(name: &str, sort: Sort, model: &Model) -> Value
- Env · struct · L75-L77 — struct Env
- get · function · L80-L86 — fn get(&self, name: &str) -> Option<Value>
- eval · function · L93-L131 — fn eval(env: &mut Env, expr: &Expr) -> Result<Value, String>
- eval_block · function · L133-L142 — fn eval_block(env: &mut Env, statements: &[Statement], tail: &Expr) -> Result<Value, String>
- eval_binary · function · L144-L214 — fn eval_binary(env: &mut Env, op: BinaryOp, left: &Expr, right: &Expr) -> Result<Value, String>
- replay_function · function · L219-L280 — pub fn replay_function(function: &Function, model: &Model) -> Result<ReplayOutcome, String>
- tests · module · L283-L357 — mod tests
- function · function · L286-L289 — fn function(source: &str) -> Function
- model · function · L291-L296 — fn model(entries: &[(&str, ModelValue)]) -> Model
- validates_a_genuine_ensures_violation · function · L299-L308 — fn validates_a_genuine_ensures_violation()
- validates_a_genuine_overflow_trap · function · L311-L320 — fn validates_a_genuine_overflow_trap()
- a_model_that_actually_satisfies_the_contract_is_inconsistent_not_a_counterexample · function · L323-L332 — fn a_model_that_actually_satisfies_the_contract_is_inconsistent_not_a_counterexample()
- a_model_violating_requires_is_reported_as_inconsistent_not_a_counterexample · function · L335-L344 — fn a_model_violating_requires_is_reported_as_inconsistent_not_a_counterexample()
- a_missing_model_entry_defaults_to_zero_and_can_still_reproduce_a_violation · function · L347-L356 — fn a_missing_model_entry_defaults_to_zero_and_can_still_reproduce_a_violation()
