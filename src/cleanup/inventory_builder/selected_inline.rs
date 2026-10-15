//! Exact borrowed classification twins; finite error data never allocates.
//! Callers pay their receiving controls before invoking these pure queries.
use crate::cleanup_plan::renewal_profile::{
    child_refusal::Refusal, profile_refusal::ProfileRefusal,
};
use crate::hir::{
    DeclarationId, DeclarationKind, ResolvedFieldDeclaration, ResolvedType,
    ResolvedTypeDeclarationKind,
};
const MAX_DECLARATIONS: usize = 32;
const MAX_FIELDS: usize = 8;
fn refusal(reason: ProfileRefusal) -> Refusal {
    Refusal::Profile(reason)
}
pub(crate) fn selected_vector_element_inline<'a>(
    program: &dyn crate::cleanup::ReplayContext,
    ty: &'a ResolvedType,
) -> Result<Option<&'a ResolvedType>, Refusal> {
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
    authenticate_selected_vector_inline(program, declaration)?;
    let [element] = arguments.as_slice() else {
        return Err(refusal(
            ProfileRefusal::SelectedVecHasInvalidTypeArgumentCount,
        ));
    };
    if crate::vec_ops::resolved_vec_element_is_admitted(element) || *element == ResolvedType::String
    {
        return Ok(Some(element));
    }
    if matches!(element, ResolvedType::Nominal { arguments, .. } if arguments.is_empty()) {
        selected_flat_record_fields_inline(program, element)?;
        if crate::hir::copy_record_collection::admitted(program.declarations(), element)
            || crate::hir::owned_leaf_collection::runtime_element(program.declarations(), element)
        {
            return Ok(Some(element));
        }
    }
    Err(refusal(
        ProfileRefusal::SelectedVecElementIsOutsideTheCurrentBoundedFlatFamily,
    ))
}
pub(crate) fn authenticate_selected_vector_inline(
    program: &dyn crate::cleanup::ReplayContext,
    id: &DeclarationId,
) -> Result<(), Refusal> {
    if program.type_count() > 32 {
        return Err(refusal(
            ProfileRefusal::SelectedVecCurrentDirectoryExceedsItsBoundedTranche,
        ));
    }
    let mut current = None;
    for ordinal in 0..program.type_count() {
        let item = program
            .type_at(ordinal)
            .ok_or_else(|| refusal(ProfileRefusal::SelectedVecCurrentDirectoryIsIncomplete))?;
        if &item.id == id && current.replace(item).is_some() {
            return Err(refusal(
                ProfileRefusal::SelectedVecCurrentDirectoryRepeatsItsIdentity,
            ));
        }
    }
    let current = current
        .ok_or_else(|| refusal(ProfileRefusal::SelectedVecDeclarationIsAbsentFromCurrentTypes))?;
    let index = program.declarations();
    let fail =
        || refusal(ProfileRefusal::SelectedVecDeclarationIsNotTheExactCompilerOwnedCollection);
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
pub(crate) fn prepay_selected_collection_probes_inline<E: From<Refusal>>(
    program: &dyn crate::cleanup::ReplayContext,
    query_bytes: usize,
    charge: &mut dyn FnMut(usize) -> Result<(), E>,
) -> Result<(), E> {
    let fail = || {
        E::from(refusal(
            ProfileRefusal::SelectedCollectionProbePrepaymentRefused,
        ))
    };
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

pub(crate) fn selected_flat_record_fields_inline<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    ty: &'a ResolvedType,
) -> Result<Option<(&'a DeclarationId, &'a [ResolvedFieldDeclaration])>, Refusal> {
    if crate::hir::borrowed_scalar::option_u8::is_type(ty) {
        return Ok(None);
    }
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(None);
    };
    if !arguments.is_empty() {
        return Err(refusal(
            ProfileRefusal::SelectedFlatRecordArgumentsAreUnsupported,
        ));
    }
    selected_flat_record_declaration_inline(program, declaration)
}
pub(crate) fn selected_flat_record_declaration_inline<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    declaration: &'a DeclarationId,
) -> Result<Option<(&'a DeclarationId, &'a [ResolvedFieldDeclaration])>, Refusal> {
    let fail = || {
        refusal(ProfileRefusal::SelectedFlatRecordInventoryHasUnauthenticatedOrUnsupportedFields)
    };
    if program.type_count() > MAX_DECLARATIONS {
        return Err(fail());
    }
    let mut matching = None;
    for ordinal in 0..program.type_count() {
        let item = program.type_at(ordinal).ok_or_else(fail)?;
        if item.id == *declaration && matching.replace(item).is_some() {
            return Err(fail());
        }
    }
    let record = matching.ok_or_else(fail)?;
    if !record.type_parameters.is_empty() {
        return Err(fail());
    }
    let ResolvedTypeDeclarationKind::Record { fields } = &record.kind else {
        return Err(fail());
    };
    if !(1..=MAX_FIELDS).contains(&fields.len()) {
        return Err(fail());
    }
    // Establish leaf-only equality before comparing against untrusted indexed
    // fields; no nested type tree is walked by the selected classifier.
    if fields.iter().any(|field| {
        !(crate::hir::is_scalar_resolved_type(&field.ty)
            || matches!(field.ty, ResolvedType::String | ResolvedType::Bytes))
    }) {
        return Err(fail());
    }
    let index = program.declarations();
    let declared = index.declaration(declaration).ok_or_else(fail)?;
    if declared.kind != DeclarationKind::Record
        || declared.owner.is_some()
        || declared.name != record.name
        || index.record_fields(declaration) != Some(fields.as_slice())
    {
        return Err(fail());
    }
    for (position, field) in fields.iter().enumerate() {
        let field_fact = index.declaration(&field.id).ok_or_else(fail)?;
        if field.index as usize != position
            || field_fact.kind != DeclarationKind::Field
            || field_fact.owner.as_ref() != Some(declaration)
            || field_fact.name != field.name
            || fields[..position]
                .iter()
                .any(|old| old.id == field.id || old.name == field.name)
            || !(crate::hir::is_scalar_resolved_type(&field.ty)
                || matches!(field.ty, ResolvedType::String | ResolvedType::Bytes))
        {
            return Err(fail());
        }
    }
    Ok(Some((declaration, fields)))
}
