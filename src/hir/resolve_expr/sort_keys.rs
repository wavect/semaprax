//! The literal descriptor is checked metadata, never a String expression.
use super::path_arena::{ExprPath, PathArena};
use super::*;
impl Resolver<'_, '_> {
    pub(super) fn finish_vec_literal_frame(
        &self,
        function: &FunctionExecutionId,
        frame: Frame<'_>,
        paths: &PathArena<'_>,
        results: &mut Vec<ResolvedExpr>,
    ) -> Result<(), Diagnostic> {
        match frame {
            Frame::FinishSortBy {
                span,
                path,
                element,
                callee,
            } => {
                let args = take_results(
                    results,
                    if callee
                        .as_str()
                        .starts_with(crate::vec_ops::field_update::PREFIX)
                    {
                        3
                    } else {
                        1
                    },
                );
                let path = paths.at(path);
                results.push(self.finish_sort_by(function, span, path, element, callee, args)?);
            }
            Frame::FinishVecField {
                span,
                path,
                element,
                field,
            } => {
                let args = take_results(results, 2);
                let path = paths.at(path);
                results.push(self.finish_vec_field(function, span, path, element, field, args)?);
            }
            _ => unreachable!("literal vector frame"),
        }
        Ok(())
    }
    pub(in crate::hir) fn prepare_sort_by(
        &self,
        function: &FunctionExecutionId,
        expression: &Expr,
        types: &[crate::ast::Type],
        args: &[Expr],
    ) -> Result<(ResolvedType, DeclarationId), Diagnostic> {
        // Field uniqueness shares this literal-metadata route but keeps its own
        // distinct versioned descriptor, borrowed operand and Bool result.
        let unique = matches!(&expression.kind, ExprKind::Call { name, .. }
            if name == crate::vec_ops::field_unique::NAME);
        let update = matches!(&expression.kind, ExprKind::Call { name, .. } if name == crate::vec_ops::field_update::NAME);
        let fail = || {
            if update {
                return self.error(
                    "SPX-H006",
                    crate::vec_ops::field_update::SHAPE,
                    expression.span,
                );
            }
            if unique {
                self.error(
                    "SPX-H006",
                    crate::vec_ops::field_unique::SHAPE,
                    expression.span,
                )
            } else {
                self.error("SPX-H006","vec_sort_by requires a monomorphic explicit record and one finite literal key list",expression.span)
            }
        };
        if function.instance().is_some() {
            return Err(fail());
        }
        let [ty] = types else {
            return Err(fail());
        };
        let literal = if update {
            let [_, _, _, literal] = args else {
                return Err(fail());
            };
            literal
        } else {
            let [_, literal] = args else {
                return Err(fail());
            };
            literal
        };
        let ExprKind::String(keys) = &literal.kind else {
            return Err(fail());
        };
        let element = self.resolve_type(ty, expression.span)?;
        let descriptor = if update {
            crate::vec_ops::field_update::resolve(&self.declarations, &element, keys)
                .and_then(|d| d.identity())
        } else if unique {
            crate::vec_ops::field_unique::resolve(&self.declarations, &element, keys)
                .and_then(|d| d.identity())
        } else {
            crate::vec_ops::sort_keys::resolve(&self.declarations, &element, keys)
                .and_then(|d| d.identity())
        }
        .ok_or_else(fail)?;
        Ok((element, DeclarationId::new(descriptor)))
    }
    pub(in crate::hir) fn finish_sort_by<'p>(
        &self,
        function: &FunctionExecutionId,
        span: crate::ast::Span,
        path: impl Into<ExprPath<'p>>,
        element: ResolvedType,
        callee: DeclarationId,
        args: Vec<ResolvedExpr>,
    ) -> Result<ResolvedExpr, Diagnostic> {
        let update = callee
            .as_str()
            .starts_with(crate::vec_ops::field_update::PREFIX);
        if update {
            let (_, field_type) = crate::vec_ops::field_update::position(
                &self.declarations,
                &element,
                callee.as_str(),
            )
            .ok_or_else(|| self.error("SPX-H006", crate::vec_ops::field_update::SHAPE, span))?;
            if !crate::vec_ops::field_update::arguments(&args, &element, &field_type) {
                return Err(self.error("SPX-H006", crate::vec_ops::field_update::SHAPE, span));
            }
        }
        let op = if update {
            crate::vec_ops::VecOp::UpdateField
        } else if callee
            .as_str()
            .starts_with(crate::vec_ops::field_unique::PREFIX)
        {
            crate::vec_ops::VecOp::UniqueBy
        } else {
            crate::vec_ops::VecOp::SortBy
        };
        let mut expression =
            super::super::resolve_vec_call::finish(self, function, path, span, op, element, args)?;
        let ResolvedExprKind::Call { callee: target, .. } = &mut expression.kind else {
            unreachable!("vector call")
        };
        *target = callee;
        Ok(expression)
    }
    pub(super) fn sort_by_frames<'e>(
        &self,
        function: &FunctionExecutionId,
        expression: &Expr,
        types: &[crate::ast::Type],
        args: &'e [Expr],
        bindings: Scope<'e>,
        path: PathId,
    ) -> Result<[Frame<'e>; 2], Diagnostic> {
        let (element, callee) = self.prepare_sort_by(function, expression, types, args)?;
        let runtime_count = if callee
            .as_str()
            .starts_with(crate::vec_ops::field_update::PREFIX)
        {
            3
        } else {
            1
        };
        Ok([
            Frame::FinishSortBy {
                span: expression.span,
                path,
                element,
                callee,
            },
            Frame::ChildNext {
                children: &args[..runtime_count],
                index: 0,
                bindings,
                path,
                segment: Seg::Arg,
            },
        ])
    }
}
