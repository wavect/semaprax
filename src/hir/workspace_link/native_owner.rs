//! Admission for the destructor belonging to an indexed opaque Rust owner.
//! The ordinary scalar linker still refuses unrelated interface imports.
use super::*;

pub(in crate::hir) fn admitted_resource(
    declaration: &ResolvedTypeDeclaration,
    interfaces: &[ResolvedInterface],
) -> bool {
    let ResolvedTypeDeclarationKind::Resource { drop } = &declaration.kind else {
        return false;
    };
    let ResolvedResourceDropKind::Imported {
        import: finalizer,
        import_key,
    } = &drop.kind
    else {
        return false;
    };
    if !declaration.type_parameters.is_empty() {
        return false;
    }
    let resource = ResolvedType::Nominal {
        declaration: declaration.id.clone(),
        arguments: Vec::new(),
    };
    let imports = || interfaces.iter().flat_map(|i| &i.imports);
    let destructor = imports().any(|i| {
        &i.id == finalizer
            && &i.import_key == import_key
            && !i.native_rust
            && !i.index_selected
            && i.effects.is_empty()
            && i.failure == ResolvedImportFailure::Infallible
            && i.result.kind == ResolvedImportResultKind::Unit
            && i.parameters.len() == 1
            && i.parameters[0].ty == resource
            && i.parameters[0].ownership == OwnershipMode::Own
            && i.parameters[0].consumes_on_failure
    });
    let constructor = imports().find(|i| i.native_rust && i.index_selected
        && i.selected_index_digest.is_some() && i.selected_receiver.is_none()
        && matches!(&i.result.kind,ResolvedImportResultKind::OwnedResource {resource} if resource==&declaration.id)
        && i.parameters.len()==1 && i.parameters[0].ty==ResolvedType::I64 && i.parameters[0].ownership==OwnershipMode::Value);
    let Some(constructor) = constructor else {
        return false;
    };
    let Some((rust_type, _)) = constructor
        .rust_path
        .as_deref()
        .and_then(|p| p.rsplit_once("::"))
    else {
        return false;
    };
    let method = imports().any(|i| {
        i.native_rust
            && i.index_selected
            && i.selected_index_digest == constructor.selected_index_digest
            && i.rust_path
                .as_deref()
                .and_then(|p| p.rsplit_once("::"))
                .map(|p| p.0)
                == Some(rust_type)
            && i.selected_receiver.as_deref() == Some("owned")
            && i.parameters.len() == 2
            && i.parameters[0].ty == resource
            && i.parameters[0].ownership == OwnershipMode::Own
            && i.parameters[1].ty == ResolvedType::I64
            && i.parameters[1].ownership == OwnershipMode::Value
            && i.result.kind == ResolvedImportResultKind::Bool
    });
    destructor && method
}

/// The RI-06 registry route admits one exact indexed Regex owner shape.
/// This authenticates linked type reachability and its finalizer; it does not
/// admit borrowed methods as ordinary scalar callable helpers.
pub(crate) fn admitted_ri06_regex_resource(
    declaration: &ResolvedTypeDeclaration,
    interfaces: &[ResolvedInterface],
) -> bool {
    let ResolvedTypeDeclarationKind::Resource { drop } = &declaration.kind else {
        return false;
    };
    let ResolvedResourceDropKind::Imported {
        import: finalizer,
        import_key,
    } = &drop.kind
    else {
        return false;
    };
    if !declaration.type_parameters.is_empty() || declaration.name != "Regex" {
        return false;
    }
    let resource = ResolvedType::Nominal {
        declaration: declaration.id.clone(),
        arguments: Vec::new(),
    };
    let imports = || interfaces.iter().flat_map(|interface| &interface.imports);
    let destructor = imports().any(|import| {
        &import.id == finalizer
            && &import.import_key == import_key
            && !import.native_rust
            && !import.index_selected
            && import.effects.is_empty()
            && import.failure == ResolvedImportFailure::Infallible
            && import.result.kind == ResolvedImportResultKind::Unit
            && matches!(import.parameters.as_slice(), [parameter]
                if parameter.ty == resource
                    && parameter.ownership == OwnershipMode::Own
                    && parameter.consumes_on_failure)
    });
    let constructor = imports().find(|import| {
        import.native_rust
            && import.index_selected
            && import
                .selected_index_digest
                .as_deref()
                .is_some_and(|digest| !digest.is_empty())
            && import.selected_receiver.is_none()
            && import.rust_path.as_deref() == Some("regex_alias::Regex::new")
            && matches!(&import.result.kind, ResolvedImportResultKind::OwnedResultResourceI64 { resource: id } if id == &declaration.id)
            && matches!(import.parameters.as_slice(), [parameter]
                if parameter.ty == ResolvedType::String
                    && parameter.ownership == OwnershipMode::Borrow)
    });
    let Some(constructor) = constructor else {
        return false;
    };
    let method = imports().any(|import| {
        import.native_rust
            && import.index_selected
            && import.selected_index_digest == constructor.selected_index_digest
            && import.selected_receiver.as_deref() == Some("shared")
            && import.rust_path.as_deref() == Some("regex_alias::Regex::is_match")
            && import.result.kind == ResolvedImportResultKind::Bool
            && matches!(import.parameters.as_slice(), [receiver, text]
                if receiver.ty == resource
                    && receiver.ownership == OwnershipMode::Borrow
                    && text.ty == ResolvedType::String
                    && text.ownership == OwnershipMode::Borrow)
    });
    destructor && method
}

pub(super) fn admitted_finalizer(
    parts: &LinkedScalarProjectParts,
    import: &ResolvedImport,
) -> bool {
    parts.types.iter().any(|declaration| (admitted_resource(declaration,&parts.interfaces)
        || admitted_ri06_regex_resource(declaration, &parts.interfaces))
        && matches!(&declaration.kind,ResolvedTypeDeclarationKind::Resource {drop} if matches!(&drop.kind,ResolvedResourceDropKind::Imported {import:id,..} if id==&import.id)))
}

/// Only internal helpers can carry the selected owner across Semaprax calls.
/// The public scalar entry remains scalar, and every nominal signature type
/// must be the resource authenticated by the same selected Rust import pair.
pub(crate) fn admitted_helper(
    types: &[ResolvedTypeDeclaration],
    interfaces: &[ResolvedInterface],
    function: &ResolvedFunction,
) -> bool {
    let owner = |ty: &ResolvedType| {
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = ty
        else {
            return false;
        };
        arguments.is_empty()
            && types
                .iter()
                .any(|ty| &ty.id == declaration && admitted_resource(ty, interfaces))
    };
    function.effects.is_empty()
        && function.params.len() <= 8
        && function.params.iter().all(|p| {
            p.ty == ResolvedType::I64 && p.ownership == OwnershipMode::Value
                || p.ownership == OwnershipMode::Own && owner(&p.ty)
        })
        && (matches!(function.return_type, ResolvedType::I64 | ResolvedType::Bool)
            || owner(&function.return_type))
        && (owner(&function.return_type) || function.params.iter().any(|p| owner(&p.ty)))
}

fn regex_result_resource(ty: &ResolvedType) -> Option<&DeclarationId> {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return None;
    };
    let [ResolvedType::Nominal {
        declaration: resource,
        arguments: resource_arguments,
    }, ResolvedType::I64] = arguments.as_slice()
    else {
        return None;
    };
    (declaration.as_str() == crate::prelude::RESULT_ID && resource_arguments.is_empty())
        .then_some(resource)
}

pub(crate) fn admitted_ri06_regex_result(program: &ResolvedProgram, ty: &ResolvedType) -> bool {
    let Some(resource) = regex_result_resource(ty) else {
        return false;
    };
    program
        .types
        .iter()
        .any(|d| &d.id == resource && admitted_ri06_regex_resource(d, &program.interfaces))
}

pub(in crate::hir) fn resolver_ri06_regex_result(
    program: &crate::ast::Program,
    ty: &ResolvedType,
) -> bool {
    let Some(resource) = regex_result_resource(ty) else {
        return false;
    };
    let Some(declaration) = program
        .types
        .iter()
        .find(|d| d.stable_id == resource.as_str())
    else {
        return false;
    };
    let source_ty = crate::ast::Type::Named {
        name: "Result".into(),
        arguments: vec![
            crate::ast::Type::Named {
                name: declaration.name.clone(),
                arguments: vec![],
            },
            crate::ast::Type::I64,
        ],
    };
    crate::native_rust_binding::admitted_regex_result(program, &source_ty)
}
