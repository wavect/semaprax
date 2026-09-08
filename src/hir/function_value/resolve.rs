use super::super::{Binding, ExpressionId, FunctionExecutionId, Place, Resolver};
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
impl Resolver<'_> {
    pub(in crate::hir) fn function_reference(
        &self,
        function: &FunctionExecutionId,
        expr: &Expr,
        bindings: &BTreeMap<String, Binding>,
        path: &str,
    ) -> Result<Option<ResolvedExpr>, Diagnostic> {
        let ExprKind::Var(name) = &expr.kind else {
            return Ok(None);
        };
        if bindings.contains_key(name) {
            return Ok(None);
        }
        let Some(target) = self.program.functions.iter().find(|f| f.name == *name) else {
            return Ok(None);
        };
        if !target.type_parameters.is_empty()
            || !target.effects.is_empty()
            || target.params.len() > 8
            || target
                .params
                .iter()
                .any(|p| p.mode != crate::ast::ParamMode::Value)
        {
            return Err(error("ineligible function reference"));
        }
        let ty = ResolvedType::Function {
            parameters: target
                .params
                .iter()
                .map(|p| source_scalar(&p.ty))
                .collect::<Option<_>>()
                .ok_or_else(|| error("function reference parameter is not scalar"))?,
            result: Box::new(
                source_scalar(&target.return_type)
                    .ok_or_else(|| error("function reference result is not scalar"))?,
            ),
        };
        Ok(Some(ResolvedExpr {
            id: ExpressionId::new(function, path),
            ty,
            ownership: OwnershipMode::Value,
            kind: ResolvedExprKind::FunctionReference {
                target: DeclarationId::new(&target.stable_id),
            },
            span: expr.span,
        }))
    }
    pub(in crate::hir) fn invocation_target(
        &self,
        function: &FunctionExecutionId,
        name: &str,
        bindings: &BTreeMap<String, Binding>,
        path: &str,
        span: crate::ast::Span,
    ) -> Result<Option<ResolvedExpr>, Diagnostic> {
        let Some(binding) = bindings.get(name) else {
            return Ok(None);
        };
        if !is_signature(&binding.ty)
            && !function
                .monomorphic_declaration()
                .is_some_and(|owner| super::super::generic_collection::callback(&binding.ty, owner))
        {
            return Err(error("called binding is not an admitted function value"));
        }
        Ok(Some(ResolvedExpr {
            id: ExpressionId::new(function, &format!("{path}.callable")),
            ty: binding.ty.clone(),
            ownership: OwnershipMode::Value,
            kind: ResolvedExprKind::Place(Place {
                root: binding.id.clone(),
                projections: Vec::new(),
            }),
            span,
        }))
    }
}
pub(in crate::hir) fn finish(
    function: &FunctionExecutionId,
    path: &str,
    span: crate::ast::Span,
    callable: ResolvedExpr,
    args: Vec<ResolvedExpr>,
) -> Result<ResolvedExpr, Diagnostic> {
    let ResolvedType::Function { result, .. } = &callable.ty else {
        return Err(error("invalid callable"));
    };
    let expr = ResolvedExpr {
        id: ExpressionId::new(function, path),
        ty: *result.clone(),
        ownership: OwnershipMode::Value,
        kind: ResolvedExprKind::Invoke {
            callable: Box::new(callable),
            args,
        },
        span,
    };
    super::validate_invocation_scoped(&expr, function.monomorphic_declaration())?;
    Ok(expr)
}

impl Resolver<'_> {
    pub(in crate::hir) fn resolve_generic_callable_type(
        &self,
        function: &crate::ast::Function,
        parameters: &[Type],
        result: &Type,
        span: crate::ast::Span,
    ) -> Result<ResolvedType, Diagnostic> {
        let ty = ResolvedType::Function {
            parameters: parameters
                .iter()
                .map(|ty| self.resolve_function_type(function, ty, span))
                .collect::<Result<_, _>>()?,
            result: Box::new(self.resolve_function_type(function, result, span)?),
        };
        if !is_signature(&ty)
            && !(function.type_parameters.len() == 1
                && super::super::generic_collection::callback(
                    &ty,
                    &DeclarationId::new(&function.stable_id),
                ))
        {
            return Err(error("invalid scoped callable signature"));
        }
        Ok(ty)
    }
}
