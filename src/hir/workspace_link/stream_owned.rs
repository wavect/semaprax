//! Project v30 private runtime owned leaves; public command ABI remains scalar.
use super::*;

fn private_carrier(index: &DeclarationIndex, ty: &ResolvedType) -> bool {
    super::super::owned_leaf_collection::layout(index, ty).is_some()
        || super::super::collection_outcome::owned_admitted(index, ty)
        || super::super::owned_leaf_collection::is_vec(index, ty)
        || crate::iterator_ops::element(ty).is_some_and(|element| {
            super::super::owned_leaf_collection::layout(index, element).is_some()
        })
}

pub(super) fn return_admitted(index: &DeclarationIndex, ty: &ResolvedType) -> bool {
    super::stream_record::stream_record_return_admitted(index, ty) || private_carrier(index, ty)
}

pub(super) fn parameter_admitted(index: &DeclarationIndex, parameter: &ResolvedParam) -> bool {
    super::stream_record::stream_record_parameter_admitted(index, parameter)
        || (matches!(
            parameter.ownership,
            OwnershipMode::Own | OwnershipMode::Borrow
        ) && private_carrier(index, &parameter.ty))
}

pub(crate) fn stream_owned_signature_admitted(
    program: &ResolvedProgram,
    f: &ResolvedFunction,
) -> bool {
    return_admitted(&program.declarations, &f.return_type)
        && f.params
            .iter()
            .all(|p| parameter_admitted(&program.declarations, p))
        && (!crate::stdin_stream_ops::is_reader(&f.return_type)
            || crate::stdin_stream_ops::resolved_forward_signature(f))
}

/// A body-only operation cannot gain authority through an old scalar signature.
/// Declaration-only logical schemas are intentionally absent from this walk.
pub(crate) fn function_requires_owned_profile(
    program: &ResolvedProgram,
    f: &ResolvedFunction,
) -> bool {
    fn new_carrier(index: &DeclarationIndex, ty: &ResolvedType) -> bool {
        if super::super::collection_outcome::owned_admitted(index, ty) {
            return true;
        }
        match ty {
            ResolvedType::Nominal {
                declaration,
                arguments,
            } if matches!(
                declaration.as_str(),
                crate::prelude::VEC_ID
                    | crate::iterator_ops::ITER_ID
                    | crate::iterator_ops::STEP_ID
            ) =>
            {
                matches!(arguments.as_slice(), [element] if crate::hir::owned_leaf_collection::layout(index, element).is_some()
                    && !crate::hir::owned_record_collection::is_admitted_owned_record_collection_element(index, element))
            }
            _ => false,
        }
    }
    if new_carrier(&program.declarations, &f.return_type)
        || f.params
            .iter()
            .any(|p| new_carrier(&program.declarations, &p.ty))
    {
        return true;
    }
    let mut pending = f
        .requires
        .iter()
        .chain(std::iter::once(&f.body))
        .chain(&f.ensures)
        .collect::<Vec<_>>();
    while let Some(expr) = pending.pop() {
        if new_carrier(&program.declarations, &expr.ty)
            || matches!(&expr.kind, ResolvedExprKind::Call { callee, .. } if crate::vec_ops::by_id(callee.as_str()).is_some_and(|op| op.owned_leaf_only()))
        {
            return true;
        }
        super::super::push_resolved_expression_children_in_authored_order(expr, &mut pending);
    }
    false
}

pub(crate) fn validate_stream_owned_program(
    program: &ResolvedProgram,
    command: Option<&DeclarationId>,
) -> Result<(), Diagnostic> {
    validate(program)?;
    if super::super::collection_outcome::nested::program_requires_profile(program) {
        return Err(link_error(
            "owning nested outcomes require the nested-outcome successor profile",
        ));
    }
    if super::super::owned_collection_record::program_requires_profile(program) {
        return Err(link_error(
            "nested collection records require the collection-record successor profile",
        ));
    }
    if !program.interfaces.is_empty()
        || !program.function_templates.is_empty()
        || !program.function_instances.is_empty()
    {
        return Err(link_error(
            "owned-data command requires a monomorphic interface-free closure",
        ));
    }
    if command.is_some_and(|id| !program.functions.iter().any(|f| &f.id == id)) {
        return Err(link_error("owned-data command is absent"));
    }
    for f in &program.functions {
        let root = f.id == program.entrypoint || command == Some(&f.id);
        if (if root {
            !f.params.is_empty() || f.return_type != ResolvedType::I64
        } else {
            !stream_owned_signature_admitted(program, f)
        }) || program
            .declarations
            .declaration(&f.id)
            .is_none_or(|d| d.identity_origin != IdentityOrigin::Explicit)
            || (command.is_none() && !f.effects.is_empty())
            || !f.effects.iter().all(|effect| {
                matches!(
                    effect.as_str(),
                    crate::command_io_ops::ARGS_READ_EFFECT
                        | crate::command_io_ops::STDIN_READ_EFFECT
                        | crate::command_io_ops::STDERR_WRITE_EFFECT
                        | crate::host_io_ops::STDOUT_WRITE_EFFECT
                )
            })
        {
            return Err(link_error(
                "owned-data helper requires an explicit admitted signature/effect closure",
            ));
        }
        for declaration in super::super::authored_nominal_declarations(f) {
            let ty = ResolvedType::Nominal {
                declaration,
                arguments: Vec::new(),
            };
            if !super::super::copy_record_collection::admitted(&program.declarations, &ty)
                && super::stream_record::codec_mode(&program.declarations, &ty).is_none()
                && super::super::owned_leaf_collection::layout(&program.declarations, &ty).is_none()
                && !super::super::collection_outcome::owned_admitted(&program.declarations, &ty)
            {
                return Err(link_error(
                    "owned-data closure contains an unsupported authored nominal declaration",
                ));
            }
        }
    }
    Ok(())
}
