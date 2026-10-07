//! Dynamically scoped, authenticated no-allocation String condition reads.

use super::*;

impl Evaluator<'_> {
    pub(super) fn evaluate_while(
        &mut self,
        condition: &ResolvedExpr,
        body: &ResolvedExpr,
        environment: &mut Environment,
        depth: usize,
    ) -> Result<(), Flow> {
        let mut reads = crate::string_ops::conditions::condition_reads(condition);
        loop {
            self.charge()?;
            // Restore the enclosing context before either the body or an
            // error exit, and reuse the derived set across physical iterations.
            std::mem::swap(&mut self.string_condition_reads, &mut reads);
            let condition_value = self.evaluate(condition, environment, depth);
            std::mem::swap(&mut self.string_condition_reads, &mut reads);
            let flag = match condition_value? {
                Value::Bool(flag) => flag,
                _ => return Err(Flow::Guard("non-boolean while condition")),
            };
            if !flag {
                return Ok(());
            }
            self.semantic_charge()?;
            self.evaluate(body, environment, depth)?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_length_condition_preserves_fuel_without_utf8_materialization() {
        let source = "module test.condition; @id(\"condition.main\") fn main() -> i64 { let text = \"abc\"; let mut i = 0; while i < 1000 && string_len(text) == 3 { i = i + 1; 0 } i }";
        let program = parse(source, Path::new("condition.spx")).unwrap();
        assert!(verify::verify(&program).is_empty());
        let resolved = hir::resolve(&program).unwrap();
        hir::validate(&resolved).unwrap();
        let admitted = resolved
            .functions
            .iter()
            .map(|function| (function.id.as_str(), function))
            .collect();
        let entry = &resolved.functions[0];
        let (outcome, steps, _, usage) = evaluate_resolved_entry_with_utf8_budget(
            entry,
            &[],
            &admitted,
            &resolved,
            100_000,
            false,
            Utf8MaterializationBudget::fixed(),
        );
        assert!(matches!(outcome, Ok(Value::Int(1000))));
        assert_eq!(usage, (1, 3), "only the initial literal materializes UTF-8");
        let (outcome, exhausted_steps, _, _) = evaluate_resolved_entry_with_utf8_budget(
            entry,
            &[],
            &admitted,
            &resolved,
            steps - 1,
            false,
            Utf8MaterializationBudget::fixed(),
        );
        assert!(matches!(outcome, Err(Flow::Exhausted)));
        assert_eq!(exhausted_steps, steps - 1);
    }

    #[test]
    fn named_predicate_conditions_preserve_fuel_without_utf8_materialization() {
        let source = "module test.condition; @id(\"condition.main\") fn main() -> i64 { let text = \"abc\"; let prefix = \"a\"; let needle = \"b\"; let mut i = 0; while i < 1000 && string_starts_with(text, prefix) && string_contains(text, needle) { i = i + 1; 0 } i }";
        let program = parse(source, Path::new("condition.spx")).unwrap();
        assert!(verify::verify(&program).is_empty());
        let resolved = hir::resolve(&program).unwrap();
        hir::validate(&resolved).unwrap();
        let admitted = resolved
            .functions
            .iter()
            .map(|function| (function.id.as_str(), function))
            .collect();
        let entry = &resolved.functions[0];
        let (outcome, steps, _, usage) = evaluate_resolved_entry_with_utf8_budget(
            entry,
            &[],
            &admitted,
            &resolved,
            100_000,
            false,
            Utf8MaterializationBudget::fixed(),
        );
        assert!(matches!(outcome, Ok(Value::Int(1000))));
        assert_eq!(
            usage,
            (3, 5),
            "only the three initial literals materialize UTF-8"
        );
        let (outcome, exhausted_steps, _, _) = evaluate_resolved_entry_with_utf8_budget(
            entry,
            &[],
            &admitted,
            &resolved,
            steps - 1,
            false,
            Utf8MaterializationBudget::fixed(),
        );
        assert!(matches!(outcome, Err(Flow::Exhausted)));
        assert_eq!(exhausted_steps, steps - 1);
    }
}
