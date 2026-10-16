//! Full exact selected type/drop closure expanded in an existing paid caller.
//! Original public classifiers and ordinary type/drop remain independent.
macro_rules! required_closed {
    ($value:expr,$reason:ident) => {
        match $value {
            Some(value) => value,
            None => return Err(crate::cleanup::inventory_builder::typed::InventoryFailure(
                crate::cleanup::inventory_builder::typed::Kind::Profile(
                    crate::cleanup_plan::renewal_profile::profile_refusal::ProfileRefusal::$reason,
                ),
            )),
        }
    };
}

macro_rules! vector_element {($program:expr,$ty:expr)=>{{
use crate::hir::{ResolvedType};
let program:&dyn crate::cleanup::ReplayContext=$program;let ty:&crate::hir::ResolvedType=$ty;
'query:{

    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        break 'query None;
    };
    if declaration.as_str() != crate::prelude::VEC_ID {
        break 'query None;
    }
    crate::cleanup::inventory_builder::typed::queries::authenticate_vector!(program,declaration);
    let [element] = arguments.as_slice() else {
        return Err(crate::cleanup::inventory_builder::typed::InventoryFailure(crate::cleanup::inventory_builder::typed::Kind::Profile(crate::cleanup_plan::renewal_profile::profile_refusal::ProfileRefusal::SelectedVecHasInvalidTypeArgumentCount)));
    };
    if crate::vec_ops::resolved_vec_element_is_admitted(element) || *element == ResolvedType::String
    {
        break 'query Some(element);
    }
    if matches!(element, ResolvedType::Nominal { arguments, .. } if arguments.is_empty()) {
        crate::cleanup::inventory_builder::typed::queries::flat_fields!(program,element);
        if crate::hir::copy_record_collection::admitted(program.declarations(), element)
            || crate::hir::owned_leaf_collection::runtime_element(program.declarations(), element)
        {
            break 'query Some(element);
        }
    }
    return Err(crate::cleanup::inventory_builder::typed::InventoryFailure(crate::cleanup::inventory_builder::typed::Kind::Profile(crate::cleanup_plan::renewal_profile::profile_refusal::ProfileRefusal::SelectedVecElementIsOutsideTheCurrentBoundedFlatFamily)));
}
}};}

macro_rules! authenticate_vector {($program:expr,$declaration:expr)=>{{
use crate::hir::{ResolvedTypeDeclarationKind};
let program:&dyn crate::cleanup::ReplayContext=$program;let id:&crate::hir::DeclarationId=$declaration;
'query:{

    if program.type_count() > 32 {
        return Err(crate::cleanup::inventory_builder::typed::InventoryFailure(crate::cleanup::inventory_builder::typed::Kind::Profile(crate::cleanup_plan::renewal_profile::profile_refusal::ProfileRefusal::SelectedVecCurrentDirectoryExceedsItsBoundedTranche)));
    }
    let mut current = None;
    for ordinal in 0..program.type_count() {
        let item = crate::cleanup::inventory_builder::typed::queries::required_closed!(program.type_at(ordinal),SelectedVecCurrentDirectoryIsIncomplete);
        if &item.id == id && current.replace(item).is_some() {
            return Err(crate::cleanup::inventory_builder::typed::InventoryFailure(crate::cleanup::inventory_builder::typed::Kind::Profile(crate::cleanup_plan::renewal_profile::profile_refusal::ProfileRefusal::SelectedVecCurrentDirectoryRepeatsItsIdentity)));
        }
    }
    let current = crate::cleanup::inventory_builder::typed::queries::required_closed!(current,SelectedVecDeclarationIsAbsentFromCurrentTypes);
    let index = program.declarations();
    // Closed error branch, no formatter closure.
    let declaration = crate::cleanup::inventory_builder::typed::queries::required_closed!(index.declaration(id),SelectedVecDeclarationIsNotTheExactCompilerOwnedCollection);
    let parameters = crate::cleanup::inventory_builder::typed::queries::required_closed!(index.type_parameters(id),SelectedVecDeclarationIsNotTheExactCompilerOwnedCollection);
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
        return Err(crate::cleanup::inventory_builder::typed::InventoryFailure(crate::cleanup::inventory_builder::typed::Kind::Profile(crate::cleanup_plan::renewal_profile::profile_refusal::ProfileRefusal::SelectedVecDeclarationIsNotTheExactCompilerOwnedCollection)));
    }
    break 'query ();
}
}};}

macro_rules! flat_fields {($program:expr,$ty:expr)=>{{
use crate::hir::{ResolvedType};
let program:&dyn crate::cleanup::ReplayContext=$program;let ty:&crate::hir::ResolvedType=$ty;
'query:{

    if crate::hir::borrowed_scalar::option_u8::is_type(ty) {
        break 'query None;
    }
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        break 'query None;
    };
    if !arguments.is_empty() {
        return Err(crate::cleanup::inventory_builder::typed::InventoryFailure(crate::cleanup::inventory_builder::typed::Kind::Profile(crate::cleanup_plan::renewal_profile::profile_refusal::ProfileRefusal::SelectedFlatRecordArgumentsAreUnsupported)));
    }
    break 'query crate::cleanup::inventory_builder::typed::queries::flat_declaration!(program,declaration);
}
}};}

macro_rules! flat_declaration {($program:expr,$declaration:expr)=>{{
use crate::hir::{ResolvedType,ResolvedTypeDeclarationKind,DeclarationKind};
let program:&dyn crate::cleanup::ReplayContext=$program;let declaration:&crate::hir::DeclarationId=$declaration;
'query:{

    // Closed error branch, no formatter closure.
    if program.type_count() > 32usize {
        return Err(crate::cleanup::inventory_builder::typed::InventoryFailure(crate::cleanup::inventory_builder::typed::Kind::Profile(crate::cleanup_plan::renewal_profile::profile_refusal::ProfileRefusal::SelectedFlatRecordInventoryHasUnauthenticatedOrUnsupportedFields)));
    }
    let mut matching = None;
    for ordinal in 0..program.type_count() {
        let item = crate::cleanup::inventory_builder::typed::queries::required_closed!(program.type_at(ordinal),SelectedFlatRecordInventoryHasUnauthenticatedOrUnsupportedFields);
        if item.id == *declaration && matching.replace(item).is_some() {
            return Err(crate::cleanup::inventory_builder::typed::InventoryFailure(crate::cleanup::inventory_builder::typed::Kind::Profile(crate::cleanup_plan::renewal_profile::profile_refusal::ProfileRefusal::SelectedFlatRecordInventoryHasUnauthenticatedOrUnsupportedFields)));
        }
    }
    let record = crate::cleanup::inventory_builder::typed::queries::required_closed!(matching,SelectedFlatRecordInventoryHasUnauthenticatedOrUnsupportedFields);
    if !record.type_parameters.is_empty() {
        return Err(crate::cleanup::inventory_builder::typed::InventoryFailure(crate::cleanup::inventory_builder::typed::Kind::Profile(crate::cleanup_plan::renewal_profile::profile_refusal::ProfileRefusal::SelectedFlatRecordInventoryHasUnauthenticatedOrUnsupportedFields)));
    }
    let ResolvedTypeDeclarationKind::Record { fields } = &record.kind else {
        return Err(crate::cleanup::inventory_builder::typed::InventoryFailure(crate::cleanup::inventory_builder::typed::Kind::Profile(crate::cleanup_plan::renewal_profile::profile_refusal::ProfileRefusal::SelectedFlatRecordInventoryHasUnauthenticatedOrUnsupportedFields)));
    };
    if !(1..=8usize).contains(&fields.len()) {
        return Err(crate::cleanup::inventory_builder::typed::InventoryFailure(crate::cleanup::inventory_builder::typed::Kind::Profile(crate::cleanup_plan::renewal_profile::profile_refusal::ProfileRefusal::SelectedFlatRecordInventoryHasUnauthenticatedOrUnsupportedFields)));
    }
    // Establish leaf-only equality before comparing against untrusted indexed
    // fields; no nested type tree is walked by the selected classifier.
    if fields.iter().any(|field| {
        !(crate::hir::is_scalar_resolved_type(&field.ty)
            || matches!(field.ty, ResolvedType::String | ResolvedType::Bytes))
    }) {
        return Err(crate::cleanup::inventory_builder::typed::InventoryFailure(crate::cleanup::inventory_builder::typed::Kind::Profile(crate::cleanup_plan::renewal_profile::profile_refusal::ProfileRefusal::SelectedFlatRecordInventoryHasUnauthenticatedOrUnsupportedFields)));
    }
    let index = program.declarations();
    let declared = crate::cleanup::inventory_builder::typed::queries::required_closed!(index.declaration(declaration),SelectedFlatRecordInventoryHasUnauthenticatedOrUnsupportedFields);
    if declared.kind != DeclarationKind::Record
        || declared.owner.is_some()
        || declared.name != record.name
        || index.record_fields(declaration) != Some(fields.as_slice())
    {
        return Err(crate::cleanup::inventory_builder::typed::InventoryFailure(crate::cleanup::inventory_builder::typed::Kind::Profile(crate::cleanup_plan::renewal_profile::profile_refusal::ProfileRefusal::SelectedFlatRecordInventoryHasUnauthenticatedOrUnsupportedFields)));
    }
    for (position, field) in fields.iter().enumerate() {
        let field_fact = crate::cleanup::inventory_builder::typed::queries::required_closed!(index.declaration(&field.id),SelectedFlatRecordInventoryHasUnauthenticatedOrUnsupportedFields);
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
            return Err(crate::cleanup::inventory_builder::typed::InventoryFailure(crate::cleanup::inventory_builder::typed::Kind::Profile(crate::cleanup_plan::renewal_profile::profile_refusal::ProfileRefusal::SelectedFlatRecordInventoryHasUnauthenticatedOrUnsupportedFields)));
        }
    }
    break 'query Some((declaration,fields.as_slice()));
}
}};}

macro_rules! primitive_drop {($ty:expr)=>{{use crate::hir::ResolvedType;let ty:&crate::hir::ResolvedType=$ty;'query:{
    if crate::hir::borrowed_scalar::option_u8::is_type(ty) {
        break 'query false;
    }
    match ty {
        ResolvedType::Bytes
        | ResolvedType::String
        | ResolvedType::StringMap
        | ResolvedType::OnceFunction
        | ResolvedType::OnceFunctionI64
        | ResolvedType::OnceFunctionI64Pair => true,
        ResolvedType::Unit
        | ResolvedType::I64
        | ResolvedType::I32
        | ResolvedType::Char
        | ResolvedType::U8
        | ResolvedType::Usize
        | ResolvedType::ArrayU8(_)
        | ResolvedType::F32
        | ResolvedType::F64
        | ResolvedType::Bool
        | ResolvedType::Str
        | ResolvedType::SliceU8
        | ResolvedType::MutFunctionI64 => false,
        ResolvedType::Never
        | ResolvedType::Function { .. }
        | ResolvedType::TypeParameter { .. }
        | ResolvedType::Nominal { .. } => return Err(crate::cleanup::inventory_builder::typed::InventoryFailure(crate::cleanup::inventory_builder::typed::Kind::TypeDrop(crate::cleanup_plan::renewal_profile::type_drop_refusal::TypeDropRefusal::UnsupportedCleanupType))),
    }}}};}

macro_rules! needs_drop_in {($program:expr,$ty:expr)=>{{
use crate::hir::ResolvedType;
let program:&dyn crate::cleanup::ReplayContext=$program;let ty:&crate::hir::ResolvedType=$ty;
'query:{
if *ty==ResolvedType::Never {break 'query false;}
if crate::hir::borrowed_scalar::option_u8::is_type(ty) {
    if crate::hir::borrowed_scalar::option_u8::authenticated(program.declarations(),ty) {break 'query false;}
    return Err(crate::cleanup::inventory_builder::typed::InventoryFailure(crate::cleanup::inventory_builder::typed::Kind::TypeDrop(crate::cleanup_plan::renewal_profile::type_drop_refusal::TypeDropRefusal::UnauthenticatedOption)));
}
if crate::cleanup::inventory_builder::typed::queries::vector_element!(program,ty).is_some() {break 'query true;}
if let Some((_,fields))=crate::cleanup::inventory_builder::typed::queries::flat_fields!(program,ty) {break 'query fields.iter().any(|field|matches!(field.ty,ResolvedType::String|ResolvedType::Bytes));}
crate::cleanup::inventory_builder::typed::queries::primitive_drop!(ty)
}
}};}

pub(super) use {
    authenticate_vector, flat_declaration, flat_fields, needs_drop_in, primitive_drop,
    required_closed, vector_element,
};
