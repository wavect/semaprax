//! Bounded exhaustive AST traversal shared by candidate intentions.

use crate::ast::{Expr, ExprKind, Function, Program, Statement, TypeDeclarationKind};

use super::{capacity, grammar, literal_nodes, Result, MAX_WALK_DEPTH, MAX_WALK_NODES};

pub(in crate::project::candidate) fn program(
    program: &mut Program,
    nodes: &mut usize,
    visit: &mut impl FnMut(&mut Expr) -> Result<()>,
) -> Result<()> {
    for item in &mut program.functions {
        function(item, nodes, visit)?;
    }
    for declaration in &mut program.types {
        if let TypeDeclarationKind::Class { methods, .. } = &mut declaration.kind {
            for method in methods {
                function(method, nodes, visit)?;
            }
        }
    }
    Ok(())
}

pub(in crate::project::candidate) fn function(
    function: &mut Function,
    nodes: &mut usize,
    visit: &mut impl FnMut(&mut Expr) -> Result<()>,
) -> Result<()> {
    for item in function
        .requires
        .iter_mut()
        .chain(function.ensures.iter_mut())
        .chain(std::iter::once(&mut function.body))
    {
        expression(item, 0, nodes, visit)?;
    }
    Ok(())
}

/// Exhaustive child traversal; generic bodies, contracts, guards and loops are
/// included. Unknown future AST variants cause a compiler error, not omission.
fn expression(
    expression: &mut Expr,
    depth: usize,
    nodes: &mut usize,
    visit: &mut impl FnMut(&mut Expr) -> Result<()>,
) -> Result<()> {
    *nodes += 1;
    if depth > MAX_WALK_DEPTH || *nodes > MAX_WALK_NODES {
        return Err(capacity(
            "candidate call migration exceeds its traversal bound",
        ));
    }
    let next = depth + 1;
    match &mut expression.kind {
        ExprKind::Call { args, .. } | ExprKind::SuperMethod { args, .. } => {
            for arg in args {
                self::expression(arg, next, nodes, visit)?;
            }
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            self::expression(receiver, next, nodes, visit)?;
            for arg in args {
                self::expression(arg, next, nodes, visit)?;
            }
        }
        ExprKind::Unary { value, .. }
        | ExprKind::Try { operand: value }
        | ExprKind::Project { base: value, .. } => self::expression(value, next, nodes, visit)?,
        ExprKind::Binary { left, right, .. } => {
            self::expression(left, next, nodes, visit)?;
            self::expression(right, next, nodes, visit)?;
        }
        ExprKind::Block { statements, tail } => {
            for statement in statements {
                match statement {
                    Statement::Let { value, .. } | Statement::Assign { value, .. } => {
                        self::expression(value, next, nodes, visit)?
                    }
                    Statement::Unsafe { body, .. } => self::expression(body, next, nodes, visit)?,
                    Statement::While {
                        condition, body, ..
                    } => {
                        self::expression(condition, next, nodes, visit)?;
                        self::expression(body, next, nodes, visit)?;
                    }
                }
            }
            self::expression(tail, next, nodes, visit)?;
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            self::expression(condition, next, nodes, visit)?;
            self::expression(then_branch, next, nodes, visit)?;
            self::expression(else_branch, next, nodes, visit)?;
        }
        ExprKind::ConstructRecord { fields, .. } | ExprKind::ConstructVariant { fields, .. } => {
            for field in fields {
                self::expression(&mut field.value, next, nodes, visit)?;
            }
        }
        ExprKind::Match {
            scrutinee, arms, ..
        } => {
            self::expression(scrutinee, next, nodes, visit)?;
            for arm in arms {
                if let Some(guard) = &mut arm.guard {
                    self::expression(guard, next, nodes, visit)?;
                }
                self::expression(&mut arm.value, next, nodes, visit)?;
            }
        }
        ExprKind::UpdateRecord { base, fields } => {
            self::expression(base, next, nodes, visit)?;
            for field in fields {
                self::expression(&mut field.value, next, nodes, visit)?;
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
    let previous_arity = match &expression.kind {
        ExprKind::Call { args, .. } => args.len(),
        _ => 0,
    };
    visit(expression)?;
    if let ExprKind::Call { args, .. } = &expression.kind {
        // The sole growth admitted by migration is direct literal arguments;
        // charge them even though traversal deliberately did not revisit them.
        let added = args.get(previous_arity..).ok_or_else(|| {
            grammar("candidate migration unexpectedly reduced a caller argument inventory")
        })?;
        *nodes += added.iter().map(literal_nodes).sum::<usize>();
        if *nodes > MAX_WALK_NODES {
            return Err(capacity(
                "candidate migrated arguments exceed the node bound",
            ));
        }
    }
    Ok(())
}
