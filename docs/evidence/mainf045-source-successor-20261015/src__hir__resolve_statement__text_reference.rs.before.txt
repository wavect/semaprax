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
    let current = include_str!("../resolve_statement.rs");
    assert_eq!(current.matches(DECLARATION).count(), 1);
    assert_eq!(current.matches(DISPATCH).count(), 1);
    assert_eq!(
        current.replace(DECLARATION, "").replace(DISPATCH, ""),
        include_str!("../../../docs/evidence/path-arena-whole-oracle/resolve_statement-69df.txt")
    );
}
