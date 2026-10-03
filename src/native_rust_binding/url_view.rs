//! Exact selected Url signatures. These facts grant no tool or package authority.
use super::*;
use crate::ast::{
    ImportDeclaration, ImportResult, Param, ParamMode, Program, Type, TypeDeclaration,
    TypeDeclarationKind,
};

pub fn bind_selected_url_signature(
    import: &mut ImportDeclaration,
    types: &[TypeDeclaration],
    signature: &str,
    index_digest: &str,
    receiver: &str,
) -> Result<bool, Diagnostic> {
    let path = import.rust_path.as_deref().unwrap_or("");
    if !matches!(path, "url_alias::Url::parse" | "url_alias::Url::as_str") {
        return Ok(false);
    }
    if !import.native_rust || !import.index_selected || !valid_digest(index_digest) {
        return Err(error(
            "SPX-B142",
            "selected Url import has invalid index identity",
            import.span,
        ));
    }
    if types
        .iter()
        .filter(|ty| ty.name == "Url" && matches!(ty.kind, TypeDeclarationKind::Resource { .. }))
        .count()
        != 1
    {
        return Err(error(
            "SPX-B145",
            "selected Url import requires one matching declared resource",
            import.span,
        ));
    }
    let parse = path == "url_alias::Url::parse";
    if if parse {
        receiver != "none"
            || signature != "fn parse(input: &str) -> core::result::Result<Self, url::ParseError>"
    } else {
        receiver != "shared" || signature != "fn as_str(&self) -> &str"
    } {
        return Err(error(
            "SPX-B145",
            "selected Url signature is outside the receiver-tied view profile",
            import.span,
        ));
    }
    import.params = vec![Param {
        name: if parse { "text" } else { "receiver" }.into(),
        mode: ParamMode::Borrow,
        ty: if parse {
            Type::String
        } else {
            Type::Named {
                name: "Url".into(),
                arguments: Vec::new(),
            }
        },
        span: import.span,
    }];
    import.result = if parse {
        ImportResult::OwnedResultResourceI64 { name: "Url".into() }
    } else {
        ImportResult::BorrowedStr {
            owner: "Url".into(),
        }
    };
    import.selected_signature = Some(signature.into());
    import.selected_index_digest = Some(index_digest.into());
    import.selected_receiver = (!parse).then(|| "shared".into());
    Ok(true)
}

pub(crate) fn admitted_url_view(import: &ImportDeclaration) -> bool {
    import.native_rust
        && import.index_selected
        && import.rust_path.as_deref() == Some("url_alias::Url::as_str")
        && import
            .selected_index_digest
            .as_deref()
            .is_some_and(valid_digest)
        && import.selected_signature.as_deref() == Some("fn as_str(&self) -> &str")
        && import.selected_receiver.as_deref() == Some("shared")
        && matches!(&import.result, ImportResult::BorrowedStr { owner } if owner == "Url")
        && matches!(import.params.as_slice(), [receiver] if receiver.mode == ParamMode::Borrow
            && receiver.ty == (Type::Named { name: "Url".into(), arguments: Vec::new() }))
}

pub(crate) fn admitted_url_result(program: &Program, ty: &Type) -> bool {
    let imports = || {
        program
            .interfaces
            .iter()
            .flat_map(|interface| &interface.imports)
    };
    let Some(constructor) = imports().find(|import| import.native_rust && import.index_selected
        && import.rust_path.as_deref() == Some("url_alias::Url::parse")
        && import.selected_index_digest.as_deref().is_some_and(valid_digest)
        && import.selected_signature.as_deref() == Some("fn parse(input: &str) -> core::result::Result<Self, url::ParseError>")
        && import.selected_receiver.is_none()
        && matches!(&import.result, ImportResult::OwnedResultResourceI64 { name } if name == "Url")
        && import.result.value_type() == *ty
        && matches!(import.params.as_slice(), [input] if input.mode == ParamMode::Borrow && input.ty == Type::String)) else { return false; };
    imports().any(|import| {
        admitted_url_view(import)
            && import.selected_index_digest == constructor.selected_index_digest
    })
}
