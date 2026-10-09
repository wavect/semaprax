//! Project v29: authenticated private Copy records, with the v27 host boundary.
use super::*;

/// Codec outcomes have no name-based privilege: only these exact closed shapes qualify.
fn codec_mode(index: &DeclarationIndex, ty: &ResolvedType) -> Option<OwnershipMode> {
    if super::super::collection_outcome::admitted(index, ty) {
        return Some(OwnershipMode::Own);
    }
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return None;
    };
    if !arguments.is_empty()
        || index
            .type_parameters(declaration)
            .is_none_or(|p| !p.is_empty())
        || index.declaration(declaration).is_none_or(|d| {
            d.kind != DeclarationKind::Variant || d.identity_origin != IdentityOrigin::Explicit
        })
    {
        return None;
    }
    let cases = index.variant_cases(declaration)?;
    if cases.len() != 2 {
        return None;
    }
    let record = |fields: &[ResolvedFieldDeclaration]| matches!(fields,[field] if super::super::copy_record_collection::admitted(index,&field.ty));
    let error = |fields: &[ResolvedFieldDeclaration]| matches!(fields,[a,b,c] if a.ty==ResolvedType::I64&&b.ty==ResolvedType::Usize&&c.ty==ResolvedType::I64);
    if (record(&cases[0].fields) && error(&cases[1].fields))
        || (record(&cases[1].fields) && error(&cases[0].fields))
    {
        return Some(OwnershipMode::Value);
    }
    let buffered = |fields: &[ResolvedFieldDeclaration]| matches!(fields,[bytes,length] if bytes.ty==ResolvedType::Bytes && length.ty==ResolvedType::Usize);
    if (buffered(&cases[0].fields) && error(&cases[1].fields))
        || (buffered(&cases[1].fields) && error(&cases[0].fields))
    {
        return Some(OwnershipMode::Own);
    }
    let text = |fields: &[ResolvedFieldDeclaration]| matches!(fields,[field] if field.ty==ResolvedType::String);
    let refused = |fields: &[ResolvedFieldDeclaration]| matches!(fields,[field] if field.ty==ResolvedType::Usize);
    if (text(&cases[0].fields) && refused(&cases[1].fields))
        || (text(&cases[1].fields) && refused(&cases[0].fields))
    {
        return Some(OwnershipMode::Own);
    }
    None
}

pub(crate) fn stream_record_parameter_admitted(
    index: &DeclarationIndex,
    p: &ResolvedParam,
) -> bool {
    super::stdin_stream::stream_data_parameter_admitted(p)
        || codec_mode(index, &p.ty) == Some(p.ownership)
        || (p.ownership == OwnershipMode::Borrow
            && super::super::collection_outcome::admitted(index, &p.ty))
        || (p.ownership == OwnershipMode::Value
            && super::super::copy_record_collection::admitted(index, &p.ty))
        || (matches!(p.ownership, OwnershipMode::Own | OwnershipMode::Borrow)
            && super::super::copy_record_collection::is_vec(index, &p.ty))
}
pub(crate) fn stream_record_return_admitted(index: &DeclarationIndex, ty: &ResolvedType) -> bool {
    super::stdin_stream::stream_text_return_admitted(ty)
        || codec_mode(index, ty).is_some()
        || super::super::copy_record_collection::admitted(index, ty)
        || super::super::copy_record_collection::is_vec(index, ty)
}
pub(crate) fn stream_record_signature_admitted(
    program: &ResolvedProgram,
    f: &ResolvedFunction,
) -> bool {
    stream_record_return_admitted(&program.declarations, &f.return_type)
        && f.params
            .iter()
            .all(|p| stream_record_parameter_admitted(&program.declarations, p))
        && (!crate::stdin_stream_ops::is_reader(&f.return_type)
            || crate::stdin_stream_ops::resolved_forward_signature(f))
}

pub(crate) fn validate_stream_record_program(
    program: &ResolvedProgram,
    command: Option<&DeclarationId>,
) -> Result<(), Diagnostic> {
    validate(program)?;
    if !program.interfaces.is_empty()
        || !program.function_templates.is_empty()
        || !program.function_instances.is_empty()
    {
        return Err(link_error(
            "stream record transport requires a monomorphic interface-free closure",
        ));
    }
    if command.is_some_and(|id| !program.functions.iter().any(|f| &f.id == id)) {
        return Err(link_error("stream record command is absent"));
    }
    for f in &program.functions {
        let root = f.id == program.entrypoint || command == Some(&f.id);
        if (if root {
            !f.params.is_empty() || f.return_type != ResolvedType::I64
        } else {
            !stream_record_signature_admitted(program, f)
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
                "stream record helper requires an explicit admitted signature/effect closure",
            ));
        }
        for declaration in super::super::authored_nominal_declarations(f) {
            let ty = ResolvedType::Nominal {
                declaration,
                arguments: Vec::new(),
            };
            if !super::super::copy_record_collection::admitted(&program.declarations, &ty)
                && codec_mode(&program.declarations, &ty).is_none()
            {
                return Err(link_error(
                    "stream record closure contains an unsupported authored nominal declaration",
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const TYPES: &str = r#"module record.profile;
@id("r") record R { @id("r.a") a:i64, @id("r.b") b:bool, }
@id("d") variant D { @id("d.ok") Decoded { @id("d.ok.value") value:R }, @id("d.error") Error { @id("d.error.code") code:i64, @id("d.error.offset") offset:usize, @id("d.error.field") field:i64 }, }
@id("e") variant E { @id("e.ok") Encoded { @id("e.ok.text") text:string }, @id("e.no") Refused { @id("e.no.required") required:usize }, }
@id("app.main") fn main()->i64 {0}
"#;
    fn resolved(source: &str) -> ResolvedProgram {
        let ast = crate::check(source, "record-profile.spx").unwrap();
        crate::hir::resolve(&ast).unwrap()
    }
    fn nominal(id: &str) -> ResolvedType {
        ResolvedType::Nominal {
            declaration: DeclarationId::new(id),
            arguments: vec![],
        }
    }
    #[test]
    fn exact_codec_shapes_use_declared_types_and_ownership() {
        let program = resolved(TYPES);
        assert_eq!(
            codec_mode(&program.declarations, &nominal("d")),
            Some(OwnershipMode::Value)
        );
        assert_eq!(
            codec_mode(&program.declarations, &nominal("e")),
            Some(OwnershipMode::Own)
        );
        for source in [
            TYPES.replace("offset:usize", "offset:i64"),
            TYPES.replace("value:R", "value:i64"),
            TYPES.replace("required:usize", "required:bool"),
            TYPES.replace("text:string", "text:Bytes"),
        ] {
            let changed = resolved(&source);
            assert!(
                codec_mode(&changed.declarations, &nominal("d")).is_none()
                    || codec_mode(&changed.declarations, &nominal("e")).is_none()
            );
        }
    }
    #[test]
    fn profile_independently_refuses_noncopy_record_and_old_profile_stays_closed() {
        let source = TYPES.replace(
            "fn main()->i64 {0}",
            "fn main()->i64 { let r=R{a:1,b:true}; r.a }",
        );
        let mut program = resolved(&source);
        validate_stream_record_program(&program, None).unwrap();
        assert!(super::super::stdin_stream::validate_stream_data_program(&program, None).is_err());
        let fields = program
            .declarations
            .record_fields
            .get_mut(&DeclarationId::new("r"))
            .unwrap();
        fields[0].ty = ResolvedType::String;
        assert!(validate_stream_record_program(&program, None).is_err());
    }
    #[test]
    fn stream_record_profile_authenticates_owned_buffer_length_outcome() {
        let source = r#"module stream.input;
@id("input") variant Input {
 @id("input.ok") Ready { @id("input.bytes") bytes:Bytes, @id("input.length") length:usize, },
 @id("input.no") Error { @id("input.code") code:i64, @id("input.offset") offset:usize, @id("input.field") field:i64, },
}
@id("app.main") fn main()->i64 {0}
"#;
        let checked = resolved(source);
        assert_eq!(
            codec_mode(&checked.declarations, &nominal("input")),
            Some(OwnershipMode::Own)
        );
        for changed in [
            source.replace("length:usize", "length:i64"),
            source.replace("offset:usize", "offset:i64"),
        ] {
            let checked = resolved(&changed);
            assert_eq!(codec_mode(&checked.declarations, &nominal("input")), None);
        }
    }
}
