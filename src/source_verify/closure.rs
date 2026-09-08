//! Closed scalar snapshot capture analysis shared with HIR source lowering.
use super::binding::{Availability, Binding, CheckedValue};
use super::declared_type::{function_value_scalar_type, function_value_signature};
use super::diagnostics::{error, source_identifier};
use super::scope::{VerifierFrame, VerifierScope};
use super::IterativeVerifier;
use crate::ast::{ClosureParam, Expr, ExprKind, ParamMode, Program, Statement, Type};
use crate::diagnostic::Diagnostic;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

pub(crate) fn capture_names(
    program: &Program,
    params: &[ClosureParam],
    return_type: &Type,
    body: &Expr,
    outer: &BTreeMap<&str, &Type>,
) -> Result<Vec<String>, Diagnostic> {
    let reject = |message| error(program, "SPX-T288", message, body.span);
    if params.len() > 8
        || !function_value_scalar_type(return_type)
        || params
            .iter()
            .any(|p| !function_value_scalar_type(&p.ty) || !source_identifier(&p.name))
    {
        return Err(reject(
            "closures require zero through eight named scalar parameters and a scalar result",
        ));
    }
    let names = params
        .iter()
        .map(|p| p.name.as_str())
        .collect::<BTreeSet<_>>();
    if names.len() != params.len() {
        return Err(reject("closure parameter names must be unique"));
    }
    enum Item<'a> {
        Expr(&'a Expr, BTreeSet<&'a str>),
        Block(&'a [Statement], usize, &'a Expr, BTreeSet<&'a str>),
    }
    let mut pending = vec![Item::Expr(body, names)];
    let mut captures = BTreeSet::new();
    let mut nodes = 0usize;
    while let Some(item) = pending.pop() {
        nodes += 1;
        if nodes > 4096 {
            return Err(reject("closure scalar body exceeds 4096 analysis nodes"));
        }
        match item {
            Item::Block(statements, next, tail, mut locals) => {
                let Some(statement) = statements.get(next) else { pending.push(Item::Expr(tail, locals)); continue; };
                match statement {
                    Statement::Let { name, declared, value, .. } => {
                        if declared.as_ref().is_some_and(|ty| !function_value_scalar_type(ty)) { return Err(reject("closure locals must be Copy scalars")); }
                        let prior = locals.clone(); locals.insert(name);
                        pending.push(Item::Block(statements, next + 1, tail, locals));
                        pending.push(Item::Expr(value, prior));
                    }
                    Statement::Assign { name, field, value, .. } => {
                        if field.is_some() || !locals.contains(name.as_str()) { return Err(reject("closure snapshots cannot be mutated")); }
                        pending.push(Item::Block(statements, next + 1, tail, locals.clone()));
                        pending.push(Item::Expr(value, locals));
                    }
                    Statement::While { condition, body, .. } => {
                        pending.push(Item::Block(statements, next + 1, tail, locals.clone()));
                        pending.push(Item::Expr(body, locals.clone())); pending.push(Item::Expr(condition, locals));
                    }
                    _ => return Err(reject("closure body contains an unsupported statement")),
                }
            }
            Item::Expr(expression, locals) => match &expression.kind {
                ExprKind::Int(_) | ExprKind::Int32(_) | ExprKind::Char(_) | ExprKind::Uint8(_) | ExprKind::Usize(_) | ExprKind::Float32(_) | ExprKind::Float64(_) | ExprKind::Bool(_) => {}
                ExprKind::Var(name) => {
                    if locals.contains(name.as_str()) { continue; }
                    if name == "result" || outer.get(name.as_str()).is_none_or(|ty| !function_value_scalar_type(ty)) { return Err(reject("closures capture only lexical Copy scalar values")); }
                    captures.insert(name.clone());
                    if captures.len() > 8 { return Err(reject("closure capture inventory exceeds eight scalar snapshots")); }
                }
                ExprKind::Unary { value, .. } => pending.push(Item::Expr(value, locals)),
                ExprKind::Binary { left, right, .. } => { pending.push(Item::Expr(right, locals.clone())); pending.push(Item::Expr(left, locals)); }
                ExprKind::If { condition, then_branch, else_branch } => {
                    pending.push(Item::Expr(else_branch, locals.clone())); pending.push(Item::Expr(then_branch, locals.clone())); pending.push(Item::Expr(condition, locals));
                }
                ExprKind::Block { statements, tail } => pending.push(Item::Block(statements, 0, tail, locals)),
                ExprKind::Call { name, type_arguments, args } => {
                    if locals.contains(name.as_str()) || outer.contains_key(name.as_str()) || !type_arguments.is_empty() || program.functions.iter().find(|f| f.name == *name).and_then(function_value_signature).is_none() {
                        return Err(reject("closure bodies call only ordinary local scalar functions"));
                    }
                    pending.extend(args.iter().rev().map(|arg| Item::Expr(arg, locals.clone())));
                }
                _ => return Err(reject("closure bodies admit scalar expressions only; nested closures and owning values are excluded")),
            }
        }
    }
    Ok(captures.into_iter().collect())
}

fn scalar_binding(ty: Type) -> Binding {
    Binding {
        ty,
        mode: ParamMode::Value,
        availability: Availability::Available,
        moved_places: HashMap::new(),
        definitely_partial: HashSet::new(),
        native_unit_discard: false,
        mutable: false,
        active_loans: BTreeSet::new(),
        borrow_origin: None,
    }
}

impl<'a, 'p> IterativeVerifier<'a, 'p> {
    pub(super) fn enter_closure(
        &mut self,
        expression: &'p Expr,
        scope: usize,
    ) -> Result<(), Diagnostic> {
        let ExprKind::Closure {
            params,
            return_type,
            body,
        } = &expression.kind
        else {
            unreachable!()
        };
        if !self.current.type_parameters.is_empty() {
            return Err(error(
                self.program,
                "SPX-T288",
                "anonymous closures inside generic templates are not admitted",
                expression.span,
            ));
        }
        let outer = self.scopes[scope]
            .bindings
            .iter()
            .map(|(name, binding)| (name.as_str(), &binding.ty))
            .collect();
        let names = capture_names(self.program, params, return_type, body, &outer)?;
        let mut bindings = HashMap::new();
        for name in names {
            let binding = &self.scopes[scope].bindings[&name];
            if binding.mode != ParamMode::Value || binding.availability != Availability::Available {
                return Err(error(
                    self.program,
                    "SPX-T288",
                    "closure capture is not an available scalar value",
                    expression.span,
                ));
            }
            bindings.insert(name, scalar_binding(binding.ty.clone()));
        }
        for param in params {
            bindings.insert(param.name.clone(), scalar_binding(param.ty.clone()));
        }
        let scope = self.scopes.len();
        self.scopes.push(VerifierScope {
            bindings,
            local_borrow_count: 0,
        });
        self.frames
            .push(VerifierFrame::ResumeClosure { expression, scope });
        self.frames.push(VerifierFrame::Enter {
            expression: body,
            scope,
        });
        Ok(())
    }
    pub(super) fn resume_closure(
        &mut self,
        expression: &'p Expr,
        scope: usize,
    ) -> Result<(), Diagnostic> {
        if scope + 1 != self.scopes.len() {
            return Err(Diagnostic::io(
                "SPX-H006",
                "closure verifier scope is not the active child",
            ));
        }
        self.scopes.pop();
        let ExprKind::Closure {
            params,
            return_type,
            ..
        } = &expression.kind
        else {
            unreachable!()
        };
        let value = self.values.pop().flatten();
        if value
            .as_ref()
            .is_some_and(|value| value.ty == *return_type && value.mode == ParamMode::Value)
        {
            self.values.push(Some(CheckedValue::value(Type::Function {
                parameters: params.iter().map(|p| p.ty.clone()).collect(),
                result: Box::new(return_type.clone()),
            })));
        } else {
            self.diagnostics.push(error(
                self.program,
                "SPX-T288",
                "closure result does not match its scalar signature",
                expression.span,
            ));
            self.values.push(None);
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(super) fn oracle(
    program: &Program,
    current: &crate::ast::Function,
    expression: &Expr,
    outer: &HashMap<String, Binding>,
    functions: &HashMap<&str, &crate::ast::Function>,
    types: &super::type_table::TypeTable<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CheckedValue> {
    let ExprKind::Closure {
        params,
        return_type,
        body,
    } = &expression.kind
    else {
        unreachable!()
    };
    if !current.type_parameters.is_empty() {
        diagnostics.push(error(
            program,
            "SPX-T288",
            "anonymous closures inside generic templates are not admitted",
            expression.span,
        ));
        return None;
    }
    let outer_types = outer
        .iter()
        .map(|(name, binding)| (name.as_str(), &binding.ty))
        .collect();
    let names = match capture_names(program, params, return_type, body, &outer_types) {
        Ok(names) => names,
        Err(error) => {
            diagnostics.push(error);
            return None;
        }
    };
    let mut bindings = HashMap::new();
    for name in names {
        let binding = &outer[&name];
        if binding.mode != ParamMode::Value || binding.availability != Availability::Available {
            diagnostics.push(error(
                program,
                "SPX-T288",
                "closure capture is not an available scalar value",
                expression.span,
            ));
            return None;
        }
        bindings.insert(name, scalar_binding(binding.ty.clone()));
    }
    for param in params {
        bindings.insert(param.name.clone(), scalar_binding(param.ty.clone()));
    }
    let value = super::oracle::check_expr(
        program,
        current,
        body,
        &mut bindings,
        functions,
        types,
        None,
        true,
        diagnostics,
    );
    if value
        .as_ref()
        .is_some_and(|value| value.ty == *return_type && value.mode == ParamMode::Value)
    {
        Some(CheckedValue::value(Type::Function {
            parameters: params.iter().map(|param| param.ty.clone()).collect(),
            result: Box::new(return_type.clone()),
        }))
    } else {
        diagnostics.push(error(
            program,
            "SPX-T288",
            "closure result does not match its scalar signature",
            expression.span,
        ));
        None
    }
}

pub(super) fn source_signature(params: &[ClosureParam], result: &Type) -> Type {
    Type::Function {
        parameters: params
            .iter()
            .map(|parameter| parameter.ty.clone())
            .collect(),
        result: Box::new(result.clone()),
    }
}
