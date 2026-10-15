//! Canonical lowering for bounded `for item in values` Vec traversal.
use crate::hir::resolver_scope::Scope;

pub(super) mod bytes;
pub(super) mod record;

use crate::ast::{BinaryOp, Span};
use std::collections::BTreeMap;
use std::rc::Rc;

use super::resolve_expr::path_arena::{PathArena, PathId, Seg, Step};
use super::resolve_expr_frame::Frame;
use super::{Binding, ExprPath, Resolver};
use super::{
    DeclarationId, ExpressionId, FunctionExecutionId, OwnershipMode, Place, ResolvedBinding,
    ResolvedExpr, ResolvedExprKind, ResolvedStatement, ResolvedType, ValueId,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn schedule<'expr>(
    resolver: &Resolver<'_>,
    function: &FunctionExecutionId,
    frames: &mut Vec<Frame<'expr>>,
    paths: &mut PathArena<'_>,
    block_span: Span,
    block_path: PathId,
    statements: &'expr [crate::ast::Statement],
    tail: &'expr crate::ast::Expr,
    index: usize,
    scope: Scope<'expr>,
    resolved: Vec<ResolvedStatement>,
    item: &str,
    values: &'expr crate::ast::Expr,
    body: &'expr crate::ast::Expr,
) -> Result<(), crate::diagnostic::Diagnostic> {
    let crate::ast::ExprKind::Var(source_name) = &values.kind else {
        return Err(resolver.error(
            "SPX-H006",
            "for traversal source is not a simple binding",
            values.span,
        ));
    };
    let source_binding = scope.get(source_name).ok_or_else(|| {
        resolver.error(
            "SPX-H002",
            format!("unresolved value `{source_name}`"),
            values.span,
        )
    })?;
    if source_binding.mutable {
        return Err(resolver.error(
            "SPX-H006",
            "for traversal source must be immutable",
            values.span,
        ));
    }
    if scope.contains_key(item) {
        return Err(resolver.error(
            "SPX-H006",
            format!("for loop item `{item}` shadows an existing value"),
            body.span,
        ));
    }
    let element = element(resolver, source_binding, values)?;
    resolver.reject_while_disallowed_in_scope(
        body,
        None,
        bytes::admits(source_binding).then_some(&scope),
    )?;
    let source = ResolvedBinding {
        id: source_binding.id.clone(),
        name: source_name.clone(),
        ownership: source_binding.ownership,
        ty: source_binding.ty.clone(),
        span: values.span,
    };
    let statement = Seg::Statement.at(index);
    let item_edge = [statement.then(Seg::ForItem)];
    let mut body_scope = scope.clone();
    Rc::make_mut(&mut body_scope).to_mut().insert(
        item.to_owned(),
        Binding {
            traversal_row: None,
            id: paths.at(block_path).value_local_with(function, &item_edge),
            ty: element.clone(),
            ownership: OwnershipMode::Value,
            mutable: false,
        },
    );
    frames.push(Frame::BlockForBody {
        span: block_span,
        path: block_path,
        statements,
        tail,
        index,
        scope: scope.clone(),
        resolved,
        source,
        element,
    });
    frames.push(Frame::LeaveLoop);
    frames.push(Frame::Enter {
        expr: body,
        bindings: body_scope,
        path: paths.child(frames, block_path, statement.then(Seg::ForBody)),
    });
    let condition = [statement.then(Seg::ForCondition)];
    frames.push(Frame::EnterLoop(super::ControlTarget::Loop {
        owner: function.clone(),
        site: paths
            .at(block_path)
            .expression_id_with(function, &condition),
    }));
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn resume<'expr>(
    resolver: &Resolver<'_>,
    function: &FunctionExecutionId,
    frames: &mut Vec<Frame<'expr>>,
    paths: &PathArena<'_>,
    results: &mut Vec<ResolvedExpr>,
    block_span: Span,
    block_path: PathId,
    statements: &'expr [crate::ast::Statement],
    tail: &'expr crate::ast::Expr,
    index: usize,
    scope: Scope<'expr>,
    mut resolved: Vec<ResolvedStatement>,
    source: ResolvedBinding,
    element: ResolvedType,
) -> Result<(), crate::diagnostic::Diagnostic> {
    let authored_body = results.pop().expect("for body result retained");
    // The synthesized lowering spells its whole canonical subtree from text.
    let statement = [Seg::Statement.at(index)];
    let path = paths.at(block_path).materialize_with(&statement);
    resolved.push(match &statements[index] {
        crate::ast::Statement::For {
            item,
            item_span,
            span,
            ..
        } => lower_source(
            resolver,
            function,
            &path,
            item,
            *item_span,
            &source,
            element,
            authored_body,
            *span,
        )?,
        crate::ast::Statement::ForBorrow {
            item,
            item_span,
            index,
            span,
            ..
        } => record::lower(
            function,
            &path,
            item,
            *item_span,
            index.as_ref(),
            &source,
            element,
            authored_body,
            *span,
        ),
        _ => unreachable!("for frame resumes at a traversal statement"),
    });
    frames.push(Frame::BlockNext {
        span: block_span,
        path: block_path,
        statements,
        tail,
        index: index + 1,
        scope,
        resolved,
    });
    Ok(())
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_reference(
    resolver: &Resolver<'_>,
    function: &FunctionExecutionId,
    scope: &BTreeMap<String, Binding>,
    statement_path: &str,
    item: &str,
    item_span: Span,
    values: &crate::ast::Expr,
    body: &crate::ast::Expr,
    span: Span,
) -> Result<ResolvedStatement, crate::diagnostic::Diagnostic> {
    let crate::ast::ExprKind::Var(source_name) = &values.kind else {
        return Err(resolver.error(
            "SPX-H006",
            "for traversal source is not a simple binding",
            values.span,
        ));
    };
    let source_binding = scope.get(source_name).ok_or_else(|| {
        resolver.error(
            "SPX-H002",
            format!("unresolved value `{source_name}`"),
            values.span,
        )
    })?;
    if source_binding.mutable {
        return Err(resolver.error(
            "SPX-H006",
            "for traversal source must be immutable",
            values.span,
        ));
    }
    if scope.contains_key(item) {
        return Err(resolver.error(
            "SPX-H006",
            format!("for loop item `{item}` shadows an existing value"),
            body.span,
        ));
    }
    let element = element(resolver, source_binding, values)?;
    resolver.reject_while_disallowed_in_scope(
        body,
        None,
        bytes::admits(source_binding).then_some(scope),
    )?;
    let source = ResolvedBinding {
        id: source_binding.id.clone(),
        name: source_name.clone(),
        ownership: source_binding.ownership,
        ty: source_binding.ty.clone(),
        span: values.span,
    };
    let mut body_scope = scope.clone();
    body_scope.insert(
        item.to_owned(),
        Binding {
            traversal_row: None,
            id: ValueId::local(function, &format!("{statement_path}.value.s2.body.s0")),
            ty: element.clone(),
            ownership: OwnershipMode::Value,
            mutable: false,
        },
    );
    let authored_body = resolver.resolve_expr_recursive_reference_inner(
        function,
        body,
        &body_scope,
        &format!("{statement_path}.value.s2.body.s1.value"),
        Some(&crate::hir::ControlTarget::Loop {
            owner: function.clone(),
            site: ExpressionId::new(function, &format!("{statement_path}.value.s2.condition")),
        }),
    )?;
    lower_source(
        resolver,
        function,
        statement_path,
        item,
        item_span,
        &source,
        element,
        authored_body,
        span,
    )
}

/// The exact traversal item type: a Copy-scalar `Vec<T>` element, or the `u8`
/// of one immutable named `Slice<u8>` view. Every other source stays closed.
fn element(
    resolver: &Resolver<'_>,
    source: &Binding,
    values: &crate::ast::Expr,
) -> Result<ResolvedType, crate::diagnostic::Diagnostic> {
    if bytes::admits(source) {
        return Ok(ResolvedType::U8);
    }
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = &source.ty
    else {
        return Err(resolver.error(
            "SPX-H006",
            "for traversal source is not Vec<T>",
            values.span,
        ));
    };
    if declaration.as_str() != crate::prelude::VEC_ID
        || !matches!(arguments.as_slice(), [element] if crate::vec_ops::resolved_element_is_admitted(element))
    {
        return Err(resolver.error(
            "SPX-H006",
            "for traversal source is outside the Copy-scalar Vec profile",
            values.span,
        ));
    }
    Ok(arguments[0].clone())
}

/// Selects the byte receiver lowering from the already admitted source type.
/// Its receiver backing is debited before construction; a refusal is sticky
/// on the active builder ledger and constructs no traversal.
#[allow(clippy::too_many_arguments)]
fn lower_source(
    resolver: &Resolver<'_>,
    function: &FunctionExecutionId,
    statement_path: &str,
    item_name: &str,
    item_span: Span,
    source: &ResolvedBinding,
    element: ResolvedType,
    authored_body: ResolvedExpr,
    span: Span,
) -> Result<ResolvedStatement, crate::diagnostic::Diagnostic> {
    if source.ty == ResolvedType::SliceU8 {
        bytes::lower(
            function,
            statement_path,
            item_name,
            item_span,
            source,
            authored_body,
            span,
        )
        .ok_or_else(|| {
            resolver.error(
                "SPX-H006",
                "borrowed byte traversal receiver exceeds builder budget",
                span,
            )
        })
    } else {
        Ok(lower(
            function,
            statement_path,
            item_name,
            item_span,
            source,
            element,
            authored_body,
            span,
        ))
    }
}

fn place(
    function: &FunctionExecutionId,
    path: &str,
    binding: &ResolvedBinding,
    span: Span,
) -> ResolvedExpr {
    place_at(ExpressionId::new(function, path), binding, span)
}

fn place_at(id: ExpressionId, binding: &ResolvedBinding, span: Span) -> ResolvedExpr {
    ResolvedExpr {
        id,
        ty: binding.ty.clone(),
        ownership: binding.ownership,
        kind: ResolvedExprKind::Place(Place {
            root: binding.id.clone(),
            projections: Vec::new(),
        }),
        span,
    }
}

fn usize_literal(
    function: &FunctionExecutionId,
    path: &str,
    value: u64,
    span: Span,
) -> ResolvedExpr {
    ResolvedExpr {
        id: ExpressionId::new(function, path),
        ty: ResolvedType::Usize,
        ownership: OwnershipMode::Value,
        kind: ResolvedExprKind::Usize(value),
        span,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower(
    function: &FunctionExecutionId,
    statement_path: &str,
    item_name: &str,
    item_span: Span,
    source: &ResolvedBinding,
    element: ResolvedType,
    authored_body: ResolvedExpr,
    span: Span,
) -> ResolvedStatement {
    let length_arguments = vec![element.clone()];
    lower_profile(
        function,
        statement_path,
        item_name,
        item_span,
        source,
        authored_body,
        span,
        Profile {
            wrapper: "#for",
            length: "#for-length",
            length_callee: crate::vec_ops::LEN_ID,
            length_arguments,
            item: element.clone(),
            kind: super::SourceForKind::Copy,
        },
        |get_value_path, index| ResolvedExpr {
            id: ExpressionId::new(function, get_value_path),
            ty: element.clone(),
            ownership: OwnershipMode::Value,
            kind: ResolvedExprKind::Call {
                callee: DeclarationId::new(crate::vec_ops::GET_ID),
                type_arguments: vec![element],
                instance: None,
                args: vec![
                    place(function, &format!("{get_value_path}.arg.0"), source, span),
                    place(function, &format!("{get_value_path}.arg.1"), index, span),
                ],
            },
            span,
        },
    )
}

/// What distinguishes one source profile on the shared `for` paths: its
/// hidden names, its length snapshot operation, its item type and latch kind.
/// Each profile constructs its own item read in place, so no other profile's
/// carrier is ever built and discarded.
pub(super) struct Profile {
    pub(super) wrapper: &'static str,
    pub(super) length: &'static str,
    pub(super) length_callee: &'static str,
    pub(super) length_arguments: Vec<ResolvedType>,
    pub(super) item: ResolvedType,
    pub(super) kind: super::SourceForKind,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_profile(
    function: &FunctionExecutionId,
    statement_path: &str,
    item_name: &str,
    item_span: Span,
    source: &ResolvedBinding,
    authored_body: ResolvedExpr,
    span: Span,
    profile: Profile,
    read: impl FnOnce(&str, &ResolvedBinding) -> ResolvedExpr,
) -> ResolvedStatement {
    let value_path = format!("{statement_path}.value");
    let length_path = format!("{value_path}.s0");
    let index_path = format!("{value_path}.s1");
    let while_path = format!("{value_path}.s2");
    let while_body_path = format!("{while_path}.body");

    let length = ResolvedBinding {
        id: ValueId::local(function, &length_path),
        name: profile.length.to_owned(),
        ownership: OwnershipMode::Value,
        ty: ResolvedType::Usize,
        span,
    };
    let index = ResolvedBinding {
        id: ValueId::local(function, &index_path),
        name: "#for-index".to_owned(),
        ownership: OwnershipMode::Value,
        ty: ResolvedType::Usize,
        span,
    };
    let item_path = format!("{while_body_path}.s0");
    let item = ResolvedBinding {
        id: ValueId::local(function, &item_path),
        name: item_name.to_owned(),
        ownership: OwnershipMode::Value,
        ty: profile.item,
        span: item_span,
    };
    let authored_path = format!("{while_body_path}.s1");
    let authored = ResolvedBinding {
        id: ValueId::local(function, &authored_path),
        name: "#for-body".to_owned(),
        ownership: authored_body.ownership,
        ty: authored_body.ty.clone(),
        span: authored_body.span,
    };

    let length_value = ResolvedExpr {
        id: ExpressionId::new(function, &format!("{length_path}.value")),
        ty: ResolvedType::Usize,
        ownership: OwnershipMode::Value,
        kind: ResolvedExprKind::Call {
            callee: DeclarationId::new(profile.length_callee),
            type_arguments: profile.length_arguments,
            instance: None,
            args: vec![place(
                function,
                &format!("{length_path}.value.arg.0"),
                source,
                span,
            )],
        },
        span,
    };
    let condition = ResolvedExpr {
        id: ExpressionId::new(function, &format!("{while_path}.condition")),
        ty: ResolvedType::Bool,
        ownership: OwnershipMode::Value,
        kind: ResolvedExprKind::Binary {
            op: BinaryOp::Lt,
            left: Box::new(place(
                function,
                &format!("{while_path}.condition.left"),
                &index,
                span,
            )),
            right: Box::new(place(
                function,
                &format!("{while_path}.condition.right"),
                &length,
                span,
            )),
        },
        span,
    };
    let get_value = read(&format!("{item_path}.value"), &index);
    let increment_path = format!("{while_body_path}.s2.value");
    let increment = ResolvedExpr {
        id: ExpressionId::new(function, &increment_path),
        ty: ResolvedType::Usize,
        ownership: OwnershipMode::Value,
        kind: ResolvedExprKind::Binary {
            op: BinaryOp::Add,
            left: Box::new(place(
                function,
                &format!("{increment_path}.left"),
                &index,
                span,
            )),
            right: Box::new(usize_literal(
                function,
                &format!("{increment_path}.right"),
                1,
                span,
            )),
        },
        span,
    };
    let while_body = ResolvedExpr {
        id: ExpressionId::new(function, &while_body_path),
        ty: ResolvedType::Usize,
        ownership: OwnershipMode::Value,
        kind: ResolvedExprKind::Block {
            statements: vec![
                ResolvedStatement::Let {
                    binding: item,
                    mutable: false,
                    value: get_value,
                    span,
                },
                ResolvedStatement::Let {
                    binding: authored,
                    mutable: false,
                    value: authored_body,
                    span,
                },
                ResolvedStatement::Assign {
                    binding: index.clone(),
                    field: None,
                    value: increment,
                    span,
                },
            ],
            tail: Box::new(usize_literal(
                function,
                &format!("{while_body_path}.tail"),
                0,
                span,
            )),
        },
        span,
    };
    let lowered = ResolvedExpr {
        id: ExpressionId::new(function, &value_path),
        ty: ResolvedType::Usize,
        ownership: OwnershipMode::Value,
        kind: ResolvedExprKind::Block {
            statements: vec![
                ResolvedStatement::Let {
                    binding: length,
                    mutable: false,
                    value: length_value,
                    span,
                },
                ResolvedStatement::Let {
                    binding: index.clone(),
                    mutable: true,
                    value: usize_literal(function, &format!("{index_path}.value"), 0, span),
                    span,
                },
                super::resolve_expr::lexical_exit::loop_statement(
                    Box::new(condition),
                    Box::new(while_body),
                    super::LoopLatchKind::SourceForStep {
                        kind: profile.kind,
                        step: ExpressionId::new(function, &format!("{while_body_path}.s2.value")),
                    },
                    span,
                ),
            ],
            tail: Box::new(usize_literal(
                function,
                &format!("{value_path}.tail"),
                0,
                span,
            )),
        },
        span,
    };
    ResolvedStatement::Let {
        binding: ResolvedBinding {
            id: ValueId::local(function, statement_path),
            name: profile.wrapper.to_owned(),
            ownership: OwnershipMode::Value,
            ty: ResolvedType::Usize,
            span,
        },
        mutable: false,
        value: lowered,
        span,
    }
}
