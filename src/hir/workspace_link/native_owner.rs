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

pub(super) fn admitted_finalizer(
    parts: &LinkedScalarProjectParts,
    import: &ResolvedImport,
) -> bool {
    parts.types.iter().any(|declaration| admitted_resource(declaration,&parts.interfaces)
        && matches!(&declaration.kind,ResolvedTypeDeclarationKind::Resource {drop} if matches!(&drop.kind,ResolvedResourceDropKind::Imported {import:id,..} if id==&import.id)))
}
