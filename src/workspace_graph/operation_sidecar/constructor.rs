//! Independently authenticated builtin expansion; original items retain their source sites.
use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn collect(
    program: &Program,
    source: &Expr,
    resolved: &hir::ResolvedExpr,
    tokens: &[crate::lexer::Token],
    owner: &str,
    declaration_index: &BTreeMap<String, usize>,
    import_index: &BTreeMap<(String, ModuleUseKind, String, String), usize>,
    declarations: &mut [WorkspaceOperationDeclaration],
    imports: &mut [WorkspaceOperationImport],
) -> Result<(), Vec<Diagnostic>> {
    let expansion = crate::ast::vec_constructor::inspect(source)
        .map_err(|_| operation_sidecar_disagreement())?;
    hir::vec_constructor::authenticate(resolved).map_err(|_| operation_sidecar_disagreement())?;
    let typed =
        hir::vec_constructor::expansion(resolved).ok_or_else(operation_sidecar_disagreement)?;
    if expansion.count != typed.count {
        return Err(operation_sidecar_disagreement());
    }
    let span = find_identifier_token(tokens, "vec_of", source.span.start, source.span.end)?;
    let mut cursor = span.end;
    collect_operation_type_occurrences(
        program,
        expansion.element,
        typed.element,
        tokens,
        &mut cursor,
        source.span.end,
        Some(owner),
        declaration_index,
        import_index,
        declarations,
        imports,
    )?;
    // Generated builtin names are not authored rename sites. Their exact identities,
    // signatures, capacity and owning chain were authenticated above. Traverse all
    // arguments and the tail without inventing source tokens or another HIR Block.
    for (statement, resolved_statement) in expansion.statements.iter().zip(typed.statements) {
        let (
            crate::ast::Statement::Let { name, value, .. },
            hir::ResolvedStatement::Let {
                binding,
                value: right,
                ..
            },
        ) = (statement, resolved_statement)
        else {
            return Err(operation_sidecar_disagreement());
        };
        if name != &binding.name {
            return Err(operation_sidecar_disagreement());
        }
        let (ExprKind::Call { args, .. }, hir::ResolvedExprKind::Call { args: right, .. }) =
            (&value.kind, &right.kind)
        else {
            return Err(operation_sidecar_disagreement());
        };
        if args.len() != right.len() {
            return Err(operation_sidecar_disagreement());
        }
        for (argument, typed_argument) in args.iter().zip(right) {
            collect_operation_expr_occurrences(
                program,
                argument,
                typed_argument,
                tokens,
                owner,
                declaration_index,
                import_index,
                declarations,
                imports,
            )?;
        }
    }
    let ExprKind::Block { tail, .. } = &expansion.value.kind else {
        return Err(operation_sidecar_disagreement());
    };
    collect_operation_expr_occurrences(
        program,
        tail,
        typed.tail,
        tokens,
        owner,
        declaration_index,
        import_index,
        declarations,
        imports,
    )
}

// The old operation view held five borrowed slices/strs. Indexed views retain
// two extra words each for types/functions/templates; reserve the actual delta
// before creating the operation directory. This does not pay HIR body copies.
pub(super) fn reserve_module_views(count: usize) -> Result<(), Vec<Diagnostic>> {
    type Old<'a> = (
        &'a str,
        &'a [hir::ResolvedTypeDeclaration],
        &'a [hir::ResolvedInterface],
        &'a [hir::ResolvedFunction],
        &'a [hir::ResolvedFunctionTemplate],
    );
    let delta = std::mem::size_of::<super::OperationModule<'_>>()
        .saturating_sub(std::mem::size_of::<Old<'_>>());
    super::reserve_builder_structure(count.checked_mul(delta).ok_or_else(|| {
        vec![super::limit_error(
            "builder_bytes",
            super::active_builder_limit(),
        )]
    })?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ast_count_and_erased_hir_marker_fail_before_token_lookup() {
        let source = crate::parse(r#"module constructor; @id("main") fn main()->i64 { let rows=vec_of<i64>(19); if vec_len<i64>(rows)==1usize {42}else{0} }"#, std::path::Path::new("constructor.spx")).unwrap();
        let typed = hir::resolve(&source).unwrap();
        let ExprKind::Block { statements, .. } = &source.functions[0].body.kind else {
            panic!("body")
        };
        let crate::ast::Statement::Let { value: marker, .. } = &statements[0] else {
            panic!("marker")
        };
        let hir::ResolvedExprKind::Block { statements, .. } = &typed.functions[0].body.kind else {
            panic!("body")
        };
        let hir::ResolvedStatement::Let {
            value: typed_marker,
            ..
        } = &statements[0]
        else {
            panic!("marker")
        };
        let mut altered = marker.clone();
        let ExprKind::VecConstructor { count, .. } = &mut altered.kind else {
            panic!("marker")
        };
        *count += 1;
        let mut erased = typed_marker.clone();
        let hir::ResolvedExprKind::VecConstructor {
            statements, tail, ..
        } = erased.kind
        else {
            panic!("marker")
        };
        erased.kind = hir::ResolvedExprKind::Block { statements, tail };
        for (authored, retained) in [(&altered, typed_marker), (marker, &erased)] {
            assert_eq!(
                collect(
                    &source,
                    authored,
                    retained,
                    &[],
                    "main",
                    &Default::default(),
                    &Default::default(),
                    &mut [],
                    &mut []
                )
                .unwrap_err()[0]
                    .code,
                "SPX-G173"
            );
        }
    }
}
