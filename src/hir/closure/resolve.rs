use super::*;
use crate::ast::{Expr, ExprKind, Type};
use std::collections::BTreeMap;

fn source_type(ty: &ResolvedType) -> Type {
    match ty {
        ResolvedType::I64 => Type::I64,
        ResolvedType::I32 => Type::I32,
        ResolvedType::U8 => Type::U8,
        ResolvedType::Usize => Type::Usize,
        ResolvedType::Char => Type::Char,
        ResolvedType::F32 => Type::F32,
        ResolvedType::F64 => Type::F64,
        ResolvedType::Bool => Type::Bool,
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
        } = &expression.kind
        else {
            unreachable!()
        };
        if parent.monomorphic_declaration().is_none_or(|id| {
            self.program
                .functions
                .iter()
                .find(|f| f.stable_id == id.as_str())
                .is_none_or(|f| !f.type_parameters.is_empty())
        }) {
            return Err(hir_error(
                "closures require an ordinary source function scope",
            ));
        }
        let types = outer
            .iter()
            .map(|(name, binding)| (name.as_str(), source_type(&binding.ty)))
            .collect::<BTreeMap<_, _>>();
        let type_refs = types.iter().map(|(name, ty)| (*name, ty)).collect();
        let mut names = crate::source_verify::closure::capture_names(
            self.program,
            params,
            return_type,
            body,
            &type_refs,
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
                || !super::super::function_value::scalar(&captured.ty)
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
            let ty = self.resolve_type(&param.ty, param.span)?;
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
            result: Box::new(self.resolve_type(return_type, expression.span)?),
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
