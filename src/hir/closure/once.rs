//! Closed affine callable profile: one owned Bytes capture, no arguments, i64.
use super::*;
use crate::ast::{Expr, ExprKind, ParamMode, Type};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const DROP_ID: &str = "core.fn_once.drop";
pub(crate) const CONSTRUCT_ID: &str = "core.fn_once.construct";
pub(crate) const INVOKE_ID: &str = "core.fn_once.invoke";
pub(crate) const MIXED_DROP_ID: &str = "core.fn_once_i64.drop.v2";
pub(crate) const MIXED_CONSTRUCT_ID: &str = "core.fn_once_i64.construct.v2";
pub(crate) const MIXED_INVOKE_ID: &str = "core.fn_once_i64.invoke.v2";
pub(crate) const PAIR_DROP_ID: &str = "core.fn_once_i64_pair.drop.v3";
pub(crate) const PAIR_CONSTRUCT_ID: &str = "core.fn_once_i64_pair.construct.v3";
pub(crate) const PAIR_INVOKE_ID: &str = "core.fn_once_i64_pair.invoke.v3";

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
            mutable: false,
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
        let pair = args.len() == 3;
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
            || !(args.len() == 1 || mixed || pair)
            || target.params[0].ty != Type::Bytes
            || target.params[0].mode != ParamMode::Own
        {
            return Err(hir_error(
                "affine closure target must be pure (own Bytes)->i64",
            ));
        }
        let scalars = args[1..]
            .iter()
            .enumerate()
            .map(|(offset, argument)| {
                let ExprKind::Var(name) = &argument.kind else {
                    return Err(hir_error("mixed capture requires a direct scalar binding"));
                };
                let binding = outer
                    .get(name)
                    .ok_or_else(|| hir_error("mixed scalar capture absent"))?;
                let parameter = &target.params[offset + 1];
                if binding.ty != ResolvedType::I64
                    || binding.ownership != OwnershipMode::Value
                    || parameter.ty != Type::I64
                    || parameter.mode != ParamMode::Value
                {
                    return Err(hir_error("mixed capture requires available value i64"));
                }
                Ok((name.clone(), binding.clone()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if scalars
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<BTreeSet<_>>()
            .len()
            != scalars.len()
        {
            return Err(hir_error(
                "mixed scalar captures must name distinct bindings",
            ));
        }
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
        for (offset, (name, outer)) in scalars.into_iter().enumerate() {
            let binding = ResolvedBinding {
                id: ValueId::parameter(&execution, offset + 1),
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
                    id: ExpressionId::new(parent, &format!("{path}.capture.{}", offset + 1)),
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
            ty: if pair {
                ResolvedType::OnceFunctionI64Pair
            } else if mixed {
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
    let scalar_count = match expression.ty {
        ResolvedType::OnceFunction => 0,
        ResolvedType::OnceFunctionI64 => 1,
        ResolvedType::OnceFunctionI64Pair => 2,
        _ => return Err(hir_error("affine callable type is invalid")),
    };
    if captures.len() != scalar_count + 1 {
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
    for (offset, scalar) in captures[1..].iter().enumerate() {
        if scalar.binding.id != ValueId::parameter(&execution, offset + 1)
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
    for (offset, scalar) in captures[1..].iter().enumerate() {
        let index = offset + 1;
        if target.params[index].ty != ResolvedType::I64
            || target.params[index].ownership != OwnershipMode::Value
            || args[index].ty != ResolvedType::I64
            || args[index].ownership != OwnershipMode::Value
            || !matches!(&args[index].kind, ResolvedExprKind::Place(p) if p.root == scalar.binding.id && p.projections.is_empty())
        {
            return Err(hir_error(
                "mixed affine body must pass its exact scalar capture",
            ));
        }
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
    static PAIR_CONSTRUCT: std::sync::LazyLock<DeclarationId> =
        std::sync::LazyLock::new(|| DeclarationId::new(PAIR_CONSTRUCT_ID));
    static PAIR_INVOKE: std::sync::LazyLock<DeclarationId> =
        std::sync::LazyLock::new(|| DeclarationId::new(PAIR_INVOKE_ID));
    match &expression.kind {
        ResolvedExprKind::Closure { captures, .. }
            if expression.ty.is_once_function()
                && captures.len()
                    == match expression.ty {
                        ResolvedType::OnceFunction => 1,
                        ResolvedType::OnceFunctionI64 => 2,
                        ResolvedType::OnceFunctionI64Pair => 3,
                        _ => return None,
                    } =>
        {
            Some((
                if expression.ty == ResolvedType::OnceFunctionI64Pair {
                    &PAIR_CONSTRUCT
                } else if expression.ty == ResolvedType::OnceFunctionI64 {
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
                if callable.ty == ResolvedType::OnceFunctionI64Pair {
                    &PAIR_INVOKE
                } else if callable.ty == ResolvedType::OnceFunctionI64 {
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
        CONSTRUCT_ID | MIXED_CONSTRUCT_ID | PAIR_CONSTRUCT_ID => ResolvedType::Bytes,
        INVOKE_ID => ResolvedType::OnceFunction,
        MIXED_INVOKE_ID => ResolvedType::OnceFunctionI64,
        PAIR_INVOKE_ID => ResolvedType::OnceFunctionI64Pair,
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
            MIXED_CONSTRUCT_ID
                | MIXED_INVOKE_ID
                | MIXED_DROP_ID
                | PAIR_CONSTRUCT_ID
                | PAIR_INVOKE_ID
                | PAIR_DROP_ID
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
        || uses_type(program, &ResolvedType::OnceFunctionI64Pair)
}
