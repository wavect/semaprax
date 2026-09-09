//! Stack-safe consuming traversal lowering into a checked Step-carried loop.
use super::resolve_expr_frame::Frame;
use super::*;
use crate::ast::Expr;
use std::rc::Rc;

pub(super) struct ForOwnFrame<'a> {
    pub(super) span: Span,
    pub(super) path: String,
    pub(super) statements: &'a [crate::ast::Statement],
    pub(super) tail: &'a Expr,
    pub(super) index: usize,
    pub(super) scope: Rc<BTreeMap<String, Binding>>,
    pub(super) resolved: Vec<ResolvedStatement>,
    pub(super) source: Option<ResolvedExpr>,
}
#[allow(clippy::too_many_arguments)]
pub(super) fn schedule<'a>(
    frames: &mut Vec<Frame<'a>>,
    span: Span,
    path: String,
    statements: &'a [crate::ast::Statement],
    tail: &'a Expr,
    index: usize,
    scope: Rc<BTreeMap<String, Binding>>,
    resolved: Vec<ResolvedStatement>,
) {
    let crate::ast::Statement::ForOwn { values, .. } = &statements[index] else {
        unreachable!()
    };
    let source_path = format!("{path}.s{index}.value.s0.value.arg.0");
    frames.push(Frame::ForOwn(Box::new(ForOwnFrame {
        span,
        path,
        statements,
        tail,
        index,
        scope: scope.clone(),
        resolved,
        source: None,
    })));
    frames.push(Frame::Enter {
        expr: values,
        bindings: scope,
        path: source_path,
    });
}
pub(super) fn resume<'a>(
    resolver: &Resolver<'_>,
    function: &FunctionExecutionId,
    mut frame: ForOwnFrame<'a>,
    frames: &mut Vec<Frame<'a>>,
    results: &mut Vec<ResolvedExpr>,
) -> Result<(), Diagnostic> {
    let crate::ast::Statement::ForOwn {
        item,
        item_span,
        body,
        span,
        ..
    } = &frame.statements[frame.index]
    else {
        unreachable!()
    };
    let statement_path = format!("{}.s{}", frame.path, frame.index);
    if frame.source.is_none() {
        let source = results.pop().expect("consuming traversal source retained");
        let element = source_element(resolver, function, &source)?;
        if frame.scope.contains_key(item) {
            return Err(resolver.error(
                "SPX-H006",
                "consuming iterator item shadows an existing binding",
                *item_span,
            ));
        }
        let mut scope = frame.scope.clone();
        Rc::make_mut(&mut scope).insert(
            item.clone(),
            Binding {
                id: ValueId::local(
                    function,
                    &format!("{statement_path}.value.s1.body.s0.value.arm.1.binding.0"),
                ),
                ownership: crate::iterator_ops::item_ownership(&element, false),
                ty: element,
                mutable: false,
            },
        );
        frame.source = Some(source);
        frames.push(Frame::ForOwn(Box::new(frame)));
        frames.push(Frame::Enter {
            expr: body,
            bindings: scope,
            path: format!("{statement_path}.value.s1.body.s0.value.arm.1.value.s0.value"),
        });
    } else {
        let body = results.pop().expect("consuming traversal body retained");
        let source = frame
            .source
            .take()
            .expect("consuming traversal source retained");
        let element = source_element(resolver, function, &source)?;
        frame.resolved.push(lower(
            function,
            &statement_path,
            item,
            *item_span,
            source,
            element,
            body,
            *span,
        ));
        frames.push(Frame::BlockNext {
            span: frame.span,
            path: frame.path,
            statements: frame.statements,
            tail: frame.tail,
            index: frame.index + 1,
            scope: frame.scope,
            resolved: frame.resolved,
        });
    }
    Ok(())
}
fn source_element(
    resolver: &Resolver<'_>,
    function: &FunctionExecutionId,
    source: &ResolvedExpr,
) -> Result<ResolvedType, Diagnostic> {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = &source.ty
    else {
        return Err(Diagnostic::io(
            "SPX-H006",
            "consuming traversal requires an owning iterator",
        ));
    };
    let [element] = arguments.as_slice() else {
        return Err(Diagnostic::io(
            "SPX-H006",
            "consuming traversal iterator type arity differs",
        ));
    };
    if declaration.as_str() != crate::iterator_ops::ITER_ID
        || source.ownership != OwnershipMode::Own
        || !(crate::iterator_ops::resolved_element_is_admitted(element)
            || super::generic_collection::source_parameter(resolver.program, function, element))
    {
        return Err(Diagnostic::io(
            "SPX-H006",
            "consuming traversal iterator element or ownership is invalid",
        ));
    }
    Ok(element.clone())
}
#[allow(clippy::too_many_arguments)]
fn lower(
    function: &FunctionExecutionId,
    path: &str,
    item: &str,
    item_span: Span,
    source: ResolvedExpr,
    element: ResolvedType,
    user: ResolvedExpr,
    span: Span,
) -> ResolvedStatement {
    let vp = format!("{path}.value");
    let sp = format!("{vp}.s0");
    let wp = format!("{vp}.s1");
    let cp = format!("{wp}.condition");
    let bp = format!("{wp}.body");
    let mp = format!("{bp}.s0.value");
    let step_ty = crate::iterator_ops::resolved_iter_step(element.clone());
    let iter_ty = crate::iterator_ops::resolved_iter(element.clone());
    let expr = |p: &str, ty: ResolvedType, ownership, kind| ResolvedExpr {
        id: ExpressionId::new(function, p),
        ty,
        ownership,
        kind,
        span,
    };
    let binding = |p: &str, name: &str, ty: ResolvedType, ownership| ResolvedBinding {
        id: ValueId::local(function, p),
        name: name.to_owned(),
        ty,
        ownership,
        span,
    };
    let zero = |p: &str| {
        expr(
            p,
            ResolvedType::I64,
            OwnershipMode::Value,
            ResolvedExprKind::Int(0),
        )
    };
    let place = |p: &str, b: &ResolvedBinding| {
        expr(
            p,
            b.ty.clone(),
            b.ownership,
            ResolvedExprKind::Place(Place {
                root: b.id.clone(),
                projections: vec![],
            }),
        )
    };
    let step = binding(&sp, "#for-own-step", step_ty.clone(), OwnershipMode::Own);
    let next = |p: &str, arg: ResolvedExpr| {
        expr(
            p,
            step_ty.clone(),
            OwnershipMode::Own,
            ResolvedExprKind::Call {
                callee: DeclarationId::new(crate::iterator_ops::NEXT_ID),
                type_arguments: vec![element.clone()],
                instance: None,
                args: vec![arg],
            },
        )
    };
    let pattern = |yielded: bool, fields| ResolvedMatchPattern::Variant {
        variant: DeclarationId::new(crate::iterator_ops::STEP_ID),
        case: DeclarationId::new(if yielded {
            crate::iterator_ops::YIELD_ID
        } else {
            crate::iterator_ops::DONE_ID
        }),
        fields,
    };
    let field_bindings = |p: &str, ownership| {
        vec![
            ResolvedMatchPatternField {
                field: DeclarationId::new(crate::iterator_ops::ITEM_ID),
                binding: ResolvedBinding {
                    span: item_span,
                    ..binding(
                        &format!("{p}.binding.0"),
                        item,
                        element.clone(),
                        crate::iterator_ops::item_ownership(
                            &element,
                            ownership == OwnershipMode::Borrow,
                        ),
                    )
                },
            },
            ResolvedMatchPatternField {
                field: DeclarationId::new(crate::iterator_ops::REST_ID),
                binding: binding(
                    &format!("{p}.binding.1"),
                    "#for-own-rest",
                    iter_ty.clone(),
                    ownership,
                ),
            },
        ]
    };
    let arm = |pattern, value| ResolvedMatchArm {
        pattern,
        guard: None,
        value,
        span,
    };
    let condition = expr(
        &cp,
        ResolvedType::Bool,
        OwnershipMode::Value,
        ResolvedExprKind::Match {
            mode: ResolvedMatchMode::Borrow,
            scrutinee: Box::new(place(&format!("{cp}.scrutinee"), &step)),
            arms: vec![
                arm(
                    pattern(false, vec![]),
                    expr(
                        &format!("{cp}.arm.0.value"),
                        ResolvedType::Bool,
                        OwnershipMode::Value,
                        ResolvedExprKind::Bool(false),
                    ),
                ),
                arm(
                    pattern(
                        true,
                        field_bindings(&format!("{cp}.arm.1"), OwnershipMode::Borrow),
                    ),
                    expr(
                        &format!("{cp}.arm.1.value"),
                        ResolvedType::Bool,
                        OwnershipMode::Value,
                        ResolvedExprKind::Bool(true),
                    ),
                ),
            ],
        },
    );
    let fields = field_bindings(&format!("{mp}.arm.1"), OwnershipMode::Own);
    let yp = format!("{mp}.arm.1.value");
    let advance = next(
        &format!("{yp}.tail"),
        place(&format!("{yp}.tail.arg.0"), &fields[1].binding),
    );
    let yielded = expr(
        &yp,
        step_ty.clone(),
        OwnershipMode::Own,
        ResolvedExprKind::Block {
            statements: vec![ResolvedStatement::Let {
                binding: binding(
                    &format!("{yp}.s0"),
                    "#for-own-body",
                    user.ty.clone(),
                    OwnershipMode::Value,
                ),
                mutable: false,
                value: user,
                span,
            }],
            tail: Box::new(advance),
        },
    );
    let done = expr(
        &format!("{mp}.arm.0.value"),
        step_ty.clone(),
        OwnershipMode::Own,
        ResolvedExprKind::ConstructVariant {
            variant: DeclarationId::new(crate::iterator_ops::STEP_ID),
            case: DeclarationId::new(crate::iterator_ops::DONE_ID),
            fields: vec![],
        },
    );
    let replacement = expr(
        &mp,
        step_ty.clone(),
        OwnershipMode::Own,
        ResolvedExprKind::Match {
            mode: ResolvedMatchMode::Own,
            scrutinee: Box::new(place(&format!("{mp}.scrutinee"), &step)),
            arms: vec![
                arm(pattern(false, vec![]), done),
                arm(pattern(true, fields), yielded),
            ],
        },
    );
    let body = expr(
        &bp,
        ResolvedType::I64,
        OwnershipMode::Value,
        ResolvedExprKind::Block {
            statements: vec![ResolvedStatement::Assign {
                binding: step.clone(),
                field: None,
                value: replacement,
                span,
            }],
            tail: Box::new(zero(&format!("{bp}.tail"))),
        },
    );
    ResolvedStatement::Let {
        binding: binding(path, "#for-own", ResolvedType::I64, OwnershipMode::Value),
        mutable: false,
        span,
        value: expr(
            &vp,
            ResolvedType::I64,
            OwnershipMode::Value,
            ResolvedExprKind::Block {
                statements: vec![
                    ResolvedStatement::Let {
                        binding: step,
                        mutable: true,
                        value: next(&format!("{sp}.value"), source),
                        span,
                    },
                    ResolvedStatement::While {
                        condition: Box::new(condition),
                        body: Box::new(body),
                        span,
                    },
                ],
                tail: Box::new(zero(&format!("{vp}.tail"))),
            },
        ),
    }
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_reference(
    resolver: &Resolver<'_>,
    function: &FunctionExecutionId,
    scope: &BTreeMap<String, Binding>,
    path: &str,
    item: &str,
    item_span: Span,
    values: &Expr,
    body: &Expr,
    span: Span,
) -> Result<ResolvedStatement, Diagnostic> {
    let source = resolver.resolve_expr_recursive_reference(
        function,
        values,
        scope,
        &format!("{path}.value.s0.value.arg.0"),
    )?;
    let element = source_element(resolver, function, &source)?;
    if scope.contains_key(item) {
        return Err(resolver.error(
            "SPX-H006",
            "consuming iterator item shadows an existing binding",
            item_span,
        ));
    }
    let mut scope = scope.clone();
    scope.insert(
        item.to_owned(),
        Binding {
            id: ValueId::local(
                function,
                &format!("{path}.value.s1.body.s0.value.arm.1.binding.0"),
            ),
            ty: element.clone(),
            ownership: crate::iterator_ops::item_ownership(&element, false),
            mutable: false,
        },
    );
    let body = resolver.resolve_expr_recursive_reference(
        function,
        body,
        &scope,
        &format!("{path}.value.s1.body.s0.value.arm.1.value.s0.value"),
    )?;
    Ok(lower(
        function, path, item, item_span, source, element, body, span,
    ))
}
#[cfg(test)]
pub(super) fn capacity(
    frame: &ForOwnFrame<'_>,
    seen: &mut std::collections::HashSet<*const BTreeMap<String, Binding>>,
) -> usize {
    use super::capacity_probe::*;
    std::mem::size_of::<ForOwnFrame<'_>>()
        + frame.path.capacity()
        + frame.resolved.capacity() * std::mem::size_of::<ResolvedStatement>()
        + frame
            .resolved
            .iter()
            .map(resolved_statement_owned_capacity)
            .sum::<usize>()
        + frame
            .source
            .as_ref()
            .map_or(0, resolved_expr_owned_capacity)
        + if seen.insert(Rc::as_ptr(&frame.scope)) {
            resolver_scope_owned_capacity(&frame.scope)
        } else {
            0
        }
}

// Shared continuation adapter keeps the legacy Vec traversal unchanged.
pub(super) fn resume_vec<'a>(
    function: &FunctionExecutionId,
    frame: Frame<'a>,
    frames: &mut Vec<Frame<'a>>,
    results: &mut Vec<ResolvedExpr>,
) {
    let Frame::BlockForBody {
        span,
        path,
        statements,
        tail,
        index,
        scope,
        resolved,
        source,
        element,
    } = frame
    else {
        unreachable!()
    };
    super::resolve_for::resume(
        function, frames, results, span, path, statements, tail, index, scope, resolved, source,
        element,
    );
}
#[cfg(test)]
pub(super) fn resolve_statement_reference(
    resolver: &Resolver<'_>,
    function: &FunctionExecutionId,
    scope: &BTreeMap<String, Binding>,
    path: &str,
    statement: &crate::ast::Statement,
) -> Result<ResolvedStatement, Diagnostic> {
    match statement {
        crate::ast::Statement::ForOwn {
            item,
            item_span,
            values,
            body,
            span,
        } => resolve_reference(
            resolver, function, scope, path, item, *item_span, values, body, *span,
        ),
        crate::ast::Statement::For {
            item,
            item_span,
            values,
            body,
            span,
        } => super::resolve_for::resolve_reference(
            resolver, function, scope, path, item, *item_span, values, body, *span,
        ),
        _ => unreachable!(),
    }
}
