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

// This separately named reference corrects only the frozen iterator argument
// spelling. It is not unchanged old behavior or an allocation/cost baseline.
use crate::ast::{Span, Type};
use crate::diagnostic::Diagnostic;
use crate::hir::resolve_box_call::ReferenceCall;
use crate::hir::{
    DeclarationId, ExprPath, FunctionExecutionId, OwnershipMode, ResolvedExpr, ResolvedExprKind,
    ResolvedType, Resolver,
};
use crate::iterator_ops::IteratorOp;

thread_local! {
    static CANONICAL_ITERATOR_PATH: Cell<bool> = const { Cell::new(false) };
}

pub(in crate::hir) fn canonical_iterator_path_active() -> bool {
    CANONICAL_ITERATOR_PATH.get()
}

pub(in crate::hir) fn with_canonical_iterator_path<T>(body: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            CANONICAL_ITERATOR_PATH.set(self.0);
        }
    }
    let restore = Restore(CANONICAL_ITERATOR_PATH.replace(true));
    let result = body();
    drop(restore);
    result
}

fn canonical_iterator_finish<'p>(
    function: &FunctionExecutionId,
    path: impl Into<ExprPath<'p>>,
    span: Span,
    op: IteratorOp,
    element: ResolvedType,
    args: Vec<ResolvedExpr>,
) -> Result<ResolvedExpr, Diagnostic> {
    if !matches!(args.as_slice(),[value] if value.ty==op.resolved_param_type(&element)&&value.ownership==OwnershipMode::Own)
    {
        return Err(Diagnostic::io(
            "SPX-H006",
            "iterator operation requires its exact owned carrier",
        ));
    }
    Ok(ResolvedExpr {
        id: path.into().expression_id(function),
        ownership: OwnershipMode::Own,
        ty: op.resolved_return_type(&element),
        kind: ResolvedExprKind::Call {
            callee: DeclarationId::compiler(crate::hir::CompilerDeclaration::IteratorOperation(op)),
            type_arguments: vec![element],
            instance: None,
            args,
        },
        span,
    })
}

#[cfg(test)]
pub(in crate::hir) fn canonical_iterator_reference(
    resolver: &Resolver<'_, '_>,
    function: &FunctionExecutionId,
    call: ReferenceCall<'_>,
    op: IteratorOp,
) -> Result<ResolvedExpr, Diagnostic> {
    if call.type_arguments.len() != 1 || call.args.len() != 1 {
        return Err(Diagnostic::io("SPX-H006", "invalid iterator call shape"));
    }
    let element =
        canonical_iterator_element(resolver, function, &call.type_arguments[0], call.span)?;
    if !crate::iterator_ops::resolved_element_is_admitted_in(&resolver.declarations, &element)
        && !crate::hir::generic_collection::source_parameter(resolver.program, function, &element)
    {
        return Err(Diagnostic::io("SPX-H006", "invalid iterator element"));
    }
    let argument = resolver.resolve_expr_recursive_reference(
        function,
        &call.args[0],
        call.bindings,
        &format!("{}.arg.0", call.path),
    )?;
    canonical_iterator_finish(function, call.path, call.span, op, element, vec![argument])
}

fn canonical_iterator_element(
    resolver: &Resolver<'_, '_>,
    function: &FunctionExecutionId,
    ty: &Type,
    span: Span,
) -> Result<ResolvedType, Diagnostic> {
    if let FunctionExecutionId::Monomorphic(owner) = function {
        if let Some(candidate) = resolver.source_functions().find(|candidate| {
            candidate.payload().stable_id == owner.as_str()
                && crate::source_verify::generic_collection_profile(candidate.payload())
        }) {
            return resolver.resolve_function_type_ref(candidate, ty, span);
        }
    }
    resolver.resolve_type(ty, span)
}

pub(in crate::hir) fn assert_canonical_iterator_reference_source() {
    fn sha(source: &str) -> String {
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(<sha2::Sha256 as sha2::Digest>::digest(source.as_bytes()))
        )
    }
    let original = include_str!("../resolve_box_call/iterator.rs");
    assert_eq!(
        sha(original),
        "baae64178a8d976efc9b4878012de2ddb3b21156d85cfcb481bab4188e5c03c6"
    );
    let source = include_str!("text_reference.rs");
    let prefix = source
        .split("\n// This separately named reference corrects only the frozen iterator argument\n")
        .next()
        .unwrap();
    assert_eq!(
        sha(prefix),
        "5a48e8f408e3816971541426d79f40bef15a2477675c57ab0df30a3f587dadd9"
    );
    let current_finish = source[source.find("fn canonical_iterator_finish<'p>(").unwrap()..]
        .split("\n#[cfg(test)]")
        .next()
        .unwrap()
        .trim();
    let original_finish = original[original.find("pub(super) fn finish<'p>(").unwrap()..]
        .split("\n#[cfg(test)]")
        .next()
        .unwrap()
        .trim();
    assert_eq!(
        current_finish.replacen("fn canonical_iterator_finish", "pub(super) fn finish", 1),
        original_finish
    );
    let current_reference = source[source
        .find("pub(in crate::hir) fn canonical_iterator_reference(")
        .unwrap()..]
        .split("\nfn canonical_iterator_element")
        .next()
        .unwrap()
        .trim();
    let original_reference = original[original.find("pub(super) fn reference(").unwrap()..]
        .split("\nfn resolve_element")
        .next()
        .unwrap()
        .trim();
    assert_eq!(current_reference.matches("{}.arg.0").count(), 1);
    assert_eq!(
        current_reference
            .replacen(
                "pub(in crate::hir) fn canonical_iterator_reference",
                "pub(super) fn reference",
                1
            )
            .replace("canonical_iterator_element(", "resolve_element(")
            .replace("canonical_iterator_finish(", "finish(")
            .replace(
                "crate::hir::generic_collection::",
                "super::super::generic_collection::"
            )
            .replacen("{}.arg.0", "{}.arg0", 1),
        original_reference
    );
    let current_element = source[source.find("fn canonical_iterator_element(").unwrap()..]
        .split("\npub(in crate::hir) fn assert_canonical_iterator_reference_source")
        .next()
        .unwrap()
        .trim();
    let original_element = original[original.find("fn resolve_element(").unwrap()..]
        .split("\n#[cfg(test)]")
        .next()
        .unwrap()
        .trim();
    assert_eq!(
        current_element.replacen("fn canonical_iterator_element", "fn resolve_element", 1),
        original_element
    );
    let dispatch = include_str!("../resolve_box_call.rs");
    const CORRECTED: &str = r#"            Self::Iterator(op) => {
                if super::resolve_statement::text_reference::canonical_iterator_path_active() {
                    super::resolve_statement::text_reference::canonical_iterator_reference(
                        resolver, function, call, op,
                    )
                } else {
                    iterator::reference(resolver, function, call, op)
                }
            }"#;
    const ORIGINAL: &str =
        r#"            Self::Iterator(op) => iterator::reference(resolver, function, call, op),"#;
    assert_eq!(dispatch.matches(CORRECTED).count(), 1);
    assert_eq!(
        sha(&dispatch.replacen(CORRECTED, ORIGINAL, 1)),
        "b8be260eeda8912016a9b16dc2aa18b547dd4502e424a929e34efa5cfe6a23f5"
    );
}
