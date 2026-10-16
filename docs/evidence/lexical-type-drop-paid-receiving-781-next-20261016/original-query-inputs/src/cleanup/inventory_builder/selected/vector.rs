//! Borrowed current-declaration Vec classification. No generic substitution,
//! inferred layout, nested element or attached cleanup fact grants authority.
use super::*;

pub(crate) fn element<'a>(
    program: &dyn crate::cleanup::ReplayContext,
    ty: &'a ResolvedType,
) -> Result<Option<&'a ResolvedType>, Diagnostic> {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(None);
    };
    if declaration.as_str() != crate::prelude::VEC_ID {
        return Ok(None);
    }
    authenticate(program, declaration)?;
    let [element] = arguments.as_slice() else {
        return Err(cleanup_error(
            "selected Vec has invalid type argument count",
        ));
    };
    if crate::vec_ops::resolved_vec_element_is_admitted(element) || *element == ResolvedType::String
    {
        return Ok(Some(element));
    }
    if matches!(element, ResolvedType::Nominal { arguments, .. } if arguments.is_empty()) {
        flat_record::fields(program, element)?;
        if crate::hir::copy_record_collection::admitted(program.declarations(), element)
            || crate::hir::owned_leaf_collection::runtime_element(program.declarations(), element)
        {
            return Ok(Some(element));
        }
    }
    Err(cleanup_error(
        "selected Vec element is outside the current bounded flat family",
    ))
}
pub(crate) fn authenticate(
    program: &dyn crate::cleanup::ReplayContext,
    id: &DeclarationId,
) -> Result<(), Diagnostic> {
    if program.type_count() > 32 {
        return Err(cleanup_error(
            "selected Vec current directory exceeds its bounded tranche",
        ));
    }
    let mut current = None;
    for ordinal in 0..program.type_count() {
        let item = program
            .type_at(ordinal)
            .ok_or_else(|| cleanup_error("selected Vec current directory is incomplete"))?;
        if &item.id == id && current.replace(item).is_some() {
            return Err(cleanup_error(
                "selected Vec current directory repeats its identity",
            ));
        }
    }
    let current = current
        .ok_or_else(|| cleanup_error("selected Vec declaration is absent from current types"))?;
    let index = program.declarations();
    let fail =
        || cleanup_error("selected Vec declaration is not the exact compiler-owned collection");
    let declaration = index.declaration(id).ok_or_else(fail)?;
    let parameters = index.type_parameters(id).ok_or_else(fail)?;
    if id.as_str() != crate::prelude::VEC_ID
        || declaration.kind != crate::hir::DeclarationKind::Record
        || declaration.identity_origin != crate::hir::IdentityOrigin::CompilerOwned
        || declaration.owner.is_some()
        || declaration.name != "Vec"
        || index.record_fields(id) != Some(&[])
        || current.name != "Vec"
        || parameters.len() != 1
        || current.type_parameters.len() != 1
        || current.type_parameters.as_slice() != parameters
        || !matches!(&current.kind,ResolvedTypeDeclarationKind::Record{fields} if fields.is_empty())
        || parameters[0].name != "T"
        || parameters[0].index != 0
    {
        return Err(fail());
    }
    Ok(())
}

/// Conservative probe prepayment for one receiving classification. Length
/// inspection is constant-time; every directory entry is debited before it is
/// inspected. This census counts directory keys, not type admission: unrelated
/// generic declarations and non-flat fields cannot veto a demanded Vec/record.
/// The receiving classifiers separately establish leaf shapes before equality.
/// The namespace multiplier bounds even a linear ordered-map comparison path;
/// it is not an authority lease or a new work allowance.
pub(crate) fn prepay(
    program: &dyn crate::cleanup::ReplayContext,
    query_bytes: usize,
    charge: &mut dyn FnMut(usize) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    let fail = || cleanup_error("selected collection probe prepayment refused");
    if program.type_count() > 32 {
        return Err(fail());
    }
    let mut bytes = query_bytes.checked_add(1).ok_or_else(fail)?;
    for ordinal in 0..program.type_count() {
        charge(1)?;
        let item = program.type_at(ordinal).ok_or_else(fail)?;
        bytes = bytes
            .checked_add(item.id.as_str().len())
            .and_then(|n| n.checked_add(item.name.len() + 1))
            .ok_or_else(fail)?;
        for parameter in &item.type_parameters {
            charge(1)?;
            bytes = bytes
                .checked_add(parameter.name.len() + 1)
                .ok_or_else(fail)?;
        }
        if let ResolvedTypeDeclarationKind::Record { fields } = &item.kind {
            for field in fields {
                charge(1)?;
                bytes = bytes
                    .checked_add(field.id.as_str().len())
                    .and_then(|n| n.checked_add(field.name.len() + 1))
                    .ok_or_else(fail)?;
            }
        }
    }
    // Four root/argument/result Vec classifiers plus operation and ownership
    // classification use fewer than 64 indexed probes per key. The separate
    // 128 factor covers current-directory scans and bounded pairwise fields.
    let probes = program
        .declarations()
        .declarations()
        .len()
        .checked_add(1)
        .and_then(|n| n.checked_mul(64))
        .and_then(|n| n.checked_add(128))
        .ok_or_else(fail)?;
    charge(bytes.checked_mul(probes).ok_or_else(fail)?)
}

pub(in crate::cleanup::inventory_builder) fn key_bytes(
    ty: &ResolvedType,
) -> Result<usize, Diagnostic> {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(1);
    };
    let nested = match arguments.as_slice() {
        [] => 0,
        [ResolvedType::Nominal {
            declaration,
            arguments,
        }] if arguments.is_empty() => declaration.as_str().len(),
        [element]
            if crate::vec_ops::resolved_vec_element_is_admitted(element)
                || *element == ResolvedType::String =>
        {
            1
        }
        _ => {
            return Err(cleanup_error(
                "selected inventory type has unsupported arguments",
            ));
        }
    };
    declaration
        .as_str()
        .len()
        .checked_add(nested)
        .ok_or_else(|| cleanup_error("selected inventory key length overflow"))
}
