//! Resolve a checked literal selector; only vector and index become children.
use super::*;
impl Resolver<'_> {
    pub(in crate::hir) fn prepare_vec_field(
        &self,
        function: &FunctionExecutionId,
        expression: &Expr,
        type_arguments: &[crate::ast::Type],
        args: &[Expr],
    ) -> Result<(ResolvedType, DeclarationId), Diagnostic> {
        let fail = || {
            self.error("SPX-H006", "vec_field requires one admitted explicit record type, named Vec and index, and an exact literal field selector in a monomorphic function", expression.span)
        };
        if function.instance().is_some() {
            return Err(fail());
        }
        let [element] = type_arguments else {
            return Err(fail());
        };
        let [_, _, selector] = args else {
            return Err(fail());
        };
        let ExprKind::String(name) = &selector.kind else {
            return Err(fail());
        };
        let element = self.resolve_type(element, expression.span)?;
        let ResolvedType::Nominal { declaration, .. } = &element else {
            return Err(fail());
        };
        let field = self
            .declarations
            .field_id(declaration, name)
            .cloned()
            .ok_or_else(fail)?;
        super::super::vec_field::field(&self.declarations, &element, &field).ok_or_else(fail)?;
        Ok((element, field))
    }
    pub(in crate::hir) fn finish_vec_field(
        &self,
        function: &FunctionExecutionId,
        span: crate::ast::Span,
        path: &str,
        element: ResolvedType,
        field: DeclarationId,
        args: Vec<ResolvedExpr>,
    ) -> Result<ResolvedExpr, Diagnostic> {
        let fail = || {
            self.error(
                "SPX-H006",
                "vec_field requires an available named Vec carrier and usize index",
                span,
            )
        };
        let [source, index] = args.as_slice() else {
            return Err(fail());
        };
        if !matches!(&source.kind, ResolvedExprKind::Place(_))
            || !matches!(source.ownership, OwnershipMode::Own | OwnershipMode::Borrow)
            || source.ty != crate::vec_ops::resolved_vec(element.clone())
            || index.ty != ResolvedType::Usize
            || index.ownership != OwnershipMode::Value
        {
            return Err(fail());
        }
        let ty = super::super::vec_field::field(&self.declarations, &element, &field)
            .and_then(|field| field.result_type(false))
            .ok_or_else(fail)?;
        let ownership = if matches!(ty, ResolvedType::Str | ResolvedType::SliceU8) {
            OwnershipMode::Borrow
        } else {
            OwnershipMode::Value
        };
        Ok(ResolvedExpr {
            id: ExpressionId::new(function, path),
            ty,
            ownership,
            kind: ResolvedExprKind::VecFieldRead {
                element,
                field,
                bytes: false,
                args,
            },
            span,
        })
    }
}

impl Resolver<'_> {
    pub(in crate::hir) fn fuse_vec_field_bytes(
        &self,
        function: &FunctionExecutionId,
        span: crate::ast::Span,
        path: &str,
        operation: crate::byte_ops::ByteOp,
        args: &[ResolvedExpr],
    ) -> Option<ResolvedExpr> {
        if operation != crate::byte_ops::ByteOp::StrAsBytes {
            return None;
        }
        let [argument] = args else {
            return None;
        };
        let ResolvedExprKind::VecFieldRead {
            element,
            field,
            bytes: false,
            args,
        } = &argument.kind
        else {
            return None;
        };
        if argument.ty != ResolvedType::Str || argument.ownership != OwnershipMode::Borrow {
            return None;
        }
        Some(ResolvedExpr {
            id: ExpressionId::new(function, path),
            ty: ResolvedType::SliceU8,
            ownership: OwnershipMode::Borrow,
            kind: ResolvedExprKind::VecFieldRead {
                element: element.clone(),
                field: field.clone(),
                bytes: true,
                args: args.clone(),
            },
            span,
        })
    }
}

impl Resolver<'_> {
    pub(super) fn vec_field_frames<'e>(
        &self,
        function: &FunctionExecutionId,
        expression: &Expr,
        types: &[crate::ast::Type],
        args: &'e [Expr],
        bindings: Rc<BTreeMap<String, Binding>>,
        path: String,
    ) -> Result<[Frame<'e>; 2], Diagnostic> {
        let (element, field) = self.prepare_vec_field(function, expression, types, args)?;
        Ok([
            Frame::FinishVecField {
                span: expression.span,
                path: path.clone(),
                element,
                field,
            },
            Frame::ChildNext {
                children: &args[..2],
                index: 0,
                bindings,
                path,
                segment: "arg",
            },
        ])
    }
}
