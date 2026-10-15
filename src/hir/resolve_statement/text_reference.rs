//! Test-only dispatch to the complete recursive text expression receiver.
//! The thread-local guard owns no HIR, source proof or admission authority.
use std::cell::Cell;

thread_local! {
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
    static EXPRESSIONS: Cell<usize> = const { Cell::new(0) };
}

pub(in crate::hir) fn active() -> bool {
    ACTIVE.get()
}

pub(in crate::hir) fn note_expression() {
    EXPRESSIONS.set(EXPRESSIONS.get().checked_add(1).unwrap());
}

pub(in crate::hir) fn with<T>(body: impl FnOnce() -> T) -> (T, usize) {
    struct Restore(bool, usize);
    impl Drop for Restore {
        fn drop(&mut self) {
            ACTIVE.set(self.0);
            EXPRESSIONS.set(self.1);
        }
    }
    let restore = Restore(ACTIVE.replace(true), EXPRESSIONS.replace(0));
    let result = body();
    let expressions = EXPRESSIONS.get();
    drop(restore);
    (result, expressions)
}

#[test]
fn full_current_expression_entry_is_exact_original_plus_test_dispatch() {
    const DECLARATION: &str = "#[cfg(test)]\npub(in crate::hir) mod text_reference;\n\n";
    const DISPATCH: &str = "        #[cfg(test)]\n        if text_reference::active() {\n            text_reference::note_expression();\n            return self.resolve_expr_recursive_reference(function, expr, bindings, path);\n        }\n";
    // The later named-byte ancestry route retains the original receiver:
    // its old entry delegates with no byte scope. Bind both additive hooks
    // exactly, then compare every original byte below.
    const BYTE_SCOPE_ENTRY: &str = r#"        self.reject_while_disallowed_in_scope(expression, execution, None)
    }
    pub(super) fn reject_while_disallowed_in_scope(
        &self,
        expression: &Expr,
        execution: Option<&FunctionExecutionId>,
        byte_entry: Option<&BTreeMap<String, Binding>>,
    ) -> Result<(), Diagnostic> {
"#;
    const BYTE_SCOPE_FOR: &str = r#"                    Statement::For { values, body, .. }
                        if byte_entry.is_some_and(|scope| {
                            matches!(&values.kind, ExprKind::Var(name)
                                if scope.get(name).is_some_and(super::resolve_for::bytes::admits))
                                && execution.is_none_or(|owner| {
                                    owner
                                        .monomorphic_declaration()
                                        .and_then(|id| {
                                            self.program
                                                .functions()
                                                .iter()
                                                .find(|f| f.stable_id == id.as_str())
                                        })
                                        .is_some_and(|function| {
                                            super::resolve_for::bytes::source_byte_ancestry(
                                                &function.body,
                                                expression,
                                                scope,
                                            )
                                        })
                                })
                        }) =>
                    {
                        // Construct only the immutable named-byte profile.
                        // Source verification owns outer-entry availability;
                        // full HIR replay independently proves every ancestor
                        // freeze and exact receiver before backend admission.
                        pending.push(Item::Expression(body));
                        continue;
                    }
"#;
    let current = crate::source_reflows_merge::main_f045_input(
        "src/hir/resolve_statement.rs",
        include_str!("../resolve_statement.rs"),
    );
    let current = current.as_str();
    assert_eq!(current.matches(DECLARATION).count(), 1);
    assert_eq!(current.matches(DISPATCH).count(), 1);
    assert_eq!(current.matches(BYTE_SCOPE_ENTRY).count(), 1);
    assert_eq!(current.matches(BYTE_SCOPE_FOR).count(), 1);
    assert_eq!(
        current
            .replace(DECLARATION, "")
            .replace(DISPATCH, "")
            .replace(BYTE_SCOPE_ENTRY, "")
            .replace(BYTE_SCOPE_FOR, ""),
        include_str!("../../../docs/evidence/path-arena-whole-oracle/resolve_statement-69df.txt")
    );
}
