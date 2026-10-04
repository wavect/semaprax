//! Exact synchronous parameter-rooted borrowed-text capture profile.
use super::binding::{Availability, Binding, CheckedValue};
use super::diagnostics::error;
use crate::ast::{Expr, ExprKind, Function, ParamMode, Program, Statement, Type};
use crate::diagnostic::Diagnostic;
use std::collections::{BTreeMap, HashMap};

pub(crate) fn profile(
    program: &Program,
    current: &Function,
    expression: &Expr,
    outer: &BTreeMap<&str, &Type>,
) -> Result<Option<String>, Diagnostic> {
    let ExprKind::Closure {
        params,
        return_type,
        body,
        owning: false,
        retained: false,
        mutable: false,
    } = &expression.kind
    else {
        return Ok(None);
    };
    let mut mentions_borrow = false;
    body.visit_all_nodes(&mut |node| {
        if let ExprKind::Var(name) = &node.kind {
            if !params.iter().any(|p| p.name == *name)
                && outer.get(name.as_str()).is_some_and(|ty| **ty == Type::Str)
            {
                mentions_borrow = true;
            }
        }
    });
    if !mentions_borrow {
        return Ok(None);
    }
    let reject = |message| error(program, "SPX-T288", message, expression.span);
    if !current.type_parameters.is_empty()
        || !current.effects.is_empty()
        || current.yields.is_some()
        || params.len() != 1
        || params[0].ty != Type::I64
        || *return_type != Type::I64
    {
        return Err(reject(
            "borrowed closure requires the synchronous monomorphic i64 profile",
        ));
    }
    let ExprKind::Block { statements, tail } = &body.kind else {
        return Err(reject(
            "borrowed closure requires one direct transition call",
        ));
    };
    let ExprKind::Call {
        name,
        type_arguments,
        args,
    } = &tail.kind
    else {
        return Err(reject(
            "borrowed closure requires one direct transition call",
        ));
    };
    let [view, argument] = args.as_slice() else {
        return Err(reject(
            "borrowed closure requires one view and one i64 argument",
        ));
    };
    let ExprKind::Var(view) = &view.kind else {
        return Err(reject("borrowed capture must name its direct parameter"));
    };
    if !statements.is_empty()
        || !type_arguments.is_empty()
        || outer.contains_key(name.as_str())
        || view == &params[0].name
        || name == &params[0].name
        || !matches!(&argument.kind, ExprKind::Var(name) if name == &params[0].name)
        || !current
            .params
            .iter()
            .any(|p| p.name == *view && p.ty == Type::Str && p.mode == ParamMode::Borrow)
    {
        return Err(reject(
            "borrowed closure must capture its direct borrow str parameter",
        ));
    }
    let target = program
        .functions
        .iter()
        .find(|f| f.name == *name)
        .ok_or_else(|| reject("borrowed closure transition must be an ordinary local function"))?;
    if !target.type_parameters.is_empty()
        || !target.effects.is_empty()
        || target.yields.is_some()
        || target.return_type != Type::I64
        || target.params.len() != 2
        || target.params[0].ty != Type::Str
        || target.params[0].mode != ParamMode::Borrow
        || target.params[1].ty != Type::I64
        || target.params[1].mode != ParamMode::Value
    {
        return Err(reject(
            "borrowed transition requires pure (borrow str, i64) -> i64",
        ));
    }
    let mut callback = None;
    let mut shadowed = false;
    current.body.visit_all_nodes(&mut |node| {
        if let ExprKind::Block { statements, .. } = &node.kind {
            for statement in statements {
                if let Statement::Let {
                    name,
                    mutable,
                    value,
                    ..
                } = statement
                {
                    shadowed |= name == view;
                    if value.span == expression.span
                        && matches!(value.kind, ExprKind::Closure { .. })
                    {
                        if *mutable {
                            shadowed = true;
                        }
                        callback = Some(name.clone());
                    }
                }
            }
        }
    });
    let callback = callback
        .ok_or_else(|| reject("borrowed closure must directly initialize an immutable local"))?;
    let mut declarations = 0;
    current.body.visit_all_nodes(&mut |node| match &node.kind {
        ExprKind::Var(name) if name == &callback => shadowed = true,
        ExprKind::Block { statements, .. } => {
            declarations += statements
                .iter()
                .filter(|s| matches!(s, Statement::Let { name, .. } if name == &callback))
                .count();
        }
        _ => {}
    });
    if shadowed || declarations != 1 || current.params.iter().any(|p| p.name == callback) {
        return Err(reject(
            "borrowed callback cannot escape, alias, or shadow its scope",
        ));
    }
    Ok(Some(view.clone()))
}

pub(super) fn construction(
    program: &Program,
    current: &Function,
    expression: &Expr,
    variables: &HashMap<String, Binding>,
) -> Result<Option<CheckedValue>, Diagnostic> {
    let outer = variables
        .iter()
        .map(|(name, binding)| (name.as_str(), &binding.ty))
        .collect();
    let Some(name) = profile(program, current, expression, &outer)? else {
        return Ok(None);
    };
    let binding = &variables[&name];
    if binding.mode != ParamMode::Borrow || binding.availability != Availability::Available {
        return Err(error(
            program,
            "SPX-T288",
            "borrowed capture must be an available shared parameter",
            expression.span,
        ));
    }
    Ok(Some(CheckedValue::value(Type::Function {
        parameters: vec![Type::I64],
        result: Box::new(Type::I64),
    })))
}
