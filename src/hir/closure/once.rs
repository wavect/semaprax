//! Closed affine callable profile: one owned Bytes capture, no arguments, i64.
use super::*;
use crate::ast::{Expr, ExprKind, ParamMode, Type};
use std::collections::BTreeMap;

pub(crate) const DROP_ID: &str = "core.fn_once.drop";
pub(crate) const CONSTRUCT_ID: &str = "core.fn_once.construct";
pub(crate) const INVOKE_ID: &str = "core.fn_once.invoke";

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
        let [Expr {
            kind: ExprKind::Var(captured),
            ..
        }] = args.as_slice()
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
            || target.params.len() != 1
            || target.params[0].ty != Type::Bytes
            || target.params[0].mode != ParamMode::Own
        {
            return Err(hir_error(
                "affine closure target must be pure (own Bytes)->i64",
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
        let bindings = BTreeMap::from([(
            captured.clone(),
            Binding {
                id: binding.id,
                ty: binding.ty,
                ownership: OwnershipMode::Own,
                mutable: false,
            },
        )]);
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
            ty: ResolvedType::OnceFunction,
            ownership: OwnershipMode::Own,
            kind: ResolvedExprKind::Closure {
                parameters: Vec::new(),
                captures: vec![capture],
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
    let [capture] = captures.as_slice() else {
        return Err(hir_error("affine closure requires exactly one capture"));
    };
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
    if expression.ty != ResolvedType::OnceFunction
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
        || target.params.len() != 1
        || target.params[0].ty != ResolvedType::Bytes
        || target.params[0].ownership != OwnershipMode::Own
        || target.return_type != ResolvedType::I64
        || !target.effects.is_empty()
        || args.len() != 1
        || args[0].ty != ResolvedType::Bytes
        || args[0].ownership != OwnershipMode::Own
        || !matches!(&args[0].kind, ResolvedExprKind::Place(p) if p.root == capture.binding.id && p.projections.is_empty())
    {
        return Err(hir_error("affine body does not transfer its exact capture"));
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
    match &expression.kind {
        ResolvedExprKind::Closure { captures, .. }
            if expression.ty == ResolvedType::OnceFunction && captures.len() == 1 =>
        {
            Some((&CONSTRUCT, std::slice::from_ref(&captures[0].value)))
        }
        ResolvedExprKind::Invoke { callable, args }
            if callable.ty == ResolvedType::OnceFunction && args.is_empty() =>
        {
            Some((&INVOKE, std::slice::from_ref(callable.as_ref())))
        }
        _ => None,
    }
}

pub(crate) fn params(callee: &DeclarationId) -> Option<Vec<ResolvedParam>> {
    let ty = match callee.as_str() {
        CONSTRUCT_ID => ResolvedType::Bytes,
        INVOKE_ID => ResolvedType::OnceFunction,
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
