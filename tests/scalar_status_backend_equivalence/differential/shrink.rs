//! Structure-preserving shrinking.
//!
//! The shrinker works on the generated structure, not on source text, so every
//! candidate it proposes is still a typed module the compiler can admit. It
//! removes whole cases and helpers, drops statements and contract clauses,
//! collapses an expression to a neutral literal or to one of its own operands,
//! and reduces a loop bound — and it keeps a candidate only when the caller's
//! predicate says the discrepancy still reproduces.
//!
//! The predicate budget is fixed, so a minimization can never run unbounded.

use super::grammar::{Expr, Function, Module, Statement};

/// The number of predicate evaluations one minimization may spend. A shrink
/// step is as expensive as one full differential run, so this is the knob that
/// keeps a failing seed's report cheap enough for PR CI.
pub(crate) const DEFAULT_BUDGET: usize = 300;

pub(crate) struct Minimization {
    pub(crate) module: Module,
    pub(crate) steps: usize,
    pub(crate) predicate_calls: usize,
    pub(crate) budget_exhausted: bool,
}

/// Greedily minimize `module` while `reproduces` keeps holding.
///
/// `reproduces` must be a total predicate: a candidate that no longer parses,
/// verifies, or disagrees simply returns `false` and is discarded, so the
/// shrinker never needs to model the language's scoping rules itself.
pub(crate) fn minimize(
    module: &Module,
    budget: usize,
    mut reproduces: impl FnMut(&Module) -> bool,
) -> Minimization {
    let mut current = module.clone();
    let mut steps = 0;
    let mut calls = 0;
    let mut exhausted = false;
    'outer: loop {
        for candidate in candidates(&current) {
            if calls >= budget {
                exhausted = true;
                break 'outer;
            }
            calls += 1;
            if reproduces(&candidate) {
                current = candidate;
                steps += 1;
                continue 'outer;
            }
        }
        break;
    }
    Minimization {
        module: current,
        steps,
        predicate_calls: calls,
        budget_exhausted: exhausted,
    }
}

/// Every strictly smaller candidate, largest reduction first: whole cases, then
/// unreferenced helpers, then statements, contracts, loop bounds, and finally
/// individual expressions.
fn candidates(module: &Module) -> Vec<Module> {
    let mut produced = Vec::new();
    if module.cases.len() > 1 {
        for index in 0..module.cases.len() {
            let mut candidate = module.clone();
            candidate.cases.remove(index);
            produced.push(candidate);
        }
    }
    for index in 0..module.helpers.len() {
        let removed = module.helpers[index].index;
        // A helper only ever calls strictly lower-indexed helpers, so it never
        // references itself and every other function is a possible caller.
        let referenced = module
            .helpers
            .iter()
            .enumerate()
            .filter(|(position, _)| *position != index)
            .map(|(_, helper)| helper)
            .chain(module.cases.iter())
            .any(|function| function.calls().contains(&removed));
        if referenced {
            continue;
        }
        let mut candidate = module.clone();
        candidate.helpers.remove(index);
        produced.push(candidate);
    }
    for (position, function) in functions(module).enumerate() {
        for statement in 0..function.body.len() {
            let mut candidate = module.clone();
            function_at_mut(&mut candidate, position)
                .body
                .remove(statement);
            produced.push(candidate);
        }
        if function.requires.is_some() {
            let mut candidate = module.clone();
            function_at_mut(&mut candidate, position).requires = None;
            produced.push(candidate);
        }
        if function.ensures.is_some() {
            let mut candidate = module.clone();
            function_at_mut(&mut candidate, position).ensures = None;
            produced.push(candidate);
        }
        for (statement, item) in function.body.iter().enumerate() {
            if let Statement::While { bound, .. } = item {
                if *bound > 1 {
                    let mut candidate = module.clone();
                    if let Statement::While { bound, .. } =
                        &mut function_at_mut(&mut candidate, position).body[statement]
                    {
                        *bound = 1;
                    }
                    produced.push(candidate);
                }
            }
        }
    }
    for index in 0..expression_count(module) {
        let Some(expression) = nth_expression(module, index).cloned() else {
            continue;
        };
        for replacement in expression.simplifications() {
            let mut candidate = module.clone();
            if rewrite_nth(&mut candidate, index, &replacement) {
                produced.push(candidate);
            }
        }
    }
    produced
}

fn functions(module: &Module) -> impl Iterator<Item = &Function> {
    module.helpers.iter().chain(module.cases.iter())
}

fn function_at_mut(module: &mut Module, position: usize) -> &mut Function {
    let helpers = module.helpers.len();
    if position < helpers {
        &mut module.helpers[position]
    } else {
        &mut module.cases[position - helpers]
    }
}

/// Expression roots of one function, in the fixed order both traversals use.
fn roots(function: &Function) -> Vec<&Expr> {
    let mut found: Vec<&Expr> = Vec::new();
    if let Some(clause) = &function.requires {
        found.push(clause);
    }
    if let Some(clause) = &function.ensures {
        found.push(clause);
    }
    for statement in &function.body {
        found.extend(statement.expressions());
    }
    found.push(&function.tail);
    found
}

fn roots_mut(function: &mut Function) -> Vec<&mut Expr> {
    let mut found: Vec<&mut Expr> = Vec::new();
    if let Some(clause) = &mut function.requires {
        found.push(clause);
    }
    if let Some(clause) = &mut function.ensures {
        found.push(clause);
    }
    for statement in &mut function.body {
        found.extend(statement.expressions_mut());
    }
    found.push(&mut function.tail);
    found
}

fn expression_count(module: &Module) -> usize {
    let mut total = 0;
    for function in functions(module) {
        for root in roots(function) {
            total += subtree_size(root);
        }
    }
    total
}

fn subtree_size(expr: &Expr) -> usize {
    1 + expr
        .children()
        .iter()
        .map(|child| subtree_size(child))
        .sum::<usize>()
}

fn nth_expression(module: &Module, target: usize) -> Option<&Expr> {
    let mut counter = 0;
    for function in functions(module) {
        for root in roots(function) {
            if let Some(found) = find_in(root, &mut counter, target) {
                return Some(found);
            }
        }
    }
    None
}

fn find_in<'a>(expr: &'a Expr, counter: &mut usize, target: usize) -> Option<&'a Expr> {
    if *counter == target {
        return Some(expr);
    }
    *counter += 1;
    for child in expr.children() {
        if let Some(found) = find_in(child, counter, target) {
            return Some(found);
        }
    }
    None
}

fn rewrite_nth(module: &mut Module, target: usize, replacement: &Expr) -> bool {
    let mut counter = 0;
    let positions = module.helpers.len() + module.cases.len();
    for position in 0..positions {
        let function = function_at_mut(module, position);
        for root in roots_mut(function) {
            if rewrite_in(root, &mut counter, target, replacement) {
                return true;
            }
        }
    }
    false
}

fn rewrite_in(expr: &mut Expr, counter: &mut usize, target: usize, replacement: &Expr) -> bool {
    if *counter == target {
        *expr = replacement.clone();
        return true;
    }
    *counter += 1;
    for child in expr.children_mut() {
        if rewrite_in(child, counter, target, replacement) {
            return true;
        }
    }
    false
}
