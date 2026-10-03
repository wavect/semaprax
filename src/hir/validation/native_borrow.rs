//! Closed native borrow signatures, independently checked against HIR identity.
use super::*;
pub(super) fn admitted(program: &ResolvedProgram, import: &ResolvedImport) -> bool {
    if matches!(
        import.rust_path.as_deref(),
        Some("url_alias::Url::parse" | "url_alias::Url::as_str")
    ) {
        return import.native_rust
            && import.index_selected
            && program.types.iter().any(|declaration| {
                declaration.name == "Url"
                    && crate::hir::workspace_link::native_owner::admitted_ri06_regex_resource(
                        declaration,
                        &program.interfaces,
                    )
                    && match (&import.result.kind, import.parameters.as_slice()) {
                        (
                            ResolvedImportResultKind::OwnedResultResourceI64 { resource },
                            [parameter],
                        ) => {
                            resource == &declaration.id
                                && import.rust_path.as_deref() == Some("url_alias::Url::parse")
                                && parameter.ty == ResolvedType::String
                                && parameter.ownership == OwnershipMode::Borrow
                        }
                        (ResolvedImportResultKind::BorrowedStr { resource }, [receiver]) => {
                            resource == &declaration.id
                                && import.rust_path.as_deref() == Some("url_alias::Url::as_str")
                                && receiver.ownership == OwnershipMode::Borrow
                                && receiver.ty
                                    == (ResolvedType::Nominal {
                                        declaration: resource.clone(),
                                        arguments: Vec::new(),
                                    })
                        }
                        _ => false,
                    }
            });
    }
    import.index_selected
        && match (
            import.rust_path.as_deref(),
            &import.result.kind,
            import.parameters.as_slice(),
        ) {
            (
                Some("regex_alias::Regex::new"),
                ResolvedImportResultKind::OwnedResultResourceI64 { .. },
                [parameter],
            ) => {
                parameter.ownership == OwnershipMode::Borrow && parameter.ty == ResolvedType::String
            }
            (
                Some("regex_alias::Regex::is_match"),
                ResolvedImportResultKind::Bool,
                [receiver, text],
            ) => {
                receiver.ownership == OwnershipMode::Borrow
                    && matches!(receiver.ty, ResolvedType::Nominal { ref declaration, ref arguments }
                                    if arguments.is_empty() && program.declarations.declaration(declaration)
                                        .is_some_and(|item| item.kind == DeclarationKind::Resource))
                    && text.ownership == OwnershipMode::Borrow
                    && text.ty == ResolvedType::String
            }
            _ => false,
        }
}
