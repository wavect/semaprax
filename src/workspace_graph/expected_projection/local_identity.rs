//! Conservative scalar-local facts for the raw-AST accounting fallback.
//! Each discounted Place retains both its expression and binding identity.

use super::call_identity::{complete_pattern, primitive, roots, scalar_callee, walk};
use crate::ast::{Expr, ExprKind, Function, Program, Statement};
use std::collections::{BTreeMap, BTreeSet};

const MAX_NAMES: usize = 4096;

pub(super) fn discount<'a>(
    function: &'a Function,
    program: &Program,
    parameters: &BTreeSet<&str>,
    declared: &BTreeMap<&str, Option<usize>>,
    shadowed: &BTreeSet<&str>,
) -> usize {
    let mut counts = BTreeMap::<&str, u8>::new();
    let mut blocked: BTreeSet<&str> = function.params.iter().map(|p| p.name.as_str()).collect();
    for root in roots(function) {
        if !walk(root, &mut |expression| {
            match &expression.kind {
                ExprKind::Block { statements, .. } => {
                    for statement in statements {
                        match statement {
                            Statement::Let { name, .. } => {
                                let count = counts.entry(name).or_default();
                                *count = count.saturating_add(1);
                            }
                            Statement::For { item, .. } | Statement::ForOwn { item, .. } => {
                                blocked.insert(item);
                            }
                            _ => {}
                        }
                    }
                }
                ExprKind::Closure { params, .. } => {
                    for param in params {
                        blocked.insert(&param.name);
                    }
                }
                ExprKind::Match { arms, .. } => {
                    for arm in arms {
                        if !complete_pattern(&arm.pattern, &mut blocked, 0) {
                            return false;
                        }
                    }
                }
                _ => {}
            }
            counts.len() <= MAX_NAMES && blocked.len() <= MAX_NAMES
        }) {
            return 0;
        }
    }
    let unique = counts
        .into_iter()
        .filter_map(|(name, count)| (count == 1 && !blocked.contains(name)).then_some(name))
        .collect();
    let context = Context {
        unique,
        program,
        parameters,
        declared,
        shadowed,
    };
    let mut total = 0;
    for root in roots(function) {
        let mut active = BTreeSet::new();
        let Some(count) = visit(root, &context, &mut active, 0) else {
            return 0;
        };
        total += count;
    }
    total
}

struct Context<'a, 'b> {
    unique: BTreeSet<&'a str>,
    program: &'b Program,
    parameters: &'b BTreeSet<&'b str>,
    declared: &'b BTreeMap<&'b str, Option<usize>>,
    shadowed: &'b BTreeSet<&'b str>,
}

fn initializer(expression: &Expr, context: &Context<'_, '_>, active: &BTreeSet<&str>) -> bool {
    match &expression.kind {
        ExprKind::Var(name) => {
            active.contains(name.as_str()) || context.parameters.contains(name.as_str())
        }
        ExprKind::Call {
            name,
            args,
            type_arguments,
        } => {
            type_arguments.is_empty()
                && !context.shadowed.contains(name.as_str())
                && !context
                    .program
                    .interfaces
                    .iter()
                    .any(|interface| interface.imports.iter().any(|import| import.name == *name))
                && scalar_callee(name, args.len(), context.declared)
        }
        kind => super::identity_slots::scalar_expression_identity_discount(kind) != 0,
    }
}

/// Declaration-order traversal, not inferred global name typing. A candidate
/// enters only after its initializer and exits with its declaring Block. Unique
/// names exclude parameter/pattern/loop/closure collisions throughout the
/// function. Checked assignments preserve the binding's established type.
fn visit<'a>(
    expression: &'a Expr,
    context: &Context<'_, '_>,
    active: &mut BTreeSet<&'a str>,
    depth: usize,
) -> Option<usize> {
    if depth >= 128 {
        return None;
    }
    let mut count = usize::from(
        matches!(&expression.kind, ExprKind::Var(name) if active.contains(name.as_str())),
    );
    if let ExprKind::Block { statements, tail } = &expression.kind {
        let mut added = Vec::new();
        for statement in statements {
            for index in 0..statement.child_count() {
                count += visit(statement.child(index)?, context, active, depth + 1)?;
            }
            if let Statement::Let {
                name,
                declared,
                value,
                ..
            } = statement
            {
                if context.unique.contains(name.as_str())
                    && declared
                        .as_ref()
                        .map_or_else(|| initializer(value, context, active), primitive)
                {
                    if active.len() == MAX_NAMES {
                        return None;
                    }
                    active.insert(name);
                    added.push(name.as_str());
                }
            }
        }
        count += visit(tail, context, active, depth + 1)?;
        for name in added {
            active.remove(name);
        }
    } else {
        let mut index = 0;
        while let Some(child) = expression.child(index) {
            count += visit(child, context, active, depth + 1)?;
            index += 1;
        }
    }
    Some(count)
}
