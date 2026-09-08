//! Lexical call inventory for Function Value v1. `ExprKind::Call` carries a
//! spelling only, so inventory tracks every lexical binding before admitting a
//! global edge. Known references retain their exact target; an incoming or
//! dynamically selected callback conservatively ranges over its signature.

use super::declared_type::function_value_signature;
use crate::ast::{
    Expr, ExprKind, Function, MatchPattern, Program, RecordMatchFieldPattern, Statement, Type,
};
use std::collections::{BTreeSet, HashMap};

#[derive(Clone)]
enum Binding {
    Shadow,
    Callable {
        signature: Type,
        targets: Option<BTreeSet<String>>,
    },
}

type Scope<'a> = HashMap<&'a str, Binding>;

pub(crate) fn function_value_targets(
    program: &Program,
    functions: &HashMap<&str, &Function>,
) -> BTreeSet<String> {
    let mut targets = BTreeSet::new();
    for function in &program.functions {
        let scope = initial_scope(function);
        for expression in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            visit(
                expression,
                &mut scope.clone(),
                functions,
                &mut targets,
                None,
            );
        }
    }
    targets
}

pub(crate) fn calls(
    function: &Function,
    functions: &HashMap<&str, &Function>,
    universe: &BTreeSet<String>,
) -> Vec<String> {
    let scope = initial_scope(function);
    let mut output = BTreeSet::new();
    for expression in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        visit(
            expression,
            &mut scope.clone(),
            functions,
            &mut output,
            Some(universe),
        );
    }
    output.into_iter().collect()
}

fn initial_scope(function: &Function) -> Scope<'_> {
    function
        .params
        .iter()
        .map(|parameter| {
            (
                parameter.name.as_str(),
                match &parameter.ty {
                    signature @ Type::Function { .. } => Binding::Callable {
                        signature: signature.clone(),
                        targets: None,
                    },
                    _ => Binding::Shadow,
                },
            )
        })
        .collect()
}

fn visit(
    expression: &Expr,
    scope: &mut Scope<'_>,
    functions: &HashMap<&str, &Function>,
    output: &mut BTreeSet<String>,
    universe: Option<&BTreeSet<String>>,
) {
    match &expression.kind {
        ExprKind::Var(name) => {
            if !scope.contains_key(name.as_str())
                && function_value_signature_for(name, functions).is_some()
            {
                output.insert(name.clone());
            }
        }
        ExprKind::Call { name, args, .. } => {
            match scope.get(name.as_str()) {
                Some(Binding::Callable {
                    signature: _,
                    targets: Some(targets),
                }) => output.extend(targets.iter().cloned()),
                Some(Binding::Callable {
                    signature,
                    targets: None,
                }) => {
                    if let Some(universe) = universe {
                        output.extend(candidates(signature, functions, universe));
                    }
                }
                Some(Binding::Shadow) => {}
                None if universe.is_some() => {
                    output.insert(name.clone());
                }
                None => {}
            }
            for argument in args {
                visit(argument, scope, functions, output, universe);
            }
        }
        ExprKind::Block { statements, tail } => {
            let mut block = scope.clone();
            for statement in statements {
                match statement {
                    Statement::Let {
                        name,
                        declared,
                        value,
                        ..
                    } => {
                        visit(value, &mut block, functions, output, universe);
                        block.insert(
                            name,
                            binding_for_let(declared.as_ref(), value, &block, functions),
                        );
                    }
                    Statement::Assign { name, value, .. } => {
                        visit(value, &mut block, functions, output, universe);
                        block.insert(name, Binding::Shadow);
                    }
                    Statement::Unsafe { body, .. } => {
                        visit(body, &mut block, functions, output, universe)
                    }
                    Statement::While {
                        condition, body, ..
                    } => {
                        visit(condition, &mut block, functions, output, universe);
                        visit(body, &mut block, functions, output, universe);
                    }
                    Statement::For {
                        item, values, body, ..
                    } => {
                        visit(values, &mut block, functions, output, universe);
                        let mut loop_scope = block.clone();
                        loop_scope.insert(item, Binding::Shadow);
                        visit(body, &mut loop_scope, functions, output, universe);
                    }
                }
            }
            visit(tail, &mut block, functions, output, universe);
        }
        ExprKind::Match {
            scrutinee, arms, ..
        } => {
            visit(scrutinee, scope, functions, output, universe);
            for arm in arms {
                let mut arm_scope = scope.clone();
                shadow_pattern(&arm.pattern, &mut arm_scope);
                if let Some(guard) = &arm.guard {
                    visit(guard, &mut arm_scope, functions, output, universe);
                }
                visit(&arm.value, &mut arm_scope, functions, output, universe);
            }
        }
        _ => {
            let mut index = 0;
            while let Some(child) = expression.child(index) {
                visit(child, scope, functions, output, universe);
                index += 1;
            }
        }
    }
}

fn binding_for_let(
    declared: Option<&Type>,
    value: &Expr,
    scope: &Scope<'_>,
    functions: &HashMap<&str, &Function>,
) -> Binding {
    let signature = declared
        .cloned()
        .or_else(|| reference_signature(value, scope, functions));
    let Type::Function { .. } = signature.as_ref().unwrap_or(&Type::I64) else {
        return Binding::Shadow;
    };
    let targets = reference_target(value, scope, functions);
    Binding::Callable {
        signature: signature.expect("function type checked above"),
        targets,
    }
}

fn reference_signature(
    value: &Expr,
    scope: &Scope<'_>,
    functions: &HashMap<&str, &Function>,
) -> Option<Type> {
    let ExprKind::Var(name) = &value.kind else {
        return None;
    };
    match scope.get(name.as_str()) {
        Some(Binding::Callable { signature, .. }) => Some(signature.clone()),
        Some(Binding::Shadow) => None,
        None => function_value_signature_for(name, functions),
    }
}

fn reference_target(
    value: &Expr,
    scope: &Scope<'_>,
    functions: &HashMap<&str, &Function>,
) -> Option<BTreeSet<String>> {
    let ExprKind::Var(name) = &value.kind else {
        return None;
    };
    match scope.get(name.as_str()) {
        Some(Binding::Callable { targets, .. }) => targets.clone(),
        Some(Binding::Shadow) => Some(BTreeSet::new()),
        None if function_value_signature_for(name, functions).is_some() => {
            Some(BTreeSet::from([name.clone()]))
        }
        None => Some(BTreeSet::new()),
    }
}

fn function_value_signature_for(name: &str, functions: &HashMap<&str, &Function>) -> Option<Type> {
    functions
        .get(name)
        .and_then(|function| function_value_signature(function))
}

fn candidates(
    signature: &Type,
    functions: &HashMap<&str, &Function>,
    universe: &BTreeSet<String>,
) -> BTreeSet<String> {
    functions
        .iter()
        .filter_map(|(name, function)| {
            (universe.contains(*name)
                && function_value_signature(function).as_ref() == Some(signature))
            .then(|| (*name).to_owned())
        })
        .collect()
}

fn shadow_pattern<'a>(pattern: &'a MatchPattern, scope: &mut Scope<'a>) {
    match pattern {
        MatchPattern::Binding { name, .. } => {
            scope.insert(name, Binding::Shadow);
        }
        MatchPattern::Record { fields, .. } => {
            for field in fields {
                shadow_field(&field.pattern, scope);
            }
        }
        MatchPattern::Or { alternatives, .. } => {
            for alternative in alternatives {
                shadow_pattern(alternative, scope);
            }
        }
        MatchPattern::Variant { .. }
        | MatchPattern::Wildcard { .. }
        | MatchPattern::Literal { .. } => {}
    }
}
fn shadow_field<'a>(pattern: &'a RecordMatchFieldPattern, scope: &mut Scope<'a>) {
    match pattern {
        RecordMatchFieldPattern::Binding { name, .. } => {
            scope.insert(name, Binding::Shadow);
        }
        RecordMatchFieldPattern::Record { fields, .. } => {
            for field in fields {
                shadow_field(&field.pattern, scope);
            }
        }
        RecordMatchFieldPattern::Wildcard { .. } => {}
    }
}
