//! Additive Project v32 private owning nested-outcome closure.
use super::*;

pub(crate) fn stream_nested_outcome_signature_admitted(
    program: &ResolvedProgram,
    f: &ResolvedFunction,
) -> bool {
    let index = &program.declarations;
    let nested = |ty| {
        super::super::owned_collection_record::admitted(ty, index)
            || super::super::collection_outcome::nested::admitted(index, ty)
            || super::super::collection_outcome::nested::record_payload_admitted(index, ty)
    };
    (super::stream_owned::return_admitted(index, &f.return_type) || nested(&f.return_type))
        && f.params.iter().all(|parameter| {
            super::stream_owned::parameter_admitted(index, parameter)
                || (matches!(
                    parameter.ownership,
                    OwnershipMode::Own | OwnershipMode::Borrow
                ) && nested(&parameter.ty))
        })
        && (!crate::stdin_stream_ops::is_reader(&f.return_type)
            || crate::stdin_stream_ops::resolved_forward_signature(f))
}

pub(crate) fn validate_stream_nested_outcome_program(
    program: &ResolvedProgram,
    command: Option<&DeclarationId>,
) -> Result<(), Diagnostic> {
    validate(program)?;
    if !program.interfaces.is_empty()
        || !program.function_templates.is_empty()
        || !program.function_instances.is_empty()
    {
        return Err(link_error(
            "nested-outcome command requires a monomorphic interface-free closure",
        ));
    }
    if command.is_some_and(|id| !program.functions.iter().any(|f| &f.id == id)) {
        return Err(link_error("nested-outcome command is absent"));
    }
    for f in &program.functions {
        let root = f.id == program.entrypoint || command == Some(&f.id);
        if (if root {
            !f.params.is_empty() || f.return_type != ResolvedType::I64
        } else {
            !stream_nested_outcome_signature_admitted(program, f)
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
                "nested-outcome helper requires an explicit admitted signature/effect closure",
            ));
        }
        for declaration in super::super::authored_nominal_declarations(f) {
            let ty = ResolvedType::Nominal {
                declaration,
                arguments: Vec::new(),
            };
            if !super::super::owned_collection_record::admitted(&ty, &program.declarations)
                && !super::super::copy_record_collection::admitted(&program.declarations, &ty)
                && super::stream_record::codec_mode(&program.declarations, &ty).is_none()
                && super::super::owned_leaf_collection::layout(&program.declarations, &ty).is_none()
                && !super::super::collection_outcome::owned_admitted(&program.declarations, &ty)
                && !super::super::collection_outcome::nested::admitted(&program.declarations, &ty)
                && !super::super::collection_outcome::nested::record_payload_admitted(
                    &program.declarations,
                    &ty,
                )
            {
                return Err(link_error(
                    "nested-outcome closure contains an unsupported authored nominal declaration",
                ));
            }
        }
    }
    Ok(())
}
