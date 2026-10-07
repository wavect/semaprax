//! Record Invariants v1: routing every record production through its check.
//!
//! A record declared with `requires` clauses gets synthesized functions, both
//! ordinary source functions:
//!
//! ```text
//! fn Name#invariant(field: T, ...) -> bool
//!     requires <each clause, over the fields by bare name>
//! { true }
//! fn Name#check(value: Name) -> Name
//!     requires Name#invariant(value.field, ...)
//! { value }
//! ```
//!
//! Every place that produces a new value of such a record then calls the
//! check on that value: a record literal `Name { ... }` becomes
//! `Name#check(Name { ... })`, an update `base with { ... }` becomes
//! `Name#check(base with { ... })`, and a field assignment `t.f = v;` becomes
//! `t.f = { let #value = v; let #record = Name { f: #value, g: t.g, ... };
//! #value };`, whose literal is checked like any other. A violated clause is
//! therefore the ordinary contract failure of a failing precondition, with
//! its status, label, and cleanup exactly as for any other call, on every
//! backend that executes the program. Evaluation order is unchanged: literal
//! and update fields still evaluate in authored order, and the assigned value
//! evaluates before any field of the record is read.
//!
//! Only Copy records are routed through `Name#check`: a record holding an
//! owned `string` remains outside the executable record layout when it carries
//! invariants, so its clauses are carried by `Name#invariant` alone,
//! where the graph and every later lowering see them.
//!
//! The rewrite runs on a clone of the verified program after one resolution
//! of the original has supplied the record type of every update and field
//! assignment, keyed by source span. `#` cannot appear in a source
//! identifier, so the synthesized names never collide with user names. A
//! program without invariants is never cloned.

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::{
    Expr, ExprKind, FieldDeclaration, FieldInitializer, Function, Param, ParamMode, Program, Span,
    Statement, Type, TypeDeclarationKind,
};

use super::expr_nodes::{ResolvedExprKind, ResolvedStatement};
use super::nodes::{ResolvedProgram, ResolvedType};

/// The locals of a rewritten field assignment: the assigned value, and the
/// record it would produce, checked before the field is written.
const ASSIGNED: &str = "#value";
const RECORD: &str = "#record";

/// A clone of `program` whose record productions call their invariant
/// checks, or `None` when no record declares an invariant.
pub(super) fn desugar(program: &Program, resolved: &ResolvedProgram) -> Option<Program> {
    let declared = program
        .types
        .iter()
        .filter(|declaration| !declaration.invariants().is_empty())
        .filter_map(|declaration| match &declaration.kind {
            TypeDeclarationKind::Record { fields } => Some((declaration, fields.as_slice())),
            _ => None,
        })
        .collect::<Vec<_>>();
    if declared.is_empty() {
        return None;
    }
    let checked = declared
        .iter()
        .filter(|(declaration, _)| {
            let ty = ResolvedType::Nominal {
                declaration: super::DeclarationId::new(declaration.stable_id.clone()),
                arguments: Vec::new(),
            };
            resolved
                .declarations
                .type_facts(&ty)
                .is_some_and(|facts| facts.copy)
        })
        .map(|(declaration, fields)| {
            (
                declaration.stable_id.as_str(),
                (declaration.name.clone(), *fields),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let sites = Sites::collect(resolved, &checked);
    let mut rewritten = program.clone();
    let mut roots = Vec::new();
    for function in &mut rewritten.functions {
        roots.push(&mut function.body);
        roots.extend(function.requires.iter_mut());
        roots.extend(function.ensures.iter_mut());
    }
    for declaration in &mut rewritten.types {
        if let TypeDeclarationKind::Class { methods, .. } = &mut declaration.kind {
            for method in methods {
                roots.push(&mut method.body);
                roots.extend(method.requires.iter_mut());
                roots.extend(method.ensures.iter_mut());
            }
        }
    }
    rewrite(roots, &sites);
    for (declaration, fields) in declared {
        rewritten
            .functions
            .push(invariant_function(declaration, fields));
        if checked.contains_key(declaration.stable_id.as_str()) {
            rewritten
                .functions
                .push(check_function(declaration, fields));
        }
    }
    Some(rewritten)
}

/// Whether `name` is a synthesized invariant function: `#` never occurs in a
/// source identifier.
pub(crate) fn is_synthesized(name: &str) -> bool {
    name.ends_with("#check") || name.ends_with("#invariant")
}

/// The synthesized check name for record `name`.
pub(crate) fn check_name(name: &str) -> String {
    format!("{name}#check")
}

fn invariant_name(name: &str) -> String {
    format!("{name}#invariant")
}

/// Every record production of a checked record. Literals name their record;
/// updates map their source span to the record's name, and field assignments
/// map theirs to the record, its declared field names, and the assigned one.
struct Sites {
    records: BTreeSet<String>,
    productions: BTreeMap<(usize, usize), String>,
    assignments: BTreeMap<(usize, usize), Assignment>,
}

struct Assignment {
    record: String,
    fields: Vec<String>,
    assigned: String,
}

impl Sites {
    fn collect(
        resolved: &ResolvedProgram,
        checked: &BTreeMap<&str, (String, &[FieldDeclaration])>,
    ) -> Self {
        let mut sites = Self {
            records: checked.values().map(|(name, _)| name.clone()).collect(),
            productions: BTreeMap::new(),
            assignments: BTreeMap::new(),
        };
        let checked_name = |ty: &ResolvedType| match ty {
            ResolvedType::Nominal {
                declaration,
                arguments,
            } if arguments.is_empty() => checked
                .get(declaration.as_str())
                .map(|(name, fields)| (name.clone(), *fields)),
            _ => None,
        };
        let mut pending = Vec::new();
        for function in resolved.functions.iter().chain(
            resolved
                .function_instances
                .iter()
                .map(|item| &item.function),
        ) {
            pending.push(&function.body);
            pending.extend(function.requires.iter().chain(&function.ensures));
        }
        while let Some(expression) = pending.pop() {
            let key = (expression.span.start, expression.span.end);
            match &expression.kind {
                ResolvedExprKind::UpdateRecord { .. } => {
                    if let Some((name, _)) = checked_name(&expression.ty) {
                        sites.productions.insert(key, name);
                    }
                }
                ResolvedExprKind::Block { statements, .. } => {
                    for statement in statements {
                        let ResolvedStatement::Assign {
                            binding,
                            field: Some(field),
                            span,
                            ..
                        } = statement
                        else {
                            continue;
                        };
                        let Some((name, fields)) = checked_name(&binding.ty) else {
                            continue;
                        };
                        if let Some(declared) = fields
                            .iter()
                            .find(|declared| declared.stable_id == field.as_str())
                        {
                            sites.assignments.insert(
                                (span.start, span.end),
                                Assignment {
                                    record: name,
                                    fields: fields.iter().map(|field| field.name.clone()).collect(),
                                    assigned: declared.name.clone(),
                                },
                            );
                        }
                    }
                }
                _ => {}
            }
            if let ResolvedExprKind::Closure { body, .. } = &expression.kind {
                pending.push(body);
            }
            super::push_resolved_expression_children_in_authored_order(expression, &mut pending);
        }
        sites
    }
}

fn expr(kind: ExprKind, span: Span) -> Expr {
    Expr { kind, span }
}

fn call(name: String, args: Vec<Expr>, span: Span) -> Expr {
    expr(
        ExprKind::Call {
            name,
            type_arguments: Vec::new(),
            args,
        },
        span,
    )
}

fn project(base: Expr, field: &str, span: Span) -> Expr {
    expr(
        ExprKind::Project {
            base: Box::new(base),
            field: field.to_owned(),
            field_span: span,
        },
        span,
    )
}

/// Rewrites every production site below `roots`, iteratively.
fn rewrite(roots: Vec<&mut Expr>, sites: &Sites) {
    let mut pending = roots;
    while let Some(expression) = pending.pop() {
        let key = (expression.span.start, expression.span.end);
        let produced = match &expression.kind {
            ExprKind::ConstructRecord {
                type_name,
                type_arguments,
                ..
            } if type_arguments.is_empty() && sites.records.contains(type_name) => {
                Some(type_name.clone())
            }
            ExprKind::UpdateRecord { .. } => sites.productions.get(&key).cloned(),
            _ => None,
        };
        {
            if let Some(name) = produced {
                let span = expression.span;
                let original = std::mem::replace(expression, expr(ExprKind::Bool(true), span));
                *expression = call(check_name(&name), vec![original], span);
                let ExprKind::Call { args, .. } = &mut expression.kind else {
                    unreachable!("the check call was just built");
                };
                push_children(&mut args[0], &mut pending);
                continue;
            }
        }
        if let ExprKind::Block { statements, .. } = &mut expression.kind {
            for statement in statements.iter_mut() {
                rewrite_assignment(statement, sites);
            }
        }
        push_children(expression, &mut pending);
    }
}

/// `t.f = v;` becomes
/// `t.f = { let #value = v; let #record = Name { ..., f: #value, ... }; #value };`.
/// The literal is pushed back for rewriting, which wraps it in its check.
fn rewrite_assignment(statement: &mut Statement, sites: &Sites) {
    let Statement::Assign {
        name: binding,
        name_span,
        field: Some(_),
        value,
        span,
    } = statement
    else {
        return;
    };
    let Some(assignment) = sites.assignments.get(&(span.start, span.end)) else {
        return;
    };
    let span = value.span;
    let assigned = std::mem::replace(value, expr(ExprKind::Bool(true), span));
    let fields = assignment
        .fields
        .iter()
        .map(|field| FieldInitializer {
            name: field.clone(),
            name_span: span,
            value: if *field == assignment.assigned {
                expr(ExprKind::Var(ASSIGNED.to_owned()), span)
            } else {
                project(
                    expr(ExprKind::Var(binding.clone()), *name_span),
                    field,
                    span,
                )
            },
            span,
        })
        .collect();
    let record = expr(
        ExprKind::ConstructRecord {
            type_name: assignment.record.clone(),
            type_span: span,
            type_arguments: Vec::new(),
            fields,
        },
        span,
    );
    let local = |name: &str, value: Expr| Statement::Let {
        syntax: crate::ast::LetSyntax::Authored,
        name: name.to_owned(),
        name_span: span,
        mutable: false,
        declared: None,
        value,
        span,
    };
    *value = expr(
        ExprKind::Block {
            statements: vec![local(ASSIGNED, assigned), local(RECORD, record)],
            tail: Box::new(expr(ExprKind::Var(ASSIGNED.to_owned()), span)),
        },
        span,
    );
}

fn push_children<'e>(expression: &'e mut Expr, pending: &mut Vec<&'e mut Expr>) {
    match &mut expression.kind {
        ExprKind::Closure { body, .. } => pending.push(body),
        ExprKind::Call { args, .. } | ExprKind::SuperMethod { args, .. } => {
            pending.extend(args.iter_mut())
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            pending.push(receiver);
            pending.extend(args.iter_mut());
        }
        ExprKind::Unary { value, .. }
        | ExprKind::Try { operand: value }
        | ExprKind::Project { base: value, .. }
        | ExprKind::Yield { request: value } => pending.push(value),
        ExprKind::Binary { left, right, .. } => {
            pending.push(left);
            pending.push(right);
        }
        ExprKind::Block { statements, tail } => {
            for statement in statements {
                match statement {
                    Statement::Let { value, .. } | Statement::Assign { value, .. } => {
                        pending.push(value)
                    }
                    Statement::Unsafe { body, .. } => pending.push(body),
                    Statement::While {
                        condition, body, ..
                    } => {
                        pending.push(condition);
                        pending.push(body);
                    }
                    Statement::For { values, body, .. }
                    | Statement::ForOwn { values, body, .. } => {
                        pending.push(values);
                        pending.push(body);
                    }
                }
            }
            pending.push(tail);
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            pending.push(condition);
            pending.push(then_branch);
            pending.push(else_branch);
        }
        ExprKind::ConstructRecord { fields, .. } | ExprKind::ConstructVariant { fields, .. } => {
            pending.extend(fields.iter_mut().map(|field| &mut field.value))
        }
        ExprKind::UpdateRecord { base, fields } => {
            pending.push(base);
            pending.extend(fields.iter_mut().map(|field| &mut field.value));
        }
        ExprKind::Match {
            scrutinee, arms, ..
        } => {
            pending.push(scrutinee);
            for arm in arms {
                if let Some(guard) = &mut arm.guard {
                    pending.push(guard);
                }
                pending.push(&mut arm.value);
            }
        }
        ExprKind::Int(_)
        | ExprKind::Int32(_)
        | ExprKind::Char(_)
        | ExprKind::Uint8(_)
        | ExprKind::Usize(_)
        | ExprKind::ArrayU8(_)
        | ExprKind::RepeatArrayU8 { .. }
        | ExprKind::Float32(_)
        | ExprKind::Float64(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::Var(_) => {}
    }
}

fn synthesized(
    declaration: &crate::ast::TypeDeclaration,
    name: String,
    suffix: &str,
    params: Vec<Param>,
    return_type: Type,
    requires: Vec<Expr>,
    body: ExprKind,
) -> Function {
    Function {
        stable_id: format!("{}{suffix}", declaration.stable_id),
        explicit_id: declaration.explicit_id,
        name,
        name_span: declaration.name_span,
        type_parameters: Vec::new(),
        params,
        return_type,
        effects: Vec::new(),
        yields: None,
        follows: None,
        requires,
        ensures: Vec::new(),
        body: expr(body, declaration.span),
        span: declaration.span,
    }
}

fn invariant_function(
    declaration: &crate::ast::TypeDeclaration,
    fields: &[FieldDeclaration],
) -> Function {
    synthesized(
        declaration,
        invariant_name(&declaration.name),
        "#invariant",
        fields
            .iter()
            .map(|field| Param {
                name: field.name.clone(),
                mode: ParamMode::Value,
                ty: field.ty.clone(),
                span: field.name_span,
            })
            .collect(),
        Type::Bool,
        declaration.invariants().to_vec(),
        ExprKind::Bool(true),
    )
}

fn check_function(
    declaration: &crate::ast::TypeDeclaration,
    fields: &[FieldDeclaration],
) -> Function {
    let span = declaration.span;
    let value = || expr(ExprKind::Var("value".to_owned()), span);
    let record = Type::Named {
        name: declaration.name.clone(),
        arguments: Vec::new(),
    };
    synthesized(
        declaration,
        check_name(&declaration.name),
        "#check",
        vec![Param {
            name: "value".to_owned(),
            mode: ParamMode::Value,
            ty: record.clone(),
            span: declaration.name_span,
        }],
        record,
        vec![call(
            invariant_name(&declaration.name),
            fields
                .iter()
                .map(|field| project(value(), &field.name, field.name_span))
                .collect(),
            span,
        )],
        ExprKind::Var("value".to_owned()),
    )
}
