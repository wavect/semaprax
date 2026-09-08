//! Independent resolution of bounded inference from exact declaration types.
//! No source-verifier inference result is trusted or retained here.
use super::*;
use crate::ast::{Expr, ExprKind};

impl Resolver<'_> {
    pub(super) fn infer_call_arguments(
        &self,
        caller: &FunctionExecutionId,
        target: &crate::ast::Function,
        args: &[Expr],
        bindings: &BTreeMap<String, Binding>,
    ) -> Option<Vec<Type>> {
        let FunctionExecutionId::Monomorphic(caller) = caller else {
            return None;
        };
        if self
            .declarations
            .type_parameters(caller)
            .is_some_and(|parameters| !parameters.is_empty())
            || target.type_parameters.len() != 1
            || args.len() != target.params.len()
        {
            return None;
        }
        let owner = DeclarationId::new(target.stable_id.clone());
        let mut inferred = None;
        for (formal, argument) in target.params.iter().zip(args) {
            let formal = self
                .resolve_function_type(target, &formal.ty, formal.span)
                .ok()?;
            let actual = match &argument.kind {
                ExprKind::Int(_) => ResolvedType::I64,
                ExprKind::Int32(_) => ResolvedType::I32,
                ExprKind::Uint8(_) => ResolvedType::U8,
                ExprKind::Usize(_) => ResolvedType::Usize,
                ExprKind::Char(_) => ResolvedType::Char,
                ExprKind::Float32(_) => ResolvedType::F32,
                ExprKind::Float64(_) => ResolvedType::F64,
                ExprKind::Bool(_) => ResolvedType::Bool,
                ExprKind::Var(name) => bindings.get(name)?.ty.clone(),
                ExprKind::ConstructRecord {
                    type_name,
                    type_arguments,
                    ..
                }
                | ExprKind::ConstructVariant {
                    type_name,
                    type_arguments,
                    ..
                } => self
                    .resolve_type(
                        &Type::Named {
                            name: type_name.clone(),
                            arguments: type_arguments.clone(),
                        },
                        argument.span,
                    )
                    .ok()?,
                _ => return None,
            };
            let mut pending = vec![(&formal, &actual)];
            while let Some((formal, actual)) = pending.pop() {
                if matches!(formal, ResolvedType::TypeParameter { owner: parameter_owner, index: 0 } if *parameter_owner == owner)
                {
                    let ty = match actual {
                        ResolvedType::I64 => Type::I64,
                        ResolvedType::I32 => Type::I32,
                        ResolvedType::U8 => Type::U8,
                        ResolvedType::Usize => Type::Usize,
                        ResolvedType::Char => Type::Char,
                        ResolvedType::F32 => Type::F32,
                        ResolvedType::F64 => Type::F64,
                        ResolvedType::Bool => Type::Bool,
                        _ => return None,
                    };
                    if inferred.as_ref().is_some_and(|old| old != &ty) {
                        return None;
                    }
                    inferred = Some(ty);
                } else if let (
                    ResolvedType::Nominal {
                        declaration: a,
                        arguments: aa,
                    },
                    ResolvedType::Nominal {
                        declaration: b,
                        arguments: ba,
                    },
                ) = (formal, actual)
                {
                    if a != b || aa.len() != ba.len() {
                        return None;
                    }
                    pending.extend(aa.iter().zip(ba));
                } else if formal != actual {
                    return None;
                }
            }
        }
        inferred.map(|ty| vec![ty])
    }
}

impl Resolver<'_> {
    /// Prepare one ordinary call without evaluating or resolving its arguments.
    /// Both resolver engines use this boundary and then visit arguments once.
    pub(super) fn ordinary_call_signature(
        &self,
        caller: &FunctionExecutionId,
        target: &crate::ast::Function,
        authored_arguments: &[Type],
        args: &[Expr],
        bindings: &BTreeMap<String, Binding>,
        span: Span,
    ) -> Result<(Vec<ResolvedType>, Option<FunctionInstanceId>, Type), Diagnostic> {
        let inferred = if authored_arguments.is_empty() && !target.type_parameters.is_empty() {
            self.infer_call_arguments(caller, target, args, bindings)
        } else {
            None
        };
        let arguments = inferred.as_deref().unwrap_or(authored_arguments);
        let resolved = arguments
            .iter()
            .map(|argument| self.resolve_call_type_argument(caller, argument, span))
            .collect::<Result<Vec<_>, _>>()?;
        let template = DeclarationId::new(target.stable_id.clone());
        if target.type_parameters.is_empty() {
            if !resolved.is_empty() {
                return Err(self.error(
                    "SPX-H006",
                    format!("monomorphic function `{template}` has type arguments"),
                    span,
                ));
            }
            return Ok((resolved, None, target.return_type.clone()));
        }
        if resolved.len() != target.type_parameters.len()
            || !self.generic_function_arguments_are_admitted(caller, target, &resolved)?
        {
            return Err(self.error(
                "SPX-H006",
                format!("generic function `{template}` has invalid type arguments"),
                span,
            ));
        }
        let instance = FunctionInstanceId::derive(&template, &resolved);
        let return_type = super::monomorphize::substitute_source_function_type(
            target,
            arguments,
            &target.return_type,
        )
        .ok_or_else(|| {
            self.error(
                "SPX-H006",
                format!("generic function `{template}` return substitution failed"),
                span,
            )
        })?;
        Ok((resolved, Some(instance), return_type))
    }
}
