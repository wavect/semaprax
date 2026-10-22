//! `super.method(...)` continuation: canonical receiver upcast and call.
//!
//! Relocated from the iterative resolver's `FinishSuperMethod` arm; both
//! synthesized receiver identities stream from the call's path coordinate.
use super::path_arena::ExprPath;
use super::*;
use crate::ast::Span;

impl Resolver<'_, '_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn finish_super_method(
        &self,
        function: &FunctionExecutionId,
        span: Span,
        method_span: Span,
        path: ExprPath<'_>,
        method: &str,
        holder: DeclarationId,
        callee: DeclarationId,
        mut all: Vec<ResolvedExpr>,
    ) -> Result<ResolvedExpr, Diagnostic> {
        let receiver = all.remove(0);
        let args = all;
        // The inherited receiver is the enclosing override's own `self`, upcast to the declaring ancestor exactly like a declared-type binding.
        // The source place is synthesized here, so both identities are canonical by construction.
        let ResolvedType::Nominal {
            declaration: self_class,
            ..
        } = &receiver.ty
        else {
            return Err(self.error(
                "SPX-H006",
                "super receiver is not a class value",
                method_span,
            ));
        };
        let receiver = if *self_class == holder {
            receiver
        } else {
            self.check_upcast_admissible(self_class, &holder, method_span)?;
            let source = ResolvedExpr {
                id: path.expression_id_with(function, &[Seg::ArgZeroSource.into()]),
                ty: receiver.ty.clone(),
                ownership: receiver.ownership,
                kind: receiver.kind,
                span: receiver.span,
            };
            ResolvedExpr {
                id: path.expression_id_with(function, &[Seg::ArgZero.into()]),
                ty: ResolvedType::Nominal {
                    declaration: holder.clone(),
                    arguments: Vec::new(),
                },
                ownership: self.expression_ownership(
                    &ResolvedType::Nominal {
                        declaration: holder.clone(),
                        arguments: Vec::new(),
                    },
                    OwnershipMode::Own,
                    span,
                )?,
                kind: ResolvedExprKind::Upcast {
                    source: Box::new(source),
                },
                span: receiver.span,
            }
        };
        let holder_ast = self
            .program
            .types()
            .iter()
            .find(|t| t.stable_id == holder.as_str())
            .ok_or_else(|| self.error("SPX-H006", format!("class `{holder}` has no AST"), span))?;
        let TypeDeclarationKind::Class { methods, .. } = &holder_ast.kind else {
            return Err(self.error("SPX-H006", format!("`{holder}` is not a class"), span));
        };
        let method_ast = methods.iter().find(|m| m.name == *method).ok_or_else(|| {
            self.error(
                "SPX-H001",
                format!("unresolved super method `{method}` on `{holder}`"),
                method_span,
            )
        })?;
        if method_ast.params.is_empty() {
            return Err(self.error(
                "SPX-H001",
                format!("super method `{method}` has no self parameter"),
                method_ast.span,
            ));
        }
        if method_ast.params.len() - 1 != args.len() {
            return Err(self.error(
                "SPX-T231",
                format!(
                    "`super.{method}` expects {} arguments, found {}",
                    method_ast.params.len() - 1,
                    args.len()
                ),
                span,
            ));
        }
        for (arg, param) in args.iter().zip(method_ast.params.iter().skip(1)) {
            let param_ty = self.resolve_type(&param.ty, param.span)?;
            if arg.ty != param_ty {
                return Err(self.error(
                    "SPX-T231",
                    format!(
                        "`super.{method}` argument `{}` expects type `{}`, found `{}`",
                        param.name,
                        param_ty.identity_key(),
                        arg.ty.identity_key()
                    ),
                    arg.span,
                ));
            }
        }
        let return_ty = self.resolve_type(&method_ast.return_type, method_ast.span)?;
        let ownership = self.expression_ownership(&return_ty, OwnershipMode::Own, span)?;
        let mut call_args = Vec::with_capacity(1 + args.len());
        call_args.push(receiver);
        call_args.extend(args);
        Ok(ResolvedExpr {
            id: path.expression_id(function),
            ty: return_ty,
            ownership,
            kind: ResolvedExprKind::Call {
                callee,
                type_arguments: Vec::new(),
                instance: None,
                args: call_args,
            },
            span,
        })
    }
}
