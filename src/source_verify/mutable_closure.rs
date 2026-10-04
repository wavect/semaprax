//! Narrow noncopyable transactional scalar callback admission.
use super::binding::{Availability, Binding, CheckedValue};
use super::diagnostics::{error, source_identifier};
use crate::ast::{Expr, ExprKind, Function, ParamMode, Program, Span, Type};
use crate::diagnostic::Diagnostic;
use std::collections::HashMap;

pub(super) fn construction(
    program: &Program,
    current: &Function,
    expression: &Expr,
    variables: &HashMap<String, Binding>,
) -> Result<CheckedValue, Diagnostic> {
    let reject = |message| error(program, "SPX-T308", message, expression.span);
    let ExprKind::Closure {
        params,
        return_type: Type::I64,
        body,
        owning: false,
        retained: false,
        mutable: true,
    } = &expression.kind
    else {
        return Err(reject(
            "mutable closure requires the transactional i64 signature",
        ));
    };
    if !current.type_parameters.is_empty()
        || params.len() != 1
        || params[0].ty != Type::I64
        || !source_identifier(&params[0].name)
    {
        return Err(reject(
            "mutable closure requires one i64 argument in a monomorphic function",
        ));
    }
    let ExprKind::Block { statements, tail } = &body.kind else {
        return Err(reject("mutable closure requires its state-update block"));
    };
    if !statements.is_empty() {
        return Err(reject("mutable closure requires a direct transition call"));
    }
    let ExprKind::Call {
        name,
        type_arguments,
        args,
    } = &tail.kind
    else {
        return Err(reject("mutable closure requires a direct transition call"));
    };
    let [state, argument] = args.as_slice() else {
        return Err(reject("mutable transition requires state and argument"));
    };
    let ExprKind::Var(state) = &state.kind else {
        return Err(reject(
            "mutable state must be a direct available i64 binding",
        ));
    };
    if !type_arguments.is_empty()
        || state == &params[0].name
        || variables.contains_key(name)
        || name == &params[0].name
        || !matches!(&argument.kind, ExprKind::Var(name) if name == &params[0].name)
    {
        return Err(reject(
            "mutable transition must pass state then invocation argument",
        ));
    }
    let Some(capture) = variables.get(state) else {
        return Err(reject(
            "mutable closure state must be a lexical i64 capture",
        ));
    };
    if capture.ty != Type::I64
        || capture.mode != ParamMode::Value
        || capture.availability != Availability::Available
    {
        return Err(reject(
            "mutable closure state must be an available value i64",
        ));
    }
    let target = program
        .functions
        .iter()
        .find(|function| function.name == *name)
        .ok_or_else(|| reject("mutable update target must be an ordinary local function"))?;
    if !target.type_parameters.is_empty()
        || !target.effects.is_empty()
        || target.return_type != Type::I64
        || target.params.len() != 2
        || target
            .params
            .iter()
            .any(|p| p.ty != Type::I64 || p.mode != ParamMode::Value)
    {
        return Err(reject(
            "mutable update target requires pure (i64, i64) -> i64",
        ));
    }
    Ok(CheckedValue::value(Type::MutFunctionI64))
}

/// Reuse ordinary argument checking only after checking the distinct receiver.
/// The returned synthetic signature carries no ability to construct a plain
/// function value from the stateful receiver.
pub(super) fn invocation_signature(
    program: &Program,
    binding: &Binding,
    name: &str,
    args: &[Expr],
    allow_moves: bool,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) -> Type {
    if binding.ty != Type::MutFunctionI64 {
        return binding.ty.clone();
    }
    if !binding.mutable
        || binding.mode != ParamMode::Own
        || binding.availability != Availability::Available
        || !allow_moves
    {
        diagnostics.push(error(
            program,
            "SPX-T308",
            "mutable invocation requires an available mutable local outside contracts",
            span,
        ));
    }
    for argument in args {
        argument.visit_calls(&mut |called, span| {
            if called == name {
                diagnostics.push(error(
                    program,
                    "SPX-T308",
                    "mutable receiver cannot be entered while staging its invocation argument",
                    span,
                ));
            }
        });
    }
    Type::Function {
        parameters: vec![Type::I64],
        result: Box::new(Type::I64),
    }
}
