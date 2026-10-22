//! Full exact selected type/drop closure expanded in an existing paid caller.
//! Original public classifiers and ordinary type/drop remain independent.
macro_rules! required_closed {
    ($permit:ident,$function:ident,$budget:ident,$value:expr,$value_type:ty,$reason:ident) => {{
        // Scrutinee result plus the successful match binding are distinct
        // source roles. Pay before either is formed, for this actual query.
        $budget.reserve_array::<(Option<$value_type>, $value_type)>(
            $function,
            1,
            "selected closed metadata receiving",
        )?;
        $budget.charge($function, 2, "selected closed metadata receiving")?;
        match $value {
            Some(value) => value,
            None => return Err($permit.refuse(
                crate::cleanup_plan::renewal_profile::child_refusal::Refusal::Profile(
                    crate::cleanup_plan::renewal_profile::profile_refusal::ProfileRefusal::$reason,
                ),
            )),
        }
    }};
}

macro_rules! vector_element {($permit:ident,$function:ident,$budget:ident,$program:expr,$ty:expr)=>{{
$budget.reserve_array::<(&dyn crate::cleanup::ReplayContext, &crate::hir::ResolvedType, &crate::hir::DeclarationId, &Vec<crate::hir::ResolvedType>, &crate::hir::ResolvedType, Option<&crate::hir::ResolvedType>)>($function,1,"selected vector_element query activation")?;
$budget.charge($function,6,"selected vector_element query activation")?;

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
    crate::cleanup_plan::replay::inventory_coverage::paid::queries::authenticate_vector!($permit,$function,$budget,program,declaration);
    let [element] = arguments.as_slice() else {
        return Err($permit.refuse(crate::cleanup_plan::renewal_profile::child_refusal::Refusal::Profile(crate::cleanup_plan::renewal_profile::profile_refusal::ProfileRefusal::SelectedVecHasInvalidTypeArgumentCount)));
    };
    if crate::vec_ops::resolved_vec_element_is_admitted(element) || *element == ResolvedType::String
    {
        break 'query Some(element);
    }
    if matches!(element, ResolvedType::Nominal { arguments, .. } if arguments.is_empty()) {
        crate::cleanup_plan::replay::inventory_coverage::paid::queries::flat_fields!($permit,$function,$budget,program,element);
        if crate::hir::copy_record_collection::admitted(program.declarations(), element)
            || crate::hir::owned_leaf_collection::runtime_element(program.declarations(), element)
        {
            break 'query Some(element);
        }
    }
    return Err($permit.refuse(crate::cleanup_plan::renewal_profile::child_refusal::Refusal::Profile(crate::cleanup_plan::renewal_profile::profile_refusal::ProfileRefusal::SelectedVecElementIsOutsideTheCurrentBoundedFlatFamily)));
}
}};}

macro_rules! authenticate_vector {($permit:ident,$function:ident,$budget:ident,$program:expr,$declaration:expr)=>{{
$budget.reserve_array::<(&dyn crate::cleanup::ReplayContext, &crate::hir::DeclarationId, Option<&crate::hir::ResolvedTypeDeclaration>, usize, &crate::hir::ResolvedTypeDeclaration, &crate::hir::ResolvedTypeDeclaration, &crate::hir::DeclarationIndex, &crate::hir::Declaration, &[crate::hir::ResolvedTypeParameterDeclaration], std::ops::Range<usize>)>($function,1,"selected authenticate_vector query activation")?;
$budget.charge($function,10,"selected authenticate_vector query activation")?;

use crate::hir::{ResolvedTypeDeclarationKind};
let program:&dyn crate::cleanup::ReplayContext=$program;let id:&crate::hir::DeclarationId=$declaration;
'query:{

    if program.type_count() > 32 {
        return Err($permit.refuse(crate::cleanup_plan::renewal_profile::child_refusal::Refusal::Profile(crate::cleanup_plan::renewal_profile::profile_refusal::ProfileRefusal::SelectedVecCurrentDirectoryExceedsItsBoundedTranche)));
    }
    let mut current = None;
    for ordinal in 0..program.type_count() {
        let item = crate::cleanup_plan::replay::inventory_coverage::paid::queries::required_closed!($permit,$function,$budget,program.type_at(ordinal),&crate::hir::ResolvedTypeDeclaration,SelectedVecCurrentDirectoryIsIncomplete);
        if &item.id == id && current.replace(item).is_some() {
            return Err($permit.refuse(crate::cleanup_plan::renewal_profile::child_refusal::Refusal::Profile(crate::cleanup_plan::renewal_profile::profile_refusal::ProfileRefusal::SelectedVecCurrentDirectoryRepeatsItsIdentity)));
        }
    }
    let current = crate::cleanup_plan::replay::inventory_coverage::paid::queries::required_closed!($permit,$function,$budget,current,&crate::hir::ResolvedTypeDeclaration,SelectedVecDeclarationIsAbsentFromCurrentTypes);
    let index = program.declarations();
    // Closed error branch, no formatter closure.
    let declaration = crate::cleanup_plan::replay::inventory_coverage::paid::queries::required_closed!($permit,$function,$budget,index.declaration(id),&crate::hir::Declaration,SelectedVecDeclarationIsNotTheExactCompilerOwnedCollection);
    let parameters = crate::cleanup_plan::replay::inventory_coverage::paid::queries::required_closed!($permit,$function,$budget,index.type_parameters(id),&[crate::hir::ResolvedTypeParameterDeclaration],SelectedVecDeclarationIsNotTheExactCompilerOwnedCollection);
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
        return Err($permit.refuse(crate::cleanup_plan::renewal_profile::child_refusal::Refusal::Profile(crate::cleanup_plan::renewal_profile::profile_refusal::ProfileRefusal::SelectedVecDeclarationIsNotTheExactCompilerOwnedCollection)));
    }
    break 'query ();
}
}};}

macro_rules! flat_fields {($permit:ident,$function:ident,$budget:ident,$program:expr,$ty:expr)=>{{
$budget.reserve_array::<(&dyn crate::cleanup::ReplayContext, &crate::hir::ResolvedType, &crate::hir::DeclarationId, &Vec<crate::hir::ResolvedType>, Option<(&crate::hir::DeclarationId, &[crate::hir::ResolvedFieldDeclaration])>)>($function,1,"selected flat_fields query activation")?;
$budget.charge($function,5,"selected flat_fields query activation")?;

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
        return Err($permit.refuse(crate::cleanup_plan::renewal_profile::child_refusal::Refusal::Profile(crate::cleanup_plan::renewal_profile::profile_refusal::ProfileRefusal::SelectedFlatRecordArgumentsAreUnsupported)));
    }
    break 'query crate::cleanup_plan::replay::inventory_coverage::paid::queries::flat_declaration!($permit,$function,$budget,program,declaration);
}
}};}

macro_rules! flat_declaration {($permit:ident,$function:ident,$budget:ident,$program:expr,$declaration:expr)=>{{
$budget.reserve_array::<(&dyn crate::cleanup::ReplayContext, &crate::hir::DeclarationId, Option<&crate::hir::ResolvedTypeDeclaration>, usize, &crate::hir::ResolvedTypeDeclaration, &crate::hir::ResolvedTypeDeclaration, &Vec<crate::hir::ResolvedFieldDeclaration>, &crate::hir::DeclarationIndex, &crate::hir::Declaration, usize, &crate::hir::ResolvedFieldDeclaration, &crate::hir::Declaration, std::ops::Range<usize>, std::iter::Enumerate<std::slice::Iter<crate::hir::ResolvedFieldDeclaration>>, Option<(&crate::hir::DeclarationId, &[crate::hir::ResolvedFieldDeclaration])>)>($function,1,"selected flat_declaration query activation")?;
$budget.charge($function,15,"selected flat_declaration query activation")?;

use crate::hir::{ResolvedType,ResolvedTypeDeclarationKind,DeclarationKind};
let program:&dyn crate::cleanup::ReplayContext=$program;let declaration:&crate::hir::DeclarationId=$declaration;
'query:{

    // Closed error branch, no formatter closure.
    if program.type_count() > 32usize {
        return Err($permit.refuse(crate::cleanup_plan::renewal_profile::child_refusal::Refusal::Profile(crate::cleanup_plan::renewal_profile::profile_refusal::ProfileRefusal::SelectedFlatRecordInventoryHasUnauthenticatedOrUnsupportedFields)));
    }
    let mut matching = None;
    for ordinal in 0..program.type_count() {
        let item = crate::cleanup_plan::replay::inventory_coverage::paid::queries::required_closed!($permit,$function,$budget,program.type_at(ordinal),&crate::hir::ResolvedTypeDeclaration,SelectedFlatRecordInventoryHasUnauthenticatedOrUnsupportedFields);
        if item.id == *declaration && matching.replace(item).is_some() {
            return Err($permit.refuse(crate::cleanup_plan::renewal_profile::child_refusal::Refusal::Profile(crate::cleanup_plan::renewal_profile::profile_refusal::ProfileRefusal::SelectedFlatRecordInventoryHasUnauthenticatedOrUnsupportedFields)));
        }
    }
    let record = crate::cleanup_plan::replay::inventory_coverage::paid::queries::required_closed!($permit,$function,$budget,matching,&crate::hir::ResolvedTypeDeclaration,SelectedFlatRecordInventoryHasUnauthenticatedOrUnsupportedFields);
    if !record.type_parameters.is_empty() {
        return Err($permit.refuse(crate::cleanup_plan::renewal_profile::child_refusal::Refusal::Profile(crate::cleanup_plan::renewal_profile::profile_refusal::ProfileRefusal::SelectedFlatRecordInventoryHasUnauthenticatedOrUnsupportedFields)));
    }
    let ResolvedTypeDeclarationKind::Record { fields } = &record.kind else {
        return Err($permit.refuse(crate::cleanup_plan::renewal_profile::child_refusal::Refusal::Profile(crate::cleanup_plan::renewal_profile::profile_refusal::ProfileRefusal::SelectedFlatRecordInventoryHasUnauthenticatedOrUnsupportedFields)));
    };
    if !(1..=8usize).contains(&fields.len()) {
        return Err($permit.refuse(crate::cleanup_plan::renewal_profile::child_refusal::Refusal::Profile(crate::cleanup_plan::renewal_profile::profile_refusal::ProfileRefusal::SelectedFlatRecordInventoryHasUnauthenticatedOrUnsupportedFields)));
    }
    // Establish leaf-only equality before comparing against untrusted indexed
    // fields; no nested type tree is walked by the selected classifier.
    if fields.iter().any(|field| {
        !(crate::hir::is_scalar_resolved_type(&field.ty)
            || matches!(field.ty, ResolvedType::String | ResolvedType::Bytes))
    }) {
        return Err($permit.refuse(crate::cleanup_plan::renewal_profile::child_refusal::Refusal::Profile(crate::cleanup_plan::renewal_profile::profile_refusal::ProfileRefusal::SelectedFlatRecordInventoryHasUnauthenticatedOrUnsupportedFields)));
    }
    let index = program.declarations();
    let declared = crate::cleanup_plan::replay::inventory_coverage::paid::queries::required_closed!($permit,$function,$budget,index.declaration(declaration),&crate::hir::Declaration,SelectedFlatRecordInventoryHasUnauthenticatedOrUnsupportedFields);
    if declared.kind != DeclarationKind::Record
        || declared.owner.is_some()
        || declared.name != record.name
        || index.record_fields(declaration) != Some(fields.as_slice())
    {
        return Err($permit.refuse(crate::cleanup_plan::renewal_profile::child_refusal::Refusal::Profile(crate::cleanup_plan::renewal_profile::profile_refusal::ProfileRefusal::SelectedFlatRecordInventoryHasUnauthenticatedOrUnsupportedFields)));
    }
    for (position, field) in fields.iter().enumerate() {
        let field_fact = crate::cleanup_plan::replay::inventory_coverage::paid::queries::required_closed!($permit,$function,$budget,index.declaration(&field.id),&crate::hir::Declaration,SelectedFlatRecordInventoryHasUnauthenticatedOrUnsupportedFields);
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
            return Err($permit.refuse(crate::cleanup_plan::renewal_profile::child_refusal::Refusal::Profile(crate::cleanup_plan::renewal_profile::profile_refusal::ProfileRefusal::SelectedFlatRecordInventoryHasUnauthenticatedOrUnsupportedFields)));
        }
    }
    break 'query Some((declaration,fields.as_slice()));
}
}};}

macro_rules! primitive_drop {($permit:ident,$function:ident,$budget:ident,$ty:expr)=>{{
$budget.reserve_array::<(&crate::hir::ResolvedType, bool)>($function,1,"selected primitive_drop query activation")?;
$budget.charge($function,2,"selected primitive_drop query activation")?;
use crate::hir::ResolvedType;let ty:&crate::hir::ResolvedType=$ty;'query:{
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
        | ResolvedType::Nominal { .. } => return Err($permit.refuse(crate::cleanup_plan::renewal_profile::child_refusal::Refusal::TypeDrop(crate::cleanup_plan::renewal_profile::type_drop_refusal::TypeDropRefusal::UnsupportedCleanupType))),
    }}}};}

macro_rules! needs_drop_in {($permit:ident,$function:ident,$budget:ident,$program:expr,$ty:expr)=>{{
$budget.reserve_array::<(&dyn crate::cleanup::ReplayContext, &crate::hir::ResolvedType, &crate::hir::DeclarationId, &[crate::hir::ResolvedFieldDeclaration], bool)>($function,1,"selected needs_drop_in query activation")?;
$budget.charge($function,5,"selected needs_drop_in query activation")?;

use crate::hir::ResolvedType;
let program:&dyn crate::cleanup::ReplayContext=$program;let ty:&crate::hir::ResolvedType=$ty;
'query:{
if *ty==ResolvedType::Never {break 'query false;}
if crate::hir::borrowed_scalar::option_u8::is_type(ty) {
    if crate::hir::borrowed_scalar::option_u8::authenticated(program.declarations(),ty) {break 'query false;}
    return Err($permit.refuse(crate::cleanup_plan::renewal_profile::child_refusal::Refusal::TypeDrop(crate::cleanup_plan::renewal_profile::type_drop_refusal::TypeDropRefusal::UnauthenticatedOption)));
}
if crate::cleanup_plan::replay::inventory_coverage::paid::queries::vector_element!($permit,$function,$budget,program,ty).is_some() {break 'query true;}
if let Some((_,fields))=crate::cleanup_plan::replay::inventory_coverage::paid::queries::flat_fields!($permit,$function,$budget,program,ty) {break 'query fields.iter().any(|field|matches!(field.ty,ResolvedType::String|ResolvedType::Bytes));}
crate::cleanup_plan::replay::inventory_coverage::paid::queries::primitive_drop!($permit,$function,$budget,ty)
}
}};}

// Expanded sibling callers also resolve these qualified nested macro paths.
// Keep the entire transitive classifier closure private to replay.
pub(in crate::cleanup_plan::replay) use {
    authenticate_vector, flat_declaration, flat_fields, needs_drop_in, primitive_drop,
    required_closed, vector_element,
};
