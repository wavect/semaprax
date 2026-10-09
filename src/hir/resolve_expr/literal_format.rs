//! Compiler-owned literal-format source admission for the iterative resolver.
use super::*;
use crate::ast::{Span, Type};

impl Resolver<'_> {
    pub(super) fn queue_literal_format_argument<'e>(
        frames: &mut Vec<Frame<'e>>,
        args: &'e [Expr],
        index: usize,
        bindings: Rc<BTreeMap<String, Binding>>,
        path: String,
    ) {
        if let Some(argument) = args.get(index) {
            frames.push(Frame::LiteralFormatArgNext {
                args,
                index: index + 1,
                bindings: Rc::clone(&bindings),
                path: path.clone(),
            });
            frames.push(Frame::Enter {
                expr: argument,
                bindings,
                path: format!("{path}.arg.{index}"),
            });
        }
    }

    pub(super) fn literal_format_frames<'e>(
        &self,
        function: &FunctionExecutionId,
        expression: &Expr,
        type_arguments: &[Type],
        args: &'e [Expr],
        bindings: Rc<BTreeMap<String, Binding>>,
        path: String,
    ) -> Result<[Frame<'e>; 2], Diagnostic> {
        let template = self.prepare_literal_format(function, expression, type_arguments, args)?;
        Ok([
            Frame::FinishLiteralFormat {
                span: expression.span,
                path: path.clone(),
                template,
                argument_count: args.len() - 1,
            },
            Frame::LiteralFormatArgNext {
                args,
                index: 1,
                bindings,
                path,
            },
        ])
    }

    pub(super) fn prepare_literal_format(
        &self,
        function: &FunctionExecutionId,
        expression: &Expr,
        type_arguments: &[Type],
        args: &[Expr],
    ) -> Result<String, Diagnostic> {
        if !type_arguments.is_empty()
            || function.instance().is_some()
            || function
                .monomorphic_declaration()
                .is_none_or(|id| self.declarations.declaration(id).is_none())
        {
            return Err(self.error(
                "SPX-H006",
                "string_format is admitted only in monomorphic ordinary function bodies",
                expression.span,
            ));
        }
        let Some(Expr {
            kind: ExprKind::String(template),
            ..
        }) = args.first()
        else {
            return Err(self.error(
                "SPX-H006",
                "string_format requires a compile-time string literal",
                expression.span,
            ));
        };
        let pieces = crate::literal_format::scan(template)
            .map_err(|reason| self.error("SPX-H006", reason.message(), expression.span))?;
        if args.len() - 1 != crate::literal_format::field_count(&pieces) {
            return Err(self.error(
                "SPX-H006",
                "string_format literal field count does not match arguments",
                expression.span,
            ));
        }
        if !crate::bounded_output::reserve_active_required(template.len()) {
            return Err(self.error(
                "SPX-H006",
                "string_format HIR template exceeds builder budget",
                expression.span,
            ));
        }
        Ok(template.clone())
    }

    pub(super) fn finish_literal_format(
        &self,
        function: &FunctionExecutionId,
        span: Span,
        path: &str,
        template: String,
        args: Vec<ResolvedExpr>,
    ) -> Result<ResolvedExpr, Diagnostic> {
        if args.iter().any(|arg| {
            !crate::literal_format::accepts_hir_type(&arg.ty)
                || (arg.ty == ResolvedType::String && arg.ownership != OwnershipMode::Own)
        }) {
            return Err(self.error(
                "SPX-H006",
                "string_format values must be i64, u8, usize, bool, or own string",
                span,
            ));
        }
        let ty = ResolvedType::String;
        let ownership = self.expression_ownership(&ty, OwnershipMode::Own, span)?;
        Ok(ResolvedExpr {
            id: ExpressionId::new(function, path),
            ty,
            ownership,
            kind: ResolvedExprKind::LiteralFormat { template, args },
            span,
        })
    }
}
