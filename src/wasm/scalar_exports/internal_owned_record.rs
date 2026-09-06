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
        hir::reachable_authored_types(&functions, &[], &[], &available).map_err(|_| {
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
    Ok(())
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
                args,
                ..
            } => {
                if !type_arguments.is_empty() || instance.is_some() {
                    return Err(body_error(function_id));
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
