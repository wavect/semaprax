//! Exact normalizer-owner to finite nested outcome join; no name-based privilege.
use super::*;

pub(super) fn match_result(
    types: &TypeTable<'_>,
    input: &Type,
    mode: MatchMode,
    result: &Type,
    ownership: ParamMode,
) -> bool {
    if mode != MatchMode::Own || ownership != ParamMode::Own {
        return false;
    }
    let (
        Type::Named { name, arguments },
        Type::Named {
            name: output,
            arguments: output_arguments,
        },
    ) = (input, result)
    else {
        return false;
    };
    if !arguments.is_empty() || !output_arguments.is_empty() {
        return false;
    }
    let Some(declaration) = types.declaration(name) else {
        return false;
    };
    if !declaration.explicit_id
        || !declaration.type_parameters.is_empty()
        || !declaration.invariants().is_empty()
    {
        return false;
    }
    let TypeDeclarationKind::Variant { cases } = &declaration.kind else {
        return false;
    };
    if cases.len() != 2
        || cases
            .iter()
            .any(|case| !case.explicit_id || case.fields.iter().any(|field| !field.explicit_id))
    {
        return false;
    }
    let ready = |fields: &[FieldDeclaration]| matches!(fields, [bytes, length] if bytes.ty==Type::Bytes && length.ty==Type::Usize);
    let error = |fields: &[FieldDeclaration]| matches!(fields, [code, offset, field] if code.ty==Type::I64 && offset.ty==Type::Usize && field.ty==Type::I64);
    ((ready(&cases[0].fields) && error(&cases[1].fields))
        || (ready(&cases[1].fields) && error(&cases[0].fields)))
        && types
            .declaration(output)
            .is_some_and(|declaration| super::nested::admitted(types, declaration))
}
