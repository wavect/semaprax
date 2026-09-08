//! Narrow internal aggregate body admitted behind the frozen scalar ABI.

use super::*;
use crate::hir::{ResolvedMatchMode, ResolvedMatchPattern};

pub(super) fn validate_program(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    let mut available = BTreeMap::new();
    for declaration in &program.types {
        if available
            .insert(declaration.id.clone(), declaration.clone())
            .is_some()
        {
            return Err(admission(
                "Public Scalar Export Profile v1 internal record identity is duplicated",
            ));
        }
    }
    let functions = program
        .functions
        .iter()
        .map(|function| {
            let origin = program
                .declarations
                .declaration(&function.id)
                .map(|declaration| declaration.identity_origin)
                .ok_or_else(|| {
                    admission(
                        "Public Scalar Export Profile v1 internal function identity is absent",
                    )
                })?;
            Ok(hir::LinkedScalarFunction {
                function: function.clone(),
                origin,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    let mut expected =
        hir::reachable_authored_types(&functions, &program.function_instances, &[], &available)
            .map_err(|_| {
                admission("Public Scalar Export Profile v1 internal record closure is not exact")
            })?;
    let mut actual = program
        .types
        .iter()
        .filter(|declaration| {
            program
                .declarations
                .declaration(&declaration.id)
                .is_some_and(|item| item.identity_origin != IdentityOrigin::CompilerOwned)
        })
        .cloned()
        .collect::<Vec<_>>();
    expected.sort_by(|left, right| left.id.cmp(&right.id));
    actual.sort_by(|left, right| left.id.cmp(&right.id));
    if actual.iter().any(|declaration| {
        declaration.type_parameters.is_empty()
            || !matches!(
                declaration.kind,
                hir::ResolvedTypeDeclarationKind::Record { .. }
            )
    }) {
        return Err(admission(
            "Public Scalar Export Profile v1 does not admit authored resource, record, or variant declarations",
        ));
    }
    if actual != expected {
        return Err(admission(
            "Public Scalar Export Profile v1 admits only the exact reachable internal generic record closure",
        ));
    }
    validate_generic_closure(program)?;
    Ok(())
}

pub(super) fn validate_instance(
    program: &ResolvedProgram,
    instance: &hir::ResolvedFunctionInstance,
) -> Result<(), Diagnostic> {
    let scalar_signature = instance.function.params.iter().all(|parameter| {
        parameter.ownership == OwnershipMode::Value && super::scalar_type(&parameter.ty).is_some()
    }) && super::scalar_type(&instance.function.return_type).is_some();
    let owned_identity_signature = instance.function.params.len() == 1
        && instance.function.params[0].ownership == OwnershipMode::Own
        && instance.function.params[0].ty == instance.function.return_type
        && hir::is_flat_owned_byte_record(&program.declarations, &instance.function.return_type);
    if !instance.function.effects.is_empty() || (!scalar_signature && !owned_identity_signature) {
        return Err(body_error(&instance.template));
    }
    for expression in instance
        .function
        .requires
        .iter()
        .chain(std::iter::once(&instance.function.body))
        .chain(instance.function.ensures.iter())
    {
        validate_expression(program, expression, &instance.template)?;
    }
    Ok(())
}

fn validate_generic_closure(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    let templates = program
        .function_templates
        .iter()
        .map(|template| (template.id.clone(), template))
        .collect::<BTreeMap<_, _>>();
    let instances = program
        .function_instances
        .iter()
        .map(|instance| (instance.id.clone(), instance))
        .collect::<BTreeMap<_, _>>();
    if templates.len() != program.function_templates.len()
        || instances.len() != program.function_instances.len()
    {
        return Err(admission(
            "Public Scalar Export Profile v1 internal generic closure is duplicated",
        ));
    }
    let mut pending = std::collections::VecDeque::new();
    for function in &program.functions {
        collect_generic_calls(function, &mut pending);
    }
    let mut seen_instances = BTreeSet::new();
    let mut seen_templates = BTreeSet::new();
    while let Some((callee, attached, arguments)) = pending.pop_front() {
        let Some(template) = templates.get(&callee) else {
            return Err(admission(
                "Public Scalar Export Profile v1 internal generic call target is not authenticated",
            ));
        };
        let derived = hir::FunctionInstanceId::derive(&callee, &arguments);
        if template.type_parameters.len() != arguments.len() || attached != derived {
            return Err(admission(
                "Public Scalar Export Profile v1 internal generic call instance is not canonical",
            ));
        }
        let Some(instance) = instances.get(&derived) else {
            return Err(admission(
                "Public Scalar Export Profile v1 internal generic call instance is absent",
            ));
        };
        seen_templates.insert(callee);
        if seen_instances.insert(derived) {
            collect_generic_calls(&instance.function, &mut pending);
        }
    }
    if seen_templates != templates.keys().cloned().collect()
        || seen_instances != instances.keys().cloned().collect()
    {
        return Err(admission(
            "Public Scalar Export Profile v1 internal generic closure is not exact",
        ));
    }
    Ok(())
}

fn collect_generic_calls(
    function: &ResolvedFunction,
    pending: &mut std::collections::VecDeque<(
        DeclarationId,
        hir::FunctionInstanceId,
        Vec<ResolvedType>,
    )>,
) {
    for expression in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(function.ensures.iter())
    {
        hir::visit_resolved_calls(expression, &mut |callee, instance, arguments| {
            if let Some(instance) = instance {
                pending.push_back((callee.clone(), instance.clone(), arguments.to_vec()));
            }
        });
    }
}

pub(super) fn validate_expression(
    program: &ResolvedProgram,
    expression: &ResolvedExpr,
    function_id: &DeclarationId,
) -> Result<(), Diagnostic> {
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        if !internal_type(program, &expression.ty) {
            return Err(body_error(function_id));
        }
        match &expression.kind {
            ResolvedExprKind::FunctionReference { .. } | ResolvedExprKind::Invoke { .. } => {
                return Err(body_error(function_id));
            }
            ResolvedExprKind::Int(_)
            | ResolvedExprKind::Int32(_)
            | ResolvedExprKind::Char(_)
            | ResolvedExprKind::Uint8(_)
            | ResolvedExprKind::Usize(_)
            | ResolvedExprKind::ArrayU8(_)
            | ResolvedExprKind::RepeatArrayU8 { .. }
            | ResolvedExprKind::Float32(_)
            | ResolvedExprKind::Float64(_)
            | ResolvedExprKind::Bool(_) => {}
            ResolvedExprKind::String(_) => return Err(body_error(function_id)),
            ResolvedExprKind::Place(place) => {
                if !place.projections.is_empty() {
                    return Err(body_error(function_id));
                }
            }
            ResolvedExprKind::BorrowPlace { .. } => {
                if expression.ty != ResolvedType::SliceU8 {
                    return Err(body_error(function_id));
                }
            }
            ResolvedExprKind::ByteRange { .. }
            | ResolvedExprKind::NativeRustImportCall(_)
            | ResolvedExprKind::HostCommandCall(_)
            | ResolvedExprKind::ConstructVariant { .. }
            | ResolvedExprKind::Try { .. }
            | ResolvedExprKind::TryOption { .. }
            | ResolvedExprKind::UpdateRecord { .. }
            | ResolvedExprKind::Project { .. }
            | ResolvedExprKind::Upcast { .. } => return Err(body_error(function_id)),
            ResolvedExprKind::Call {
                type_arguments,
                instance,
                callee,
                args,
                ..
            } => {
                if !type_arguments.is_empty() || instance.is_some() {
                    let Some(instance) = instance else {
                        return Err(body_error(function_id));
                    };
                    if *instance != hir::FunctionInstanceId::derive(callee, type_arguments)
                        || program
                            .resolve_call_target(callee, Some(instance))
                            .is_none()
                    {
                        return Err(body_error(function_id));
                    }
                }
                pending.extend(args);
            }
            ResolvedExprKind::Unary { value, .. } => pending.push(value),
            ResolvedExprKind::Binary { left, right, .. } => {
                pending.push(left);
                pending.push(right);
            }
            ResolvedExprKind::Block { statements, tail } => {
                for statement in statements {
                    match statement {
                        ResolvedStatement::Let { binding, value, .. } => {
                            if !internal_type(program, &binding.ty) {
                                return Err(body_error(function_id));
                            }
                            pending.push(value);
                        }
                        ResolvedStatement::Assign {
                            binding,
                            field: None,
                            value,
                            ..
                        } if super::scalar_type(&binding.ty).is_some() => pending.push(value),
                        ResolvedStatement::Assign { .. }
                        | ResolvedStatement::Unsafe { .. }
                        | ResolvedStatement::While { .. } => return Err(body_error(function_id)),
                    }
                }
                pending.push(tail);
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                pending.push(condition);
                pending.push(then_branch);
                pending.push(else_branch);
            }
            ResolvedExprKind::ConstructRecord { record, fields } => {
                let ResolvedType::Nominal { declaration, .. } = &expression.ty else {
                    return Err(body_error(function_id));
                };
                if declaration != record
                    || !hir::is_flat_owned_byte_record(&program.declarations, &expression.ty)
                {
                    return Err(body_error(function_id));
                }
                pending.extend(fields.iter().map(|field| &field.value));
            }
            ResolvedExprKind::Match {
                mode,
                scrutinee,
                arms,
            } => {
                if *mode != ResolvedMatchMode::Own
                    || !hir::is_flat_owned_byte_record(&program.declarations, &scrutinee.ty)
                    || arms.iter().any(|arm| {
                        arm.guard.is_some() || !record_pattern_is_exact(program, &arm.pattern)
                    })
                {
                    return Err(body_error(function_id));
                }
                pending.push(scrutinee);
                pending.extend(arms.iter().map(|arm| &arm.value));
            }
        }
    }
    Ok(())
}

fn record_pattern_is_exact(program: &ResolvedProgram, pattern: &ResolvedMatchPattern) -> bool {
    matches!(pattern, ResolvedMatchPattern::Record { record, instance, .. }
    if matches!(instance, ResolvedType::Nominal { declaration, .. } if declaration == record)
            && hir::is_flat_owned_byte_record(&program.declarations, instance))
}

fn internal_type(program: &ResolvedProgram, ty: &ResolvedType) -> bool {
    super::scalar_type(ty).is_some()
        || matches!(
            ty,
            ResolvedType::Usize
                | ResolvedType::ArrayU8(_)
                | ResolvedType::Bytes
                | ResolvedType::SliceU8
        )
        || hir::is_flat_owned_byte_record(&program.declarations, ty)
}

fn body_error(function_id: &DeclarationId) -> Diagnostic {
    admission(format!(
        "Public Scalar Export Profile v1 function `{function_id}` is outside the internal flat owned-record body profile"
    ))
}
