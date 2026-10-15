//! Source-only row aliases lower to existing authenticated reads, never storage.
use super::*;
use crate::hir::resolve_for::record::Row;

#[derive(Clone)]
pub(in crate::hir) struct FrozenRow {
    pub(in crate::hir) source: ResolvedBinding,
    pub(in crate::hir) index: ResolvedBinding,
    pub(in crate::hir) element: ResolvedType,
    /// Lowered loop region and its length snapshot, named by Row Loan v1.
    pub(in crate::hir) region: ExpressionId,
    pub(in crate::hir) bound: ValueId,
}

#[allow(clippy::too_many_arguments)]
pub(in crate::hir) fn schedule<'expr>(
    resolver: &Resolver<'_, '_>,
    function: &FunctionExecutionId,
    frames: &mut Vec<Frame<'expr>>,
    paths: &mut PathArena<'_>,
    block_span: Span,
    block_path: PathId,
    statements: &'expr [crate::ast::Statement],
    tail: &'expr crate::ast::Expr,
    position: usize,
    scope: Rc<BTreeMap<String, Binding>>,
    resolved: Vec<ResolvedStatement>,
) -> Result<(), crate::diagnostic::Diagnostic> {
    let crate::ast::Statement::ForBorrow {
        item,
        item_span,
        index,
        values,
        body,
        ..
    } = &statements[position]
    else {
        unreachable!()
    };
    let statement = Seg::Statement.at(position);
    let (source, element, body_scope) = prepare(
        resolver,
        function,
        scope.clone(),
        item,
        *item_span,
        index.as_ref(),
        values,
        body,
        paths.at(block_path),
        statement.then(Seg::ForItem),
        statement.then(Seg::Value),
    )?;
    frames.push(Frame::BlockForBody {
        span: block_span,
        path: block_path,
        statements,
        tail,
        index: position,
        scope,
        resolved,
        source,
        element: element.clone(),
    });
    frames.push(Frame::LeaveLoop);
    frames.push(Frame::Enter {
        expr: body,
        bindings: body_scope,
        path: paths.child(frames, block_path, statement.then(Seg::ForBody)),
    });
    let condition = [statement.then(Seg::ForCondition)];
    frames.push(Frame::EnterLoop(super::super::ControlTarget::Loop {
        owner: function.clone(),
        site: paths
            .at(block_path)
            .expression_id_with(function, &condition),
    }));
    Ok(())
}

pub(in crate::hir) fn project<'p>(
    resolver: &Resolver<'_, '_>,
    function: &FunctionExecutionId,
    expression: &crate::ast::Expr,
    bindings: &BTreeMap<String, Binding>,
    path: impl Into<ExprPath<'p>>,
) -> Result<Option<ResolvedExpr>, crate::diagnostic::Diagnostic> {
    let crate::ast::ExprKind::Project { base, field, .. } = &expression.kind else {
        return Ok(None);
    };
    let crate::ast::ExprKind::Var(name) = &base.kind else {
        return Ok(None);
    };
    let Some(row) = bindings
        .get(name)
        .and_then(|binding| binding.traversal_row.as_deref())
    else {
        return Ok(None);
    };
    let ResolvedType::Nominal { declaration, .. } = &row.element else {
        unreachable!()
    };
    let field_id = resolver
        .declarations
        .field_id(declaration, field)
        .cloned()
        .ok_or_else(|| {
            resolver.error(
                "SPX-H001",
                "record traversal field is absent from its exact nominal layout",
                expression.span,
            )
        })?;
    let path = path.into();
    let source = place_at(
        path.expression_id_with(function, &[Seg::ArgZero.into()]),
        &row.source,
        expression.span,
    );
    let index = place_at(
        path.expression_id_with(function, &[Seg::ArgOne.into()]),
        &row.index,
        expression.span,
    );
    if super::super::owned_leaf_collection::layout(&resolver.declarations, &row.element)
        .is_some_and(|layout| layout.fields.is_some())
    {
        return resolver
            .finish_vec_field(
                function,
                expression.span,
                path,
                row.element.clone(),
                field_id,
                vec![source, index],
            )
            .map(Some);
    }
    let fields = resolver
        .declarations
        .record_fields(declaration)
        .ok_or_else(|| {
            resolver.error(
                "SPX-H001",
                "record traversal nominal layout is absent",
                expression.span,
            )
        })?;
    let ty = fields
        .iter()
        .find(|field| field.id == field_id)
        .ok_or_else(|| {
            resolver.error(
                "SPX-H001",
                "record traversal field identity is absent",
                expression.span,
            )
        })?
        .ty
        .clone();
    let source = place_at(
        path.expression_id_with(function, &[Seg::BaseArgZero.into()]),
        &row.source,
        expression.span,
    );
    let index = place_at(
        path.expression_id_with(function, &[Seg::BaseArgOne.into()]),
        &row.index,
        expression.span,
    );
    let base = ResolvedExpr {
        id: path.expression_id_with(function, &[Seg::Base.into()]),
        ty: row.element.clone(),
        ownership: OwnershipMode::Value,
        kind: ResolvedExprKind::Call {
            callee: DeclarationId::new(crate::vec_ops::GET_ID),
            type_arguments: vec![row.element.clone()],
            instance: None,
            args: vec![source, index],
        },
        span: expression.span,
    };
    Ok(Some(ResolvedExpr {
        id: path.expression_id(function),
        ty,
        ownership: OwnershipMode::Value,
        kind: ResolvedExprKind::Project {
            base: Box::new(base),
            field: field_id,
        },
        span: expression.span,
    }))
}

#[allow(clippy::too_many_arguments)]
pub(in crate::hir) fn lower(
    function: &FunctionExecutionId,
    path: &str,
    item: &str,
    item_span: Span,
    index: Option<&crate::ast::TraversalIndex>,
    source: &ResolvedBinding,
    element: ResolvedType,
    authored_body: ResolvedExpr,
    span: Span,
) -> ResolvedStatement {
    let mut statement = super::lower(
        function,
        path,
        item,
        item_span,
        source,
        element,
        authored_body,
        span,
    );
    let ResolvedStatement::Let { binding, value, .. } = &mut statement else {
        unreachable!()
    };
    binding.name = "#for-borrow".into();
    let ResolvedExprKind::Block { statements, .. } = &mut value.kind else {
        unreachable!()
    };
    let ResolvedStatement::Let {
        binding: length, ..
    } = &mut statements[0]
    else {
        unreachable!()
    };
    length.name = "#for-borrow-length".into();
    let ResolvedStatement::Let {
        binding: counter, ..
    } = &statements[1]
    else {
        unreachable!()
    };
    let counter = counter.clone();
    let body = match &mut statements[2] {
        ResolvedStatement::While { body, .. } => body,
        ResolvedStatement::LexicalWhile { body, latch, .. } => {
            *latch = crate::hir::LoopLatchKind::SourceForStep {
                kind: crate::hir::SourceForKind::BorrowRecord,
                step: ExpressionId::new(function, &format!("{path}.value.s2.body.s2.value")),
            };
            body
        }
        _ => unreachable!(),
    };
    let ResolvedExprKind::Block { statements, .. } = &mut body.kind else {
        unreachable!()
    };
    let ResolvedStatement::Let { binding, value, .. } = &mut statements[0] else {
        unreachable!()
    };
    binding.name = index
        .map_or("#for-row-index", |index| index.name.as_str())
        .into();
    binding.ty = ResolvedType::Usize;
    *value = place(
        function,
        &format!("{path}.value.s2.body.s0.value"),
        &counter,
        span,
    );
    statement
}

type PreparedTraversal = (ResolvedBinding, ResolvedType, Rc<BTreeMap<String, Binding>>);

#[allow(clippy::too_many_arguments)]
fn prepare(
    resolver: &Resolver<'_, '_>,
    function: &FunctionExecutionId,
    scope: Rc<BTreeMap<String, Binding>>,
    item: &str,
    item_span: Span,
    index: Option<&crate::ast::TraversalIndex>,
    values: &crate::ast::Expr,
    body: &crate::ast::Expr,
    statement: ExprPath<'_>,
    item_edge: Step,
    region_edge: Step,
) -> Result<PreparedTraversal, crate::diagnostic::Diagnostic> {
    let fail = || {
        resolver.error("SPX-H006", "borrowed record traversal requires an immutable named Vec of an admitted explicit record", values.span)
    };
    let crate::ast::ExprKind::Var(name) = &values.kind else {
        return Err(fail());
    };
    let source_binding = scope.get(name).ok_or_else(fail)?;
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = &source_binding.ty
    else {
        return Err(fail());
    };
    let [element] = arguments.as_slice() else {
        return Err(fail());
    };
    if source_binding.mutable
        || declaration.as_str() != crate::prelude::VEC_ID
        || !matches!(
            source_binding.ownership,
            OwnershipMode::Own | OwnershipMode::Borrow
        )
        || !(super::super::copy_record_collection::admitted(&resolver.declarations, element)
            || super::super::owned_leaf_collection::layout(&resolver.declarations, element)
                .is_some_and(|layout| layout.fields.is_some()))
        || function.instance().is_some()
    {
        return Err(fail());
    }
    if scope.contains_key(item)
        || index
            .as_ref()
            .is_some_and(|index| index.name == item || scope.contains_key(&index.name))
    {
        return Err(resolver.error(
            "SPX-H006",
            "record traversal row and index must be fresh distinct bindings",
            item_span,
        ));
    }
    resolver.reject_while_disallowed(body)?;
    let source = ResolvedBinding {
        id: source_binding.id.clone(),
        name: name.clone(),
        ownership: source_binding.ownership,
        ty: source_binding.ty.clone(),
        span: values.span,
    };
    let index_binding = ResolvedBinding {
        id: statement.value_local_with(function, &[item_edge]),
        name: index
            .as_ref()
            .map_or("#for-row-index", |index| index.name.as_str())
            .to_owned(),
        ownership: OwnershipMode::Value,
        ty: ResolvedType::Usize,
        span: index.map_or(item_span, |index| index.span),
    };
    let mut body_scope = scope.clone();
    let mutable = Rc::make_mut(&mut body_scope);
    if let Some(index) = index {
        mutable.insert(
            index.name.clone(),
            Binding {
                id: index_binding.id.clone(),
                ty: ResolvedType::Usize,
                ownership: OwnershipMode::Value,
                mutable: false,
                traversal_row: None,
            },
        );
    }
    mutable.insert(
        item.to_owned(),
        Binding {
            id: index_binding.id.clone(),
            ty: element.clone(),
            ownership: OwnershipMode::Borrow,
            mutable: false,
            traversal_row: Some(Box::new(Row {
                source: source.clone(),
                index: index_binding,
                element: element.clone(),
                region: statement.expression_id_with(function, &[region_edge]),
                bound: statement.value_local_with(function, &[region_edge, Seg::Statement.at(0)]),
            })),
        },
    );
    Ok((source, element.clone(), body_scope))
}

#[cfg(test)]
pub(in crate::hir) fn resolve_reference(
    resolver: &Resolver<'_, '_>,
    function: &FunctionExecutionId,
    scope: &BTreeMap<String, Binding>,
    path: &str,
    statement: &crate::ast::Statement,
) -> Result<ResolvedStatement, crate::diagnostic::Diagnostic> {
    let crate::ast::Statement::ForBorrow {
        item,
        item_span,
        index,
        values,
        body,
        span,
    } = statement
    else {
        unreachable!()
    };
    let (source, element, body_scope) = prepare(
        resolver,
        function,
        Rc::new(scope.clone()),
        item,
        *item_span,
        index.as_ref(),
        values,
        body,
        path.into(),
        Seg::ForItem.into(),
        Seg::Value.into(),
    )?;
    let body = resolver.resolve_expr_recursive_reference_inner(
        function,
        body,
        &body_scope,
        &format!("{path}.value.s2.body.s1.value"),
        Some(&crate::hir::ControlTarget::Loop {
            owner: function.clone(),
            site: ExpressionId::new(function, &format!("{path}.value.s2.condition")),
        }),
    )?;
    Ok(lower(
        function,
        path,
        item,
        *item_span,
        index.as_ref(),
        &source,
        element,
        body,
        *span,
    ))
}
