use super::*;
use crate::ast::{Expr, ExprKind, Type};
use std::collections::BTreeMap;

fn source_type(ty: &ResolvedType, scope: &crate::ast::Function) -> Type {
    match ty {
        ResolvedType::I64 => Type::I64,
        ResolvedType::I32 => Type::I32,
        ResolvedType::U8 => Type::U8,
        ResolvedType::Usize => Type::Usize,
        ResolvedType::Char => Type::Char,
        ResolvedType::F32 => Type::F32,
        ResolvedType::F64 => Type::F64,
        ResolvedType::Bool => Type::Bool,
        ResolvedType::TypeParameter { owner, index } if owner.as_str() == scope.stable_id => scope
            .type_parameters
            .get(*index as usize)
            .map(|p| Type::Named {
                name: p.name.clone(),
                arguments: Vec::new(),
            })
            .unwrap_or(Type::String),
        _ => Type::String,
    }
}

impl Resolver<'_> {
    pub(in crate::hir) fn resolve_closure(
        &self,
        parent: &FunctionExecutionId,
        expression: &Expr,
        outer: &BTreeMap<String, Binding>,
        path: &str,
        reference: bool,
    ) -> Result<ResolvedExpr, Diagnostic> {
        let ExprKind::Closure {
            params,
            return_type,
            body,
            owning,
        } = &expression.kind
        else {
            unreachable!()
        };
        if *owning {
            // SPX-AI-021 bounded owning-capture profile: admitted and fully
            // checked at the source level (see `source_verify::closure`),
            // but not yet lowered to HIR/backends pending the independent
            // review the profile's design calls for. Agreement-by-refusal:
            // no backend observes a partially-lowered owning capture.
            // See `docs/CLOSURES-OWNING-V1.md`.
            return Err(hir_error(
                "owning-capture closures are admitted and checked at the source level but are not yet lowered to HIR in this bounded profile; see docs/CLOSURES-OWNING-V1.md",
            ));
        }
        let source_function = parent
            .monomorphic_declaration()
            .and_then(|id| {
                self.program
                    .functions
                    .iter()
                    .find(|f| f.stable_id == id.as_str())
            })
            .ok_or_else(|| hir_error("nested closures are not admitted"))?;
        if !source_function.type_parameters.is_empty()
            && !crate::source_verify::generic_collection_profile(source_function)
        {
            return Err(hir_error(
                "generic closures require the bounded collection profile",
            ));
        }
        let types = outer
            .iter()
            .map(|(name, binding)| (name.as_str(), source_type(&binding.ty, source_function)))
            .collect::<BTreeMap<_, _>>();
        let type_refs = types.iter().map(|(name, ty)| (*name, ty)).collect();
        let mut names = crate::source_verify::closure::capture_names_scoped(
            self.program,
            params,
            return_type,
            body,
            &type_refs,
            Some(source_function),
        )?;
        names.sort_by(|a, b| outer[a].id.cmp(&outer[b].id));
        let id = ExpressionId::new(parent, path);
        let target = closure_id(&id);
        if self.declarations.declaration(&target).is_some() {
            return Err(hir_error(
                "closure identity collides with a declared identity",
            ));
        }
        let execution = FunctionExecutionId::Monomorphic(target);
        let mut bindings = BTreeMap::new();
        let mut captures = Vec::new();
        for (index, name) in names.into_iter().enumerate() {
            let captured = &outer[&name];
            if captured.ownership != OwnershipMode::Value
                || !(super::super::function_value::scalar(&captured.ty)
                    || super::super::generic_collection::parameter(
                        &captured.ty,
                        &DeclarationId::new(source_function.stable_id.clone()),
                        source_function.type_parameters.len(),
                    ))
            {
                return Err(hir_error(
                    "closure capture is not an unborrowed Copy scalar",
                ));
            }
            let binding = ResolvedBinding {
                id: ValueId::parameter(&execution, index),
                name: name.clone(),
                ownership: OwnershipMode::Value,
                ty: captured.ty.clone(),
                span: expression.span,
            };
            bindings.insert(
                name,
                Binding {
                    id: binding.id.clone(),
                    ty: binding.ty.clone(),
                    ownership: OwnershipMode::Value,
                    mutable: false,
                },
            );
            captures.push(ResolvedClosureCapture {
                binding,
                value: ResolvedExpr {
                    id: ExpressionId::new(parent, &format!("{path}.capture.{index}")),
                    ty: captured.ty.clone(),
                    ownership: OwnershipMode::Value,
                    kind: ResolvedExprKind::Place(Place {
                        root: captured.id.clone(),
                        projections: Vec::new(),
                    }),
                    span: expression.span,
                },
            });
        }
        let mut parameters = Vec::new();
        for (index, param) in params.iter().enumerate() {
            let ty = self.resolve_function_type(source_function, &param.ty, param.span)?;
            let binding = ResolvedBinding {
                id: ValueId::parameter(&execution, captures.len() + index),
                name: param.name.clone(),
                ownership: OwnershipMode::Value,
                ty,
                span: param.span,
            };
            bindings.insert(
                param.name.clone(),
                Binding {
                    id: binding.id.clone(),
                    ty: binding.ty.clone(),
                    ownership: OwnershipMode::Value,
                    mutable: false,
                },
            );
            parameters.push(binding);
        }
        let ty = ResolvedType::Function {
            parameters: parameters.iter().map(|p| p.ty.clone()).collect(),
            result: Box::new(self.resolve_function_type(
                source_function,
                return_type,
                expression.span,
            )?),
        };
        #[cfg(test)]
        let resolved_body = if reference {
            self.resolve_expr_recursive_reference(&execution, body, &bindings, "body")?
        } else {
            self.resolve_expr(&execution, body, &bindings, "body")?
        };
        #[cfg(not(test))]
        let resolved_body = {
            let _ = reference;
            self.resolve_expr(&execution, body, &bindings, "body")?
        };
        Ok(ResolvedExpr {
            id,
            ty,
            ownership: OwnershipMode::Value,
            kind: ResolvedExprKind::Closure {
                parameters,
                captures,
                body: Box::new(resolved_body),
            },
            span: expression.span,
        })
    }
}

impl Resolver<'_> {
    // Private closure execution IDs do not name source declarations. Their
    // scalar parameter/capture types retain the checked lexical generic owner.
    pub(in crate::hir) fn resolve_binding_annotation(
        &self,
        execution: &FunctionExecutionId,
        bindings: &BTreeMap<String, Binding>,
        ty: &Type,
        span: crate::ast::Span,
    ) -> Result<ResolvedType, Diagnostic> {
        if let Type::Named { name, arguments } = ty {
            if arguments.is_empty() {
                for binding in bindings.values() {
                    let ResolvedType::TypeParameter { owner, .. } = &binding.ty else {
                        continue;
                    };
                    if let Some(function) = self.program.functions.iter().find(|function| {
                        function.stable_id == owner.as_str()
                            && function
                                .type_parameters
                                .iter()
                                .any(|parameter| parameter.name == *name)
                    }) {
                        return self.resolve_function_type(function, ty, span);
                    }
                }
            }
        }
        self.resolve_expression_type(execution, ty, span)
    }
}
