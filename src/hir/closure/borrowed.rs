//! Parameter-rooted borrowed text closures with independently bounded lifetimes.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn is_borrowed(expression: &ResolvedExpr) -> bool {
    matches!(&expression.kind, ResolvedExprKind::Closure { captures, .. }
        if captures.iter().any(|c| c.binding.ty == ResolvedType::Str
            || c.value.ty == ResolvedType::Str
            || c.binding.ownership == OwnershipMode::Borrow
            || c.value.ownership == OwnershipMode::Borrow))
}

pub(crate) fn validate(
    program: &ResolvedProgram,
    expression: &ResolvedExpr,
) -> Result<(), Diagnostic> {
    let reject =
        || hir_error("borrowed closure is outside the synchronous parameter-rooted profile");
    let ResolvedExprKind::Closure {
        parameters,
        captures,
        body,
    } = &expression.kind
    else {
        return Err(reject());
    };
    let [capture] = captures.as_slice() else {
        return Err(reject());
    };
    let [parameter] = parameters.as_slice() else {
        return Err(reject());
    };
    let ty = ResolvedType::Function {
        parameters: vec![ResolvedType::I64],
        result: Box::new(ResolvedType::I64),
    };
    let target = closure_id(&expression.id);
    let execution = FunctionExecutionId::Monomorphic(target.clone());
    if expression.ty != ty
        || expression.ownership != OwnershipMode::Value
        || program.declarations.declaration(&target).is_some()
        || capture.binding.ty != ResolvedType::Str
        || capture.value.ty != ResolvedType::Str
        || capture.binding.ownership != OwnershipMode::Borrow
        || capture.value.ownership != OwnershipMode::Borrow
        || capture.binding.id != ValueId::parameter(&execution, 0)
        || parameter.id != ValueId::parameter(&execution, 1)
        || parameter.ty != ResolvedType::I64
        || parameter.ownership != OwnershipMode::Value
        || parameter.name == capture.binding.name
        || !matches!(&capture.value.kind, ResolvedExprKind::Place(place) if place.projections.is_empty())
        || body.ty != ResolvedType::I64
        || body.ownership != OwnershipMode::Value
    {
        return Err(reject());
    }
    let ResolvedExprKind::Block { statements, tail } = &body.kind else {
        return Err(reject());
    };
    let ResolvedExprKind::Call {
        callee,
        instance,
        type_arguments,
        args,
    } = &tail.kind
    else {
        return Err(reject());
    };
    if !statements.is_empty()
        || instance.is_some()
        || !type_arguments.is_empty()
        || tail.ty != ResolvedType::I64
        || tail.ownership != OwnershipMode::Value
        || args.len() != 2
    {
        return Err(reject());
    }
    let function = program
        .functions
        .iter()
        .find(|f| f.id == *callee)
        .ok_or_else(reject)?;
    if function.yields.is_some()
        || !function.effects.is_empty()
        || function.return_type != ResolvedType::I64
        || function.params.len() != 2
        || function.params[0].ty != ResolvedType::Str
        || function.params[0].ownership != OwnershipMode::Borrow
        || function.params[1].ty != ResolvedType::I64
        || function.params[1].ownership != OwnershipMode::Value
    {
        return Err(reject());
    }
    for (argument, binding) in args.iter().zip([&capture.binding, parameter]) {
        if argument.ty != binding.ty
            || argument.ownership != binding.ownership
            || !matches!(&argument.kind, ResolvedExprKind::Place(place) if place.projections.is_empty() && place.root == binding.id)
        {
            return Err(reject());
        }
    }
    Ok(())
}

/// Every descriptor remains in the creator's synchronous frame. Capture roots
/// must be that frame's shared parameters, not an owned local with a shorter loan.
pub(crate) fn validate_uses(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    for function in program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
    {
        let mut sites = BTreeSet::new();
        let mut bindings = BTreeMap::new();
        let mut invalid = false;
        super::super::function_value::walk(function, |expression| {
            if is_borrowed(expression) {
                sites.insert(expression.id.clone());
            }
            if let ResolvedExprKind::Block { statements, .. } = &expression.kind {
                for statement in statements {
                    if let ResolvedStatement::Let {
                        binding,
                        mutable,
                        value,
                        ..
                    } = statement
                    {
                        if is_borrowed(value) {
                            invalid |= *mutable;
                            bindings.insert(binding.id.clone(), value.id.clone());
                            let ResolvedExprKind::Closure { captures, .. } = &value.kind else {
                                unreachable!()
                            };
                            for capture in captures {
                                let ResolvedExprKind::Place(root) = &capture.value.kind else {
                                    invalid = true;
                                    continue;
                                };
                                invalid |= !function.params.iter().any(|p| {
                                    p.id == root.root
                                        && p.ty == ResolvedType::Str
                                        && p.ownership == OwnershipMode::Borrow
                                });
                            }
                        }
                    }
                }
            }
        });
        if sites.is_empty() {
            continue;
        }
        invalid |= !function.effects.is_empty()
            || function.yields.is_some()
            || sites != bindings.values().cloned().collect();
        let mut contracts = function
            .requires
            .iter()
            .chain(&function.ensures)
            .collect::<Vec<_>>();
        while let Some(expression) = contracts.pop() {
            invalid |= is_borrowed(expression);
            super::super::push_resolved_expression_children_in_authored_order(
                expression,
                &mut contracts,
            );
        }
        let mut direct_invocations = BTreeSet::new();
        super::super::function_value::walk(function, |expression| {
            if let ResolvedExprKind::Invoke { callable, .. } = &expression.kind {
                if matches!(&callable.kind, ResolvedExprKind::Place(place) if bindings.contains_key(&place.root) && place.projections.is_empty())
                {
                    direct_invocations.insert(callable.id.clone());
                }
            }
        });
        super::super::function_value::walk(function, |expression| {
            if let ResolvedExprKind::Place(place) = &expression.kind {
                if bindings.contains_key(&place.root)
                    && !direct_invocations.contains(&expression.id)
                {
                    invalid = true;
                }
            }
        });
        // Generic instances cannot acquire this profile by forged substitution.
        invalid |= program
            .function_instances
            .iter()
            .any(|i| i.function.id == function.id);
        if invalid {
            return Err(hir_error(
                "borrowed closure must remain in its parameter-rooted synchronous local scope",
            ));
        }
    }
    Ok(())
}
