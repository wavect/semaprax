//! Exact selected standard String signatures; target compilation separately
//! proves that a similarly named Rust type cannot substitute for std::String.
use super::*;

pub(super) fn bind(
    import: &mut ImportDeclaration,
    signature: &str,
    index_digest: &str,
    receiver: &str,
) -> Result<bool, Diagnostic> {
    const STRING: &str = "alloc::string::String";
    if !signature.contains(STRING) {
        return Ok(false);
    }
    let path = import.rust_path.as_deref().unwrap_or("");
    if !import.native_rust
        || !import.index_selected
        || !valid_digest(index_digest)
        || !valid_rust_api_path(path)
    {
        return Err(error(
            "SPX-B142",
            "selected String import has invalid index identity",
            import.span,
        ));
    }
    if receiver != "none" {
        return Err(error(
            "SPX-B144",
            "selected String profile requires a free function",
            import.span,
        ));
    }
    let name = path.rsplit("::").next().unwrap_or("");
    let (params, result) = signature
        .strip_prefix("fn ")
        .and_then(|s| s.strip_prefix(name))
        .and_then(|s| s.strip_prefix('('))
        .and_then(|s| s.split_once(") -> "))
        .ok_or_else(|| {
            error(
                "SPX-B143",
                "selected String signature disagrees with Rust path",
                import.span,
            )
        })?;
    let expected = if result == STRING {
        vec!["i64"]
    } else if result == "bool" {
        vec![STRING, "i64"]
    } else {
        return Err(error(
            "SPX-B145",
            "selected String result is outside the bounded profile",
            import.span,
        ));
    };
    let args = params.split(", ").collect::<Vec<_>>();
    if args.len() != expected.len()
        || args.iter().zip(&expected).any(|(arg, expected)| {
            arg.split_once(": ")
                .is_none_or(|(name, ty)| !valid_alias(name) || ty != *expected)
        })
    {
        return Err(error(
            "SPX-B145",
            "selected String parameters are outside the bounded profile",
            import.span,
        ));
    }
    import.params = expected
        .iter()
        .enumerate()
        .map(|(i, ty)| Param {
            name: format!("arg{i}"),
            mode: if *ty == STRING {
                ParamMode::Own
            } else {
                ParamMode::Value
            },
            ty: if *ty == STRING {
                Type::String
            } else {
                Type::I64
            },
            span: import.span,
        })
        .collect();
    import.result = if result == STRING {
        ImportResult::OwnedString
    } else {
        ImportResult::Bool
    };
    import.selected_signature = Some(signature.into());
    import.selected_index_digest = Some(index_digest.into());
    import.selected_receiver = None;
    Ok(true)
}
