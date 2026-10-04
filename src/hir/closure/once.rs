//! Closed affine callable profile: one owned Bytes capture, no arguments, i64.
use super::*;
use crate::ast::{Expr, ExprKind, ParamMode, Type};
use std::collections::BTreeMap;

pub(crate) const DROP_ID: &str = "core.fn_once.drop";
pub(crate) const CONSTRUCT_ID: &str = "core.fn_once.construct";
pub(crate) const INVOKE_ID: &str = "core.fn_once.invoke";
pub(crate) const MIXED_DROP_ID: &str = "core.fn_once_i64.drop.v2";
pub(crate) const MIXED_CONSTRUCT_ID: &str = "core.fn_once_i64.construct.v2";
pub(crate) const MIXED_INVOKE_ID: &str = "core.fn_once_i64.invoke.v2";

impl Resolver<'_> {
    pub(in crate::hir) fn resolve_once_closure(
        &self,
        parent: &FunctionExecutionId,
        expression: &Expr,
        outer: &BTreeMap<String, Binding>,
        path: &str,
        reference: bool,
    ) -> Result<ResolvedExpr, Diagnostic> {
        let ExprKind::Closure {
            params,
            return_type: Type::I64,
            body,
            owning: true,
            retained: true,
        } = &expression.kind
        else {
            return Err(hir_error("invalid affine closure literal"));
        };
        if !params.is_empty() || parent.monomorphic_declaration().is_none() {
            return Err(hir_error(
                "affine closures require a monomorphic, zero-argument creation site",
            ));
        }
        let call = match &body.kind {
            ExprKind::Block { statements, tail } if statements.is_empty() => tail.as_ref(),
            _ => return Err(hir_error("affine closure requires one tail call")),
        };
        let ExprKind::Call {
            name,
            type_arguments,
            args,
        } = &call.kind
        else {
            return Err(hir_error("affine closure body is not a direct call"));
        };
        let mixed = args.len() == 2;
        let Some(Expr {
            kind: ExprKind::Var(captured),
            ..
        }) = args.first()
        else {
            return Err(hir_error("affine closure must transfer one captured owner"));
        };
        let target = self
            .program
            .functions
            .iter()
            .find(|f| f.name == *name)
            .ok_or_else(|| hir_error("affine closure target is absent"))?;
        if !type_arguments.is_empty()
            || !target.type_parameters.is_empty()
            || !target.effects.is_empty()
            || target.return_type != Type::I64
            || target.params.len() != args.len()
            || !(args.len() == 1 || mixed)
            || target.params[0].ty != Type::Bytes
            || target.params[0].mode != ParamMode::Own
        {
            return Err(hir_error(
                "affine closure target must be pure (own Bytes)->i64",
            ));
        }
        let scalar = if mixed {
            let ExprKind::Var(name) = &args[1].kind else {
                return Err(hir_error("mixed capture requires a direct scalar binding"));
            };
            let binding = outer
                .get(name)
                .ok_or_else(|| hir_error("mixed scalar capture absent"))?;
            if binding.ty != ResolvedType::I64
                || binding.ownership != OwnershipMode::Value
                || target.params[1].ty != Type::I64
                || target.params[1].mode != ParamMode::Value
            {
                return Err(hir_error("mixed capture requires available value i64"));
            }
            Some((name.clone(), binding.clone()))
        } else {
            None
        };
        let outer = outer
            .get(captured)
            .ok_or_else(|| hir_error("affine capture is absent"))?;
        if outer.ty != ResolvedType::Bytes || outer.ownership != OwnershipMode::Own {
            return Err(hir_error("affine capture must own Bytes"));
        }
        let id = ExpressionId::new(parent, path);
        let target = closure_id(&id);
        if self.declarations.declaration(&target).is_some() {
            return Err(hir_error(
                "affine closure identity collides with declared identity",
            ));
        }
        let execution = FunctionExecutionId::Monomorphic(target);
        let binding = ResolvedBinding {
            id: ValueId::parameter(&execution, 0),
            name: captured.clone(),
            ownership: OwnershipMode::Own,
            ty: ResolvedType::Bytes,
            span: expression.span,
        };
        let capture = ResolvedClosureCapture {
            binding: binding.clone(),
            value: ResolvedExpr {
                id: ExpressionId::new(parent, &format!("{path}.capture.0")),
                ty: ResolvedType::Bytes,
                ownership: OwnershipMode::Own,
                kind: ResolvedExprKind::Place(Place {
                    root: outer.id.clone(),
                    projections: Vec::new(),
                }),
                span: expression.span,
            },
        };
        let mut bindings = BTreeMap::from([(
            captured.clone(),
            Binding {
                id: binding.id,
                ty: binding.ty,
                ownership: OwnershipMode::Own,
                mutable: false,
            },
        )]);
        let mut captures = vec![capture];
        if let Some((name, outer)) = scalar {
            let binding = ResolvedBinding {
                id: ValueId::parameter(&execution, 1),
                name: name.clone(),
                ownership: OwnershipMode::Value,
                ty: ResolvedType::I64,
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
                    id: ExpressionId::new(parent, &format!("{path}.capture.1")),
                    ty: ResolvedType::I64,
                    ownership: OwnershipMode::Value,
                    kind: ResolvedExprKind::Place(Place {
                        root: outer.id,
                        projections: Vec::new(),
                    }),
                    span: expression.span,
                },
            });
        }
        #[cfg(test)]
        let body = if reference {
            self.resolve_expr_recursive_reference(&execution, body, &bindings, "body")?
        } else {
            self.resolve_expr(&execution, body, &bindings, "body")?
        };
        #[cfg(not(test))]
        let body = {
            let _ = reference;
            self.resolve_expr(&execution, body, &bindings, "body")?
        };
        Ok(ResolvedExpr {
            id,
            ty: if mixed {
                ResolvedType::OnceFunctionI64
            } else {
                ResolvedType::OnceFunction
            },
            ownership: OwnershipMode::Own,
            kind: ResolvedExprKind::Closure {
                parameters: Vec::new(),
                captures,
                body: Box::new(body),
            },
            span: expression.span,
        })
    }
}

pub(crate) fn validate(
    program: &ResolvedProgram,
    expression: &ResolvedExpr,
) -> Result<(), Diagnostic> {
    let ResolvedExprKind::Closure {
        parameters,
        captures,
        body,
    } = &expression.kind
    else {
        return Err(hir_error("affine closure shape missing"));
    };
    let mixed = expression.ty == ResolvedType::OnceFunctionI64;
    if captures.len() != if mixed { 2 } else { 1 } {
        return Err(hir_error("affine closure capture schema mismatch"));
    }
    let capture = &captures[0];
    let execution = FunctionExecutionId::Monomorphic(closure_id(&expression.id));
    if program
        .declarations
        .declaration(&closure_id(&expression.id))
        .is_some()
    {
        return Err(hir_error(
            "affine closure identity collides with a declared function",
        ));
    }
    if !expression.ty.is_once_function()
        || expression.ownership != OwnershipMode::Own
        || !parameters.is_empty()
        || body.ty != ResolvedType::I64
        || capture.binding.id != ValueId::parameter(&execution, 0)
        || capture.binding.ownership != OwnershipMode::Own
        || capture.binding.ty != ResolvedType::Bytes
        || capture.value.ownership != OwnershipMode::Own
        || capture.value.ty != ResolvedType::Bytes
        || !matches!(&capture.value.kind, ResolvedExprKind::Place(p) if p.projections.is_empty())
    {
        return Err(hir_error(
            "affine capture signature or ownership is invalid",
        ));
    }
    if mixed {
        let scalar = &captures[1];
        if scalar.binding.id != ValueId::parameter(&execution, 1)
            || scalar.binding.ty != ResolvedType::I64
            || scalar.binding.ownership != OwnershipMode::Value
            || scalar.value.ty != ResolvedType::I64
            || scalar.value.ownership != OwnershipMode::Value
            || !matches!(&scalar.value.kind, ResolvedExprKind::Place(p) if p.projections.is_empty())
        {
            return Err(hir_error("mixed affine scalar capture schema mismatch"));
        }
    }
    let call = match &body.kind {
        ResolvedExprKind::Block { statements, tail } if statements.is_empty() => tail.as_ref(),
        _ => return Err(hir_error("affine body must retain one tail call")),
    };
    let ResolvedExprKind::Call {
        callee,
        instance: None,
        type_arguments,
        args,
    } = &call.kind
    else {
        return Err(hir_error("affine body must call one ordinary function"));
    };
    let target = program
        .functions
        .iter()
        .find(|f| f.id == *callee)
        .ok_or_else(|| hir_error("affine target is absent"))?;
    if !type_arguments.is_empty()
        || target.params.len() != captures.len()
        || args.len() != captures.len()
        || target.params[0].ty != ResolvedType::Bytes
        || target.params[0].ownership != OwnershipMode::Own
        || target.return_type != ResolvedType::I64
        || !target.effects.is_empty()
        || args.len() != captures.len()
        || args[0].ty != ResolvedType::Bytes
        || args[0].ownership != OwnershipMode::Own
        || !matches!(&args[0].kind, ResolvedExprKind::Place(p) if p.root == capture.binding.id && p.projections.is_empty())
    {
        return Err(hir_error("affine body does not transfer its exact capture"));
    }
    if mixed
        && (target.params[1].ty != ResolvedType::I64
            || target.params[1].ownership != OwnershipMode::Value
            || args[1].ty != ResolvedType::I64
            || args[1].ownership != OwnershipMode::Value
            || !matches!(&args[1].kind, ResolvedExprKind::Place(p) if p.root == captures[1].binding.id && p.projections.is_empty()))
    {
        return Err(hir_error(
            "mixed affine body must pass its exact scalar capture",
        ));
    }
    Ok(())
}

/// Canonical call boundaries for retained creation and once-only invocation.
/// Both use the ordinary owned-argument staging/commit machinery.
pub(crate) fn call(expression: &ResolvedExpr) -> Option<(&'static DeclarationId, &[ResolvedExpr])> {
    static CONSTRUCT: std::sync::LazyLock<DeclarationId> =
        std::sync::LazyLock::new(|| DeclarationId::new(CONSTRUCT_ID));
    static INVOKE: std::sync::LazyLock<DeclarationId> =
        std::sync::LazyLock::new(|| DeclarationId::new(INVOKE_ID));
    static MIXED_CONSTRUCT: std::sync::LazyLock<DeclarationId> =
        std::sync::LazyLock::new(|| DeclarationId::new(MIXED_CONSTRUCT_ID));
    static MIXED_INVOKE: std::sync::LazyLock<DeclarationId> =
        std::sync::LazyLock::new(|| DeclarationId::new(MIXED_INVOKE_ID));
    match &expression.kind {
        ResolvedExprKind::Closure { captures, .. }
            if expression.ty.is_once_function()
                && captures.len()
                    == if expression.ty == ResolvedType::OnceFunctionI64 {
                        2
                    } else {
                        1
                    } =>
        {
            Some((
                if expression.ty == ResolvedType::OnceFunctionI64 {
                    &MIXED_CONSTRUCT
                } else {
                    &CONSTRUCT
                },
                std::slice::from_ref(&captures[0].value),
            ))
        }
        ResolvedExprKind::Invoke { callable, args }
            if callable.ty.is_once_function() && args.is_empty() =>
        {
            Some((
                if callable.ty == ResolvedType::OnceFunctionI64 {
                    &MIXED_INVOKE
                } else {
                    &INVOKE
                },
                std::slice::from_ref(callable.as_ref()),
            ))
        }
        _ => None,
    }
}

pub(crate) fn params(callee: &DeclarationId) -> Option<Vec<ResolvedParam>> {
    let ty = match callee.as_str() {
        CONSTRUCT_ID | MIXED_CONSTRUCT_ID => ResolvedType::Bytes,
        INVOKE_ID => ResolvedType::OnceFunction,
        MIXED_INVOKE_ID => ResolvedType::OnceFunctionI64,
        _ => return None,
    };
    Some(vec![ResolvedParam {
        id: ValueId::intrinsic_parameter(callee.as_str(), 0),
        name: "owner".into(),
        ownership: OwnershipMode::Own,
        ty,
        span: crate::ast::Span::default(),
    }])
}

/// V2 intrinsic identities cannot be supplied by authored declarations.
pub(crate) fn reject_reserved_identities(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    if program.declarations.declarations().any(|declaration| {
        matches!(
            declaration.id.as_str(),
            MIXED_CONSTRUCT_ID | MIXED_INVOKE_ID | MIXED_DROP_ID
        )
    }) {
        return Err(hir_error(
            "authored declaration aliases a mixed affine intrinsic",
        ));
    }
    Ok(())
}

/// Runtime carriers are required by checked helper signatures even when no
/// factory literal is present. This does not add graph closure definitions.
pub(crate) fn uses_type(program: &ResolvedProgram, ty: &ResolvedType) -> bool {
    program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
        .any(|function| {
            let mut found =
                &function.return_type == ty || function.params.iter().any(|p| &p.ty == ty);
            super::super::function_value::walk(function, |expression| {
                found |= &expression.ty == ty
            });
            found
        })
}
pub(crate) fn requires_bytes(program: &ResolvedProgram) -> bool {
    uses_type(program, &ResolvedType::OnceFunction)
        || uses_type(program, &ResolvedType::OnceFunctionI64)
}
