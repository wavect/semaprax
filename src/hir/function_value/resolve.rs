use super::super::resolve_expr::path_arena::Seg;
use super::super::{Binding, ExprPath, FunctionExecutionId, Place, Resolver};
use super::*;
use crate::ast::{Expr, ExprKind, Type};
use std::collections::BTreeMap;

pub(crate) fn source_scalar(ty: &Type) -> Option<ResolvedType> {
    Some(match ty {
        Type::I64 => ResolvedType::I64,
        Type::I32 => ResolvedType::I32,
        Type::U8 => ResolvedType::U8,
        Type::Usize => ResolvedType::Usize,
        Type::Char => ResolvedType::Char,
        Type::F32 => ResolvedType::F32,
        Type::F64 => ResolvedType::F64,
        Type::Bool => ResolvedType::Bool,
        _ => return None,
    })
}
pub(crate) fn source_type(ty: &Type) -> Option<ResolvedType> {
    if *ty == Type::MutFunctionI64 {
        return Some(ResolvedType::MutFunctionI64);
    }
    if ty.is_once_function() {
        return Some(if *ty == Type::OnceFunctionI64Pair {
            ResolvedType::OnceFunctionI64Pair
        } else if *ty == Type::OnceFunctionI64 {
            ResolvedType::OnceFunctionI64
        } else {
            ResolvedType::OnceFunction
        });
    }
    let Type::Function { parameters, result } = ty else {
        return source_scalar(ty);
    };
    if parameters.len() > 8 {
        return None;
    }
    Some(ResolvedType::Function {
        parameters: parameters
            .iter()
            .map(source_scalar)
            .collect::<Option<_>>()?,
        result: Box::new(source_scalar(result)?),
    })
}
impl Resolver<'_, '_> {
    pub(in crate::hir) fn finish_invocation<'p>(
        &self,
        function: &FunctionExecutionId,
        path: impl Into<ExprPath<'p>>,
        span: crate::ast::Span,
        callable: ResolvedExpr,
        args: Vec<ResolvedExpr>,
    ) -> Result<ResolvedExpr, Diagnostic> {
        finish(self.program, function, path, span, callable, args)
    }
    pub(in crate::hir) fn function_reference<'p>(
        &self,
        function: &FunctionExecutionId,
        expr: &Expr,
        bindings: &BTreeMap<String, Binding>,
        path: impl Into<ExprPath<'p>>,
    ) -> Result<Option<ResolvedExpr>, Diagnostic> {
        let ExprKind::Var(name) = &expr.kind else {
            return Ok(None);
        };
        if bindings.contains_key(name) {
            return Ok(None);
        }
        let Some(target) = self.source_functions().find(|f| f.name() == name) else {
            return Ok(None);
        };
        if !target.payload().type_parameters.is_empty()
            || !target.payload().effects.is_empty()
            || target.payload().params.len() > 8
            || target
                .payload()
                .params
                .iter()
                .any(|p| p.mode != crate::ast::ParamMode::Value)
        {
            return Err(error("ineligible function reference"));
        }
        let ty = ResolvedType::Function {
            parameters: target
                .payload()
                .params
                .iter()
                .map(|p| source_scalar(&p.ty))
                .collect::<Option<_>>()
                .ok_or_else(|| error("function reference parameter is not scalar"))?,
            result: Box::new(
                source_scalar(&target.payload().return_type)
                    .ok_or_else(|| error("function reference result is not scalar"))?,
            ),
        };
        Ok(Some(ResolvedExpr {
            id: path.into().expression_id(function),
            ty,
            ownership: OwnershipMode::Value,
            kind: ResolvedExprKind::FunctionReference {
                target: self.declarations.held_identity(&target.payload().stable_id),
            },
            span: expr.span,
        }))
    }
    pub(in crate::hir) fn invocation_target<'p>(
        &self,
        function: &FunctionExecutionId,
        name: &str,
        bindings: &BTreeMap<String, Binding>,
        path: impl Into<ExprPath<'p>>,
        span: crate::ast::Span,
    ) -> Result<Option<ResolvedExpr>, Diagnostic> {
        let Some(binding) = bindings.get(name) else {
            return Ok(None);
        };
        if binding.ty.is_mut_function() && !binding.mutable {
            return Err(error(
                "mutable invocation requires a mutable local receiver",
            ));
        }
        if !binding.ty.is_once_function()
            && !binding.ty.is_mut_function()
            && !is_signature(&binding.ty)
            && !function.monomorphic_declaration().is_some_and(|owner| {
                super::super::generic_collection::callback(
                    &binding.ty,
                    owner,
                    super::super::generic_collection::source_count(self.program, owner),
                )
            })
        {
            return Err(error("called binding is not an admitted function value"));
        }
        Ok(Some(ResolvedExpr {
            id: path
                .into()
                .expression_id_with(function, &[Seg::Callable.into()]),
            ty: binding.ty.clone(),
            ownership: binding.ownership,
            kind: ResolvedExprKind::Place(Place {
                root: binding.id.clone(),
                projections: Vec::new(),
            }),
            span,
        }))
    }
}
pub(in crate::hir) fn finish<'p>(
    program: &dyn crate::ast::program_source::ProgramSource,
    function: &FunctionExecutionId,
    path: impl Into<ExprPath<'p>>,
    span: crate::ast::Span,
    callable: ResolvedExpr,
    args: Vec<ResolvedExpr>,
) -> Result<ResolvedExpr, Diagnostic> {
    let result = match &callable.ty {
        ResolvedType::Function { result, .. } => result.as_ref(),
        ResolvedType::OnceFunction
        | ResolvedType::OnceFunctionI64
        | ResolvedType::OnceFunctionI64Pair
        | ResolvedType::MutFunctionI64 => &ResolvedType::I64,
        _ => return Err(error("invalid callable")),
    };
    let expr = ResolvedExpr {
        id: path.into().expression_id(function),
        ty: result.clone(),
        ownership: OwnershipMode::Value,
        kind: ResolvedExprKind::Invoke {
            callable: Box::new(callable),
            args,
        },
        span,
    };
    super::validate_invocation_scoped(
        &expr,
        function.monomorphic_declaration().map(|owner| {
            (
                owner,
                super::super::generic_collection::source_count(program, owner),
            )
        }),
    )?;
    Ok(expr)
}

impl Resolver<'_, '_> {
    pub(in crate::hir) fn resolve_generic_callable_type(
        &self,
        function: &crate::ast::program_source::FunctionRef<'_>,
        parameters: &[Type],
        result: &Type,
        span: crate::ast::Span,
    ) -> Result<ResolvedType, Diagnostic> {
        let ty = ResolvedType::Function {
            parameters: parameters
                .iter()
                .map(|ty| self.resolve_function_type_ref(function, ty, span))
                .collect::<Result<_, _>>()?,
            result: Box::new(self.resolve_function_type_ref(function, result, span)?),
        };
        if !is_signature(&ty)
            && !((1..=2).contains(&function.payload().type_parameters.len())
                && super::super::generic_collection::callback(
                    &ty,
                    &self
                        .declarations
                        .held_identity(&function.payload().stable_id),
                    function.payload().type_parameters.len(),
                ))
        {
            return Err(error("invalid scoped callable signature"));
        }
        Ok(ty)
    }
}
