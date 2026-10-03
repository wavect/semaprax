//! Exact Url resource reachability, including its imported finalizer.
use super::*;
pub(super) fn admitted_resource(
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
    if declaration.name != "Url" || !declaration.type_parameters.is_empty() {
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
            && matches!(import.parameters.as_slice(), [parameter] if parameter.ty == resource
            && parameter.ownership == OwnershipMode::Own && parameter.consumes_on_failure)
    });
    let Some(constructor) = imports().find(|import| import.native_rust && import.index_selected
        && import.rust_path.as_deref() == Some("url_alias::Url::parse") && import.selected_receiver.is_none()
        && import.selected_index_digest.as_deref().is_some_and(|digest| !digest.is_empty())
        && matches!(&import.result.kind, ResolvedImportResultKind::OwnedResultResourceI64 { resource: id } if id == &declaration.id)
        && matches!(import.parameters.as_slice(), [parameter] if parameter.ty == ResolvedType::String && parameter.ownership == OwnershipMode::Borrow)) else { return false; };
    destructor && imports().any(|import| import.native_rust && import.index_selected
        && import.rust_path.as_deref() == Some("url_alias::Url::as_str")
        && import.selected_receiver.as_deref() == Some("shared")
        && import.selected_index_digest == constructor.selected_index_digest
        && matches!(&import.result.kind, ResolvedImportResultKind::BorrowedStr { resource: id } if id == &declaration.id)
        && matches!(import.parameters.as_slice(), [receiver] if receiver.ty == resource && receiver.ownership == OwnershipMode::Borrow))
}
