//! Transactional mutable carrier body product. Source admission remains closed
//! until every backend implements state commit and receiver re-entry refusal.
use super::*;
use crate::ast::{Expr, ExprKind, ParamMode, Statement, Type};
use std::collections::BTreeMap;

impl Resolver<'_> {
    pub(in crate::hir) fn resolve_mutable_closure(
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
            owning: false,
            retained: false,
            mutable: true,
        } = &expression.kind
        else {
            return Err(hir_error("invalid transactional mutable closure literal"));
        };
        if params.len() != 1 || params[0].ty != Type::I64 {
            return Err(hir_error(
                "mutable closure requires one i64 invocation parameter",
            ));
        }
        let owner = parent
            .monomorphic_declaration()
            .and_then(|id| {
                self.program
                    .functions
                    .iter()
                    .find(|f| f.stable_id == id.as_str())
            })
            .ok_or_else(|| {
                hir_error("mutable closures require an ordinary lexical creation site")
            })?;
        if !owner.type_parameters.is_empty() {
            return Err(hir_error(
                "generic mutable closure creation is not admitted",
            ));
        }
        let ExprKind::Block { statements, tail } = &body.kind else {
            return Err(hir_error(
                "mutable closure body requires a transactional state update",
            ));
        };
        let [Statement::Assign {
            name: state,
            field: None,
            value: next,
            ..
        }] = statements.as_slice()
        else {
            return Err(hir_error(
                "mutable closure requires exactly one direct state assignment",
            ));
        };
        if state == &params[0].name || !matches!(&tail.kind, ExprKind::Var(name) if name == state) {
            return Err(hir_error(
                "mutable closure must return its exact updated capture",
            ));
        }
        let ExprKind::Call {
            name,
            type_arguments,
            args,
        } = &next.kind
        else {
            return Err(hir_error(
                "mutable closure update must be a checked ordinary function call",
            ));
        };
        if !type_arguments.is_empty()
            || args.len() != 2
            || !matches!(&args[0].kind, ExprKind::Var(name) if name == state)
            || !matches!(&args[1].kind, ExprKind::Var(name) if name == &params[0].name)
        {
            return Err(hir_error(
                "mutable closure update must pass state then invocation argument",
            ));
        }
        let target = self
            .program
            .functions
            .iter()
            .find(|f| f.name == *name)
            .ok_or_else(|| hir_error("mutable closure update function is absent"))?;
        if !target.effects.is_empty()
            || !target.type_parameters.is_empty()
            || target.return_type != Type::I64
            || target.params.len() != 2
            || target
                .params
                .iter()
                .any(|p| p.ty != Type::I64 || p.mode != ParamMode::Value)
        {
            return Err(hir_error(
                "mutable closure update requires pure (i64, i64) -> i64",
            ));
        }
        let capture = outer
            .get(state)
            .ok_or_else(|| hir_error("mutable state capture is absent"))?;
        if capture.ty != ResolvedType::I64 || capture.ownership != OwnershipMode::Value {
            return Err(hir_error(
                "mutable state capture must be an available value i64",
            ));
        }
        let id = ExpressionId::new(parent, path);
        let target = closure_id(&id);
        if self.declarations.declaration(&target).is_some() {
            return Err(hir_error(
                "mutable closure identity collides with a declaration",
            ));
        }
        let execution = FunctionExecutionId::Monomorphic(target);
        let binding = ResolvedBinding {
            id: ValueId::parameter(&execution, 0),
            name: state.clone(),
            ownership: OwnershipMode::Value,
            ty: ResolvedType::I64,
            span: expression.span,
        };
        let parameter = ResolvedBinding {
            id: ValueId::parameter(&execution, 1),
            name: params[0].name.clone(),
            ownership: OwnershipMode::Value,
            ty: ResolvedType::I64,
            span: params[0].span,
        };
        let bindings = [&binding, &parameter]
            .into_iter()
            .map(|p| {
                (
                    p.name.clone(),
                    Binding {
                        id: p.id.clone(),
                        ty: p.ty.clone(),
                        ownership: p.ownership,
                        mutable: false,
                    },
                )
            })
            .collect();
        // The call result is the candidate state. Only the carrier invocation
        // commits it, after this checked body and its callee contracts succeed.
        let candidate_body = Expr {
            kind: ExprKind::Block {
                statements: Vec::new(),
                tail: Box::new(next.clone()),
            },
            span: body.span,
        };
        #[cfg(test)]
        let body = if reference {
            self.resolve_expr_recursive_reference(&execution, &candidate_body, &bindings, "body")?
        } else {
            self.resolve_expr(&execution, &candidate_body, &bindings, "body")?
        };
        #[cfg(not(test))]
        let body = {
            let _ = reference;
            self.resolve_expr(&execution, &candidate_body, &bindings, "body")?
        };
        Ok(ResolvedExpr {
            id,
            ty: ResolvedType::MutFunctionI64,
            ownership: OwnershipMode::Value,
            kind: ResolvedExprKind::Closure {
                parameters: vec![parameter],
                captures: vec![ResolvedClosureCapture {
                    binding,
                    value: ResolvedExpr {
                        id: ExpressionId::new(parent, &format!("{path}.capture.0")),
                        ty: ResolvedType::I64,
                        ownership: OwnershipMode::Value,
                        kind: ResolvedExprKind::Place(Place {
                            root: capture.id.clone(),
                            projections: Vec::new(),
                        }),
                        span: expression.span,
                    },
                }],
                body: Box::new(body),
            },
            span: expression.span,
        })
    }
}

/// Independently reconstruct the exact state/argument order from retained HIR;
/// the emitter cannot accept a forged capture, arbitrary body or new effect.
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
        return Err(hir_error("mutable closure shape is absent"));
    };
    if expression.ty != ResolvedType::MutFunctionI64
        || expression.ownership != OwnershipMode::Value
        || captures.len() != 1
        || parameters.len() != 1
        || body.ty != ResolvedType::I64
        || body.ownership != OwnershipMode::Value
    {
        return Err(hir_error(
            "mutable closure signature or capture schema is invalid",
        ));
    }
    let execution = FunctionExecutionId::Monomorphic(closure_id(&expression.id));
    let capture = &captures[0];
    let parameter = &parameters[0];
    if program
        .declarations
        .declaration(&closure_id(&expression.id))
        .is_some()
        || capture.binding.id != ValueId::parameter(&execution, 0)
        || parameter.id != ValueId::parameter(&execution, 1)
        || capture.binding.name == parameter.name
        || [&capture.binding, parameter].iter().any(|binding| {
            binding.ty != ResolvedType::I64 || binding.ownership != OwnershipMode::Value
        })
        || capture.value.ty != ResolvedType::I64
        || capture.value.ownership != OwnershipMode::Value
        || !matches!(&capture.value.kind, ResolvedExprKind::Place(p) if p.projections.is_empty())
    {
        return Err(hir_error(
            "mutable closure capture and parameter identities are invalid",
        ));
    }
    let body = match &body.kind {
        ResolvedExprKind::Block { statements, tail } if statements.is_empty() => tail.as_ref(),
        _ => {
            return Err(hir_error(
                "mutable candidate state body must be one canonical tail call",
            ))
        }
    };
    let ResolvedExprKind::Call {
        callee,
        instance: None,
        type_arguments,
        args,
    } = &body.kind
    else {
        return Err(hir_error(
            "mutable candidate state must come from an ordinary checked call",
        ));
    };
    let target = program
        .functions
        .iter()
        .find(|f| f.id == *callee)
        .ok_or_else(|| hir_error("mutable update target is absent"))?;
    if !type_arguments.is_empty()
        || args.len() != 2
        || target.params.len() != 2
        || target.return_type != ResolvedType::I64
        || !target.effects.is_empty()
        || target
            .params
            .iter()
            .any(|p| p.ty != ResolvedType::I64 || p.ownership != OwnershipMode::Value)
    {
        return Err(hir_error(
            "mutable update target violates its fixed checked signature",
        ));
    }
    for (argument, binding) in args.iter().zip([&capture.binding, parameter]) {
        if argument.ty != ResolvedType::I64
            || argument.ownership != OwnershipMode::Value
            || !matches!(&argument.kind, ResolvedExprKind::Place(p) if p.root == binding.id && p.projections.is_empty())
        {
            return Err(hir_error(
                "mutable candidate state must use its exact state and argument",
            ));
        }
    }
    Ok(())
}

/// Receiver mutability is reconstructed from declaration metadata, rather than
/// trusted from the invocation's type. Scope validation separately proves that
/// this exact globally unique binding is live at the call site.
pub(crate) fn validate_receiver(
    program: &ResolvedProgram,
    callable: &ResolvedExpr,
    allow_moves: bool,
) -> Result<(), Diagnostic> {
    if !allow_moves {
        return Err(hir_error(
            "mutable invocation cannot update state in a contract",
        ));
    }
    let ResolvedExprKind::Place(place) = &callable.kind else {
        return Err(hir_error(
            "mutable invocation requires a direct local receiver",
        ));
    };
    let mut found = 0usize;
    let mut mutable = false;
    for function in program.functions.iter().chain(
        program
            .function_instances
            .iter()
            .map(|instance| &instance.function),
    ) {
        super::super::function_value::walk(function, |expression| {
            if let ResolvedExprKind::Block { statements, .. } = &expression.kind {
                for statement in statements {
                    if let ResolvedStatement::Let {
                        binding,
                        mutable: declared,
                        ..
                    } = statement
                    {
                        if binding.id == place.root {
                            found += 1;
                            mutable = *declared
                                && binding.ty == callable.ty
                                && binding.ownership == OwnershipMode::Value;
                        }
                    }
                }
            }
        });
    }
    if found != 1 || !mutable || !place.projections.is_empty() {
        return Err(hir_error(
            "mutable invocation requires its unique mutable local declaration",
        ));
    }
    Ok(())
}
