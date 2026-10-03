//! Closed indexed opaque-owner signatures. These are type facts, not native
//! execution authority; the publisher separately replays package/type identity.
use super::*;
use crate::ast::{
    ImportDeclaration, ImportResult, Param, ParamMode, Type, TypeDeclaration, TypeDeclarationKind,
};

#[path = "string.rs"]
mod string;

/// Bind one `(i64) -> Self` constructor or `(self, i64) -> bool` method to a
/// source resource with the Rust type's final name. Returns false for scalars.
pub fn bind_selected_owner_signature(
    import: &mut ImportDeclaration,
    types: &[TypeDeclaration],
    signature: &str,
    index_digest: &str,
    receiver: &str,
) -> Result<bool, Diagnostic> {
    if string::bind(import, signature, index_digest, receiver)? {
        return Ok(true);
    }
    let path = import.rust_path.as_deref().unwrap_or("");
    let Some((type_path, method)) = path.rsplit_once("::") else {
        return Ok(false);
    };
    let owner_result = signature
        .rsplit_once(") -> ")
        .is_some_and(|(_, result)| result == "Self" || result == type_path);
    if receiver != "owned" && !owner_result {
        return Ok(false);
    }
    if !import.native_rust
        || !import.index_selected
        || !valid_digest(index_digest)
        || !valid_rust_api_path(path)
    {
        return Err(error(
            "SPX-B142",
            "selected owner import has invalid index identity",
            import.span,
        ));
    }
    let type_name = type_path.rsplit("::").next().unwrap_or("");
    let matches = types
        .iter()
        .filter(|ty| {
            ty.name == type_name && matches!(ty.kind, TypeDeclarationKind::Resource { .. })
        })
        .count();
    if matches != 1 {
        return Err(error(
            "SPX-B145",
            "selected Rust owner requires one matching declared resource",
            import.span,
        ));
    }
    let rest = signature
        .strip_prefix("fn ")
        .and_then(|text| text.strip_prefix(method))
        .and_then(|text| text.strip_prefix('('));
    let (args, result) = rest
        .and_then(|text| text.split_once(") -> "))
        .ok_or_else(|| {
            error(
                "SPX-B143",
                "selected owner signature disagrees with Rust path",
                import.span,
            )
        })?;
    let argument = match receiver {
        "none" if result == "Self" || result == type_path => args,
        "owned" if result == "bool" => args.strip_prefix("self, ").ok_or_else(|| {
            error(
                "SPX-B144",
                "selected owner receiver is invalid",
                import.span,
            )
        })?,
        _ => {
            return Err(error(
                "SPX-B144",
                "selected owner receiver is unsupported",
                import.span,
            ))
        }
    };
    let (name, ty) = argument.split_once(": ").ok_or_else(|| {
        error(
            "SPX-B145",
            "selected owner argument is unsupported",
            import.span,
        )
    })?;
    if !valid_alias(name) || ty != "i64" {
        return Err(error(
            "SPX-B145",
            "selected owner requires one i64 argument",
            import.span,
        ));
    }
    let mut params = Vec::new();
    if receiver == "owned" {
        params.push(Param {
            name: "receiver".into(),
            mode: ParamMode::Own,
            ty: Type::Named {
                name: type_name.into(),
                arguments: Vec::new(),
            },
            span: import.span,
        });
    }
    params.push(Param {
        name: "arg0".into(),
        mode: ParamMode::Value,
        ty: Type::I64,
        span: import.span,
    });
    import.params = params;
    import.result = if receiver == "owned" {
        ImportResult::Bool
    } else {
        ImportResult::OwnedResource {
            name: type_name.into(),
        }
    };
    import.selected_signature = Some(signature.into());
    import.selected_index_digest = Some(index_digest.into());
    import.selected_receiver = (receiver == "owned").then(|| "owned".into());
    Ok(true)
}
