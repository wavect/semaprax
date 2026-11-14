use super::names::Names;
use super::{complete_pattern, primitive, roots, scalar_callee, walk, Declared};
use crate::ast::{Expr, ExprKind, Function, Program, Statement};

const MAX_NAMES: usize = 4096;

pub(super) fn discount(
    function: &Function,
    program: &Program,
    parameters: &Names<'_, ()>,
    declared: &Declared<'_, '_>,
    shadowed: &Names<'_, ()>,
) -> Option<usize> {
    let mut counts = Names::<Binding>::default();
    let mut blocked = Names::<()>::default();
    for parameter in &function.params {
        blocked.entry(&parameter.name)?;
    }
    let mut refused = false;
    for root in roots(function) {
        if !walk(root, &mut |expression| {
            match &expression.kind {
                ExprKind::Block { statements, .. } => {
                    for statement in statements {
                        match statement {
                            Statement::Let { name, .. } => {
                                let Some(count) = counts.entry(name) else {
                                    refused = true;
                                    return false;
                                };
                                count.declarations = count.declarations.saturating_add(1);
                            }
                            Statement::For { item, .. }
                            | Statement::ForBorrow { item, .. }
                            | Statement::ForOwn { item, .. } => {
                                if blocked.entry(item).is_none() {
                                    refused = true;
                                    return false;
                                }
                                if let Statement::ForBorrow {
                                    index: Some(index), ..
                                } = statement
                                {
                                    if blocked.entry(&index.name).is_none() {
                                        refused = true;
                                        return false;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                ExprKind::Closure { params, .. } => {
                    for param in params {
                        if blocked.entry(&param.name).is_none() {
                            refused = true;
                            return false;
                        }
                    }
                }
                ExprKind::Match { arms, .. } => {
                    for arm in arms {
                        match complete_pattern(&arm.pattern, &mut blocked, 0) {
                            Some(true) => {}
                            Some(false) => return false,
                            None => {
                                refused = true;
                                return false;
                            }
                        }
                    }
                }
                _ => {}
            }
            counts.len() <= MAX_NAMES && blocked.len() <= MAX_NAMES
        }) {
            return if refused { None } else { Some(0) };
        }
    }
    let mut context = Context {
        counts,
        blocked,
        program,
        parameters,
        declared,
        shadowed,
    };
    let mut total = 0usize;
    for root in roots(function) {
        let Some(count) = visit(root, &mut context, 0, 0)? else {
            return Some(0);
        };
        total = total.checked_add(count)?;
    }
    Some(total)
}

#[derive(Default)]
struct Binding {
    declarations: u8,
    active: bool,
}

struct Context<'a, 'b, 'program, 'index> {
    counts: Names<'a, Binding>,
    blocked: Names<'a, ()>,
    program: &'b Program,
    parameters: &'b Names<'b, ()>,
    declared: &'b Declared<'program, 'index>,
    shadowed: &'b Names<'b, ()>,
}

fn initializer(expression: &Expr, context: &Context<'_, '_, '_, '_>) -> Option<bool> {
    match &expression.kind {
        ExprKind::Var(name) => Some(
            context
                .counts
                .get(name.as_str())
                .is_some_and(|binding| binding.active)
                || context.parameters.contains(name.as_str()),
        ),
        ExprKind::Call {
            name,
            args,
            type_arguments,
        } => {
            if !(type_arguments.is_empty()
                && !context.shadowed.contains(name.as_str())
                && !context
                    .program
                    .interfaces
                    .iter()
                    .any(|interface| interface.imports.iter().any(|import| import.name == *name)))
            {
                return Some(false);
            }
            scalar_callee(name, args.len(), context.declared)
        }
        kind => Some(super::super::identity_slots::scalar_expression_identity_discount(kind) != 0),
    }
}

/// Declaration-order traversal, not inferred global name typing. A candidate
/// enters only after its initializer and exits with its declaring Block. Unique
/// names exclude parameter/pattern/loop/closure collisions throughout the
/// function. Checked assignments preserve the binding's established type.
fn visit(
    expression: &Expr,
    context: &mut Context<'_, '_, '_, '_>,
    mut active_count: usize,
    depth: usize,
) -> Option<Option<usize>> {
    if depth >= 128 {
        return Some(None);
    }
    let mut count = usize::from(
        matches!(&expression.kind, ExprKind::Var(name) if context.counts.get(name.as_str()).is_some_and(|binding| binding.active)),
    );
    if let ExprKind::Block { statements, tail } = &expression.kind {
        for statement in statements {
            for index in 0..statement.child_count() {
                let Some(child) = statement.child(index) else {
                    return Some(None);
                };
                let Some(child_count) = visit(child, context, active_count, depth + 1)? else {
                    return Some(None);
                };
                count = count.checked_add(child_count)?;
            }
            if let Statement::Let {
                name,
                declared,
                value,
                ..
            } = statement
            {
                if context
                    .counts
                    .get(name.as_str())
                    .is_some_and(|binding| binding.declarations == 1)
                    && !context.blocked.contains(name.as_str())
                    && declared
                        .as_ref()
                        .map_or_else(|| initializer(value, context), |ty| Some(primitive(ty)))?
                {
                    if active_count == MAX_NAMES {
                        return Some(None);
                    }
                    context.counts.get_mut(name.as_str()).unwrap().active = true;
                    active_count += 1;
                }
            }
        }
        let Some(tail_count) = visit(tail, context, active_count, depth + 1)? else {
            return Some(None);
        };
        count = count.checked_add(tail_count)?;
        for statement in statements {
            if let Statement::Let { name, .. } = statement {
                context.counts.get_mut(name.as_str()).unwrap().active = false;
            }
        }
    } else {
        let mut index = 0;
        while let Some(child) = expression.child(index) {
            let Some(child_count) = visit(child, context, active_count, depth + 1)? else {
                return Some(None);
            };
            count = count.checked_add(child_count)?;
            index += 1;
        }
    }
    Some(Some(count))
}
