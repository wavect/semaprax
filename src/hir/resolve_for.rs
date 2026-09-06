//! Canonical lowering for bounded `for item in values` Vec traversal.

use crate::ast::{BinaryOp, Span};
use std::collections::BTreeMap;
use std::rc::Rc;

use super::resolve_expr_frame::Frame;
use super::{Binding, Resolver};
use super::{
    DeclarationId, ExpressionId, FunctionExecutionId, OwnershipMode, Place, ResolvedBinding,
    ResolvedExpr, ResolvedExprKind, ResolvedStatement, ResolvedType, ValueId,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn schedule<'expr>(
    resolver: &Resolver<'_>,
    function: &FunctionExecutionId,
    frames: &mut Vec<Frame<'expr>>,
    block_span: Span,
    block_path: &str,
    statements: &'expr [crate::ast::Statement],
    tail: &'expr crate::ast::Expr,
    index: usize,
    scope: Rc<BTreeMap<String, Binding>>,
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
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = &source_binding.ty
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
    resolver.reject_while_disallowed(body)?;
    let source = ResolvedBinding {
        id: source_binding.id.clone(),
        name: source_name.clone(),
        ownership: source_binding.ownership,
        ty: source_binding.ty.clone(),
        span: values.span,
    };
    let element = arguments[0].clone();
    let statement_path = format!("{block_path}.s{index}");
    let mut body_scope = scope.clone();
    Rc::make_mut(&mut body_scope).insert(
        item.to_owned(),
        Binding {
            id: ValueId::local(function, &format!("{statement_path}.value.s2.body.s0")),
            ty: element.clone(),
            ownership: OwnershipMode::Value,
            mutable: false,
        },
    );
    frames.push(Frame::BlockForBody {
        span: block_span,
        path: block_path.to_owned(),
        statements,
        tail,
        index,
        scope: scope.clone(),
        resolved,
        source,
        element,
    });
    frames.push(Frame::Enter {
        expr: body,
        bindings: body_scope,
        path: format!("{statement_path}.value.s2.body.s1.value"),
    });
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn resume<'expr>(
    function: &FunctionExecutionId,
    frames: &mut Vec<Frame<'expr>>,
    results: &mut Vec<ResolvedExpr>,
    block_span: Span,
    block_path: String,
    statements: &'expr [crate::ast::Statement],
    tail: &'expr crate::ast::Expr,
    index: usize,
    scope: Rc<BTreeMap<String, Binding>>,
    mut resolved: Vec<ResolvedStatement>,
    source: ResolvedBinding,
    element: ResolvedType,
) {
    let authored_body = results.pop().expect("for body result retained");
    let crate::ast::Statement::For {
        item,
        item_span,
        span,
        ..
    } = &statements[index]
    else {
        unreachable!("for frame resumes at a for statement")
    };
    resolved.push(lower(
        function,
        &format!("{block_path}.s{index}"),
        item,
        *item_span,
        &source,
        element,
        authored_body,
        *span,
    ));
    frames.push(Frame::BlockNext {
        span: block_span,
        path: block_path,
        statements,
        tail,
        index: index + 1,
        scope,
        resolved,
    });
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
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = &source_binding.ty
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
    resolver.reject_while_disallowed(body)?;
    let source = ResolvedBinding {
        id: source_binding.id.clone(),
        name: source_name.clone(),
        ownership: source_binding.ownership,
        ty: source_binding.ty.clone(),
        span: values.span,
    };
    let element = arguments[0].clone();
    let mut body_scope = scope.clone();
    body_scope.insert(
        item.to_owned(),
        Binding {
            id: ValueId::local(function, &format!("{statement_path}.value.s2.body.s0")),
            ty: element.clone(),
            ownership: OwnershipMode::Value,
            mutable: false,
        },
    );
    let authored_body = resolver.resolve_expr_recursive_reference(
        function,
        body,
        &body_scope,
        &format!("{statement_path}.value.s2.body.s1.value"),
    )?;
    Ok(lower(
        function,
        statement_path,
        item,
        item_span,
        &source,
        element,
        authored_body,
        span,
    ))
}

fn place(
    function: &FunctionExecutionId,
    path: &str,
    binding: &ResolvedBinding,
    span: Span,
) -> ResolvedExpr {
    ResolvedExpr {
        id: ExpressionId::new(function, path),
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
    let value_path = format!("{statement_path}.value");
    let length_path = format!("{value_path}.s0");
    let index_path = format!("{value_path}.s1");
    let while_path = format!("{value_path}.s2");
    let while_body_path = format!("{while_path}.body");

    let length = ResolvedBinding {
        id: ValueId::local(function, &length_path),
        name: "#for-length".to_owned(),
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
        ty: element.clone(),
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
            callee: DeclarationId::new(crate::vec_ops::LEN_ID),
            type_arguments: vec![element.clone()],
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
    let get_value_path = format!("{item_path}.value");
    let get_value = ResolvedExpr {
        id: ExpressionId::new(function, &get_value_path),
        ty: element.clone(),
        ownership: OwnershipMode::Value,
        kind: ResolvedExprKind::Call {
            callee: DeclarationId::new(crate::vec_ops::GET_ID),
            type_arguments: vec![element],
            instance: None,
            args: vec![
                place(function, &format!("{get_value_path}.arg.0"), source, span),
                place(function, &format!("{get_value_path}.arg.1"), &index, span),
            ],
        },
        span,
    };
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
                ResolvedStatement::While {
                    condition: Box::new(condition),
                    body: Box::new(while_body),
                    span,
                },
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
            name: "#for".to_owned(),
            ownership: OwnershipMode::Value,
            ty: ResolvedType::Usize,
            span,
        },
        mutable: false,
        value: lowered,
        span,
    }
}
