//! Source-bound selection for the narrow native Rust scalar bridge.
//!
//! A prepared API index is discovery data. This module compares a selected
//! record with checked HIR and produces an inert binding plan. It neither
//! grants execution authority nor turns an index signature into Rust code.

use sha2::{Digest, Sha256};
use std::fmt::Write as _;

use crate::ast::Span;
use crate::diagnostic::Diagnostic;
use crate::hir::{OwnershipMode, ResolvedImport, ResolvedImportResultKind, ResolvedType};

const SYMBOL_DOMAIN: &[u8] = b"semaprax.native-rust-binding.symbol.v1\0";

/// Exact, already replayed metadata for one selected index item. The caller
/// must authenticate the package closure and pass the index's exact digest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectedRustItem<'a> {
    pub cargo_alias: &'a str,
    pub package_name: &'a str,
    pub package_version: &'a str,
    pub package_source_sha256: &'a str,
    pub index_digest: &'a str,
    pub target: &'a str,
    pub feature_digest: &'a str,
    pub path: &'a str,
    pub kind: &'a str,
    pub receiver: &'a str,
    pub signature: &'a str,
    pub supported: bool,
}

/// Inert data retained separately from a Semaprax declaration identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalarBindingPlan {
    pub import_id: String,
    pub cargo_alias: String,
    pub package_name: String,
    pub package_version: String,
    pub package_source_sha256: String,
    pub index_digest: String,
    pub target: String,
    pub feature_digest: String,
    pub rust_path: String,
    pub signature: String,
    /// `shared` means an `i64` carrier is converted to a temporary Rust
    /// receiver with `From<i64>` before a checked `&self` method call.
    pub receiver: String,
    pub physical_symbol: String,
}

/// Selects only scalar free functions and receiver-free associated functions.
/// A later Rust compile must verify the emitted call against the actual crate.
pub fn prepare_scalar_binding(
    import: &ResolvedImport,
    item: SelectedRustItem<'_>,
) -> Result<ScalarBindingPlan, Diagnostic> {
    let span = import.span;
    if !import.native_rust {
        return Err(error(
            "SPX-B140",
            "Rust API binding requires a native Rust import",
            span,
        ));
    }
    if !item.supported {
        return Err(error(
            "SPX-B141",
            "selected Rust API item is unavailable",
            span,
        ));
    }
    if !valid_alias(item.cargo_alias)
        || item.package_name.is_empty()
        || item.package_version.is_empty()
        || !valid_digest(item.package_source_sha256)
        || !valid_digest(item.index_digest)
        || !valid_digest(item.feature_digest)
        || item.target.is_empty()
        || item.target.len() > 128
        || !item
            .target
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err(error(
            "SPX-B142",
            "Rust API package or index identity is invalid",
            span,
        ));
    }
    let path_segments = item.path.split("::").collect::<Vec<_>>();
    if !valid_rust_api_path(item.path)
        || path_segments.first() != Some(&item.cargo_alias)
        || import
            .rust_path
            .as_ref()
            .is_some_and(|source_path| source_path != item.path)
    {
        return Err(error(
            "SPX-B143",
            "Rust API path does not belong to the selected Cargo alias",
            span,
        ));
    }
    if !matches!(item.kind, "function" | "inherent_method")
        || !matches!(item.receiver, "none" | "shared")
        || (item.receiver == "shared"
            && (item.kind != "inherent_method" || path_segments.len() < 3))
        || (item.receiver == "none" && item.kind == "function" && path_segments.len() < 2)
    {
        return Err(error(
            "SPX-B144",
            "Rust API receiver or item kind is unsupported by the scalar bridge",
            span,
        ));
    }
    let signature = parse_scalar_signature(item.signature)
        .ok_or_else(|| error("SPX-B145", scalar_signature_refusal(item.signature), span))?;
    let receiver_parameters = usize::from(item.receiver == "shared");
    if signature.name != *path_segments.last().unwrap()
        || signature.receiver != item.receiver
        || import.selected_receiver.as_deref().unwrap_or("none") != item.receiver
        || signature.parameters.len() + receiver_parameters != import.parameters.len()
        || (receiver_parameters == 1
            && !matches!(import.parameters.first(), Some(parameter) if parameter.ty == ResolvedType::I64 && parameter.ownership == OwnershipMode::Value))
        || signature
            .parameters
            .iter()
            .zip(import.parameters.iter().skip(receiver_parameters))
            .any(|(rust, semaprax)| {
                *rust != type_text(&semaprax.ty).unwrap_or("")
                    || semaprax.ownership != OwnershipMode::Value
            })
        || signature.result != result_text(&import.result.kind)
    {
        return Err(error(
            "SPX-B146",
            "Rust API signature disagrees with the checked import",
            span,
        ));
    }
    let mut digest = Sha256::new();
    digest.update(SYMBOL_DOMAIN);
    for component in [
        item.package_name,
        item.package_version,
        item.package_source_sha256,
        item.cargo_alias,
        item.index_digest,
        item.target,
        item.feature_digest,
        item.path,
        item.signature,
        import.id.as_str(),
    ] {
        digest.update((component.len() as u64).to_be_bytes());
        digest.update(component.as_bytes());
    }
    let mut physical_symbol = String::from("spx_ri04_");
    for byte in digest.finalize().iter() {
        write!(&mut physical_symbol, "{byte:02x}").expect("writing to String cannot fail");
    }
    Ok(ScalarBindingPlan {
        import_id: import.id.as_str().to_owned(),
        cargo_alias: item.cargo_alias.to_owned(),
        package_name: item.package_name.to_owned(),
        package_version: item.package_version.to_owned(),
        package_source_sha256: item.package_source_sha256.to_owned(),
        index_digest: item.index_digest.to_owned(),
        target: item.target.to_owned(),
        feature_digest: item.feature_digest.to_owned(),
        rust_path: item.path.to_owned(),
        signature: item.signature.to_owned(),
        receiver: item.receiver.to_owned(),
        physical_symbol,
    })
}

/// Rechecks a retained plan against the current HIR declaration. This is
/// required again at the physical builder boundary, after any source replay.
pub fn verify_scalar_binding(
    import: &ResolvedImport,
    plan: &ScalarBindingPlan,
) -> Result<(), Diagnostic> {
    let expected = prepare_scalar_binding(
        import,
        SelectedRustItem {
            cargo_alias: &plan.cargo_alias,
            package_name: &plan.package_name,
            package_version: &plan.package_version,
            package_source_sha256: &plan.package_source_sha256,
            index_digest: &plan.index_digest,
            target: &plan.target,
            feature_digest: &plan.feature_digest,
            path: &plan.rust_path,
            kind: if plan.receiver == "shared" {
                "inherent_method"
            } else {
                "function"
            },
            receiver: &plan.receiver,
            signature: &plan.signature,
            supported: true,
        },
    )?;
    if &expected != plan {
        return Err(error(
            "SPX-B142",
            "Rust API binding plan disagrees with the checked import or selected identity",
            import.span,
        ));
    }
    Ok(())
}

fn error(code: &'static str, message: &'static str, span: Span) -> Diagnostic {
    Diagnostic::error(code, message, span)
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn valid_alias(value: &str) -> bool {
    let value = value.strip_prefix("r#").unwrap_or(value);
    value.len() <= 128
        && value
            .as_bytes()
            .first()
            .is_some_and(|byte| byte.is_ascii_alphabetic() || *byte == b'_')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

/// Source shape only. Package alias, index, and target are checked later.
pub fn valid_rust_api_path(path: &str) -> bool {
    path.len() <= 512 && path.split("::").count() >= 2 && rust_api_path_tokens(path).is_some()
}

/// Render an index path as Rust tokens without changing its canonical identity.
/// Cargo aliases and public item names can be Rust keywords; the generated
/// module and call must use the same raw identifier spelling in that case.
pub fn rust_api_path_tokens(path: &str) -> Option<String> {
    let mut rendered = String::with_capacity(path.len() + 8);
    for (index, segment) in path.split("::").enumerate() {
        let raw = segment.strip_prefix("r#");
        let ident = raw.unwrap_or(segment);
        if !valid_alias(segment) || matches!(ident, "_" | "self" | "Self" | "super" | "crate") {
            return None;
        }
        if index > 0 {
            rendered.push_str("::");
        }
        if raw.is_some() || rust_keyword(ident) {
            rendered.push_str("r#");
        }
        rendered.push_str(ident);
    }
    Some(rendered)
}

fn rust_keyword(ident: &str) -> bool {
    matches!(
        ident,
        "as" | "async"
            | "await"
            | "break"
            | "const"
            | "continue"
            | "dyn"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "gen"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "static"
            | "struct"
            | "trait"
            | "true"
            | "try"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
            | "abstract"
            | "become"
            | "box"
            | "do"
            | "final"
            | "macro"
            | "override"
            | "priv"
            | "typeof"
            | "unsized"
            | "virtual"
            | "yield"
    )
}

fn type_text(ty: &ResolvedType) -> Option<&'static str> {
    match ty {
        ResolvedType::I64 => Some("i64"),
        ResolvedType::Bool => Some("bool"),
        _ => None,
    }
}

fn result_text(kind: &ResolvedImportResultKind) -> &'static str {
    match kind {
        ResolvedImportResultKind::Unit => "()",
        ResolvedImportResultKind::I64 => "i64",
        ResolvedImportResultKind::Bool => "bool",
        ResolvedImportResultKind::ResultI64I64 => "core::result::Result<i64, i64>",
    }
}

struct ScalarSignature<'a> {
    name: &'a str,
    parameters: Vec<&'a str>,
    result: &'a str,
    receiver: &'static str,
}

fn parse_scalar_signature(value: &str) -> Option<ScalarSignature<'_>> {
    if value.len() > 4096 {
        return None;
    }
    let rest = value.strip_prefix("fn ")?;
    let (name, rest) = rest.split_once('(')?;
    if !valid_alias(name) {
        return None;
    }
    let (parameters, result) = rest.split_once(") -> ")?;
    if !matches!(
        result,
        "()" | "i64" | "bool" | "core::result::Result<i64, i64>"
    ) {
        return None;
    }
    let (receiver, parameters) = if parameters == "&self" {
        ("shared", "")
    } else if let Some(rest) = parameters.strip_prefix("&self, ") {
        ("shared", rest)
    } else {
        ("none", parameters)
    };
    let mut types = Vec::new();
    if !parameters.is_empty() {
        for parameter in parameters.split(", ") {
            let (name, ty) = parameter.split_once(": ")?;
            if !valid_alias(name)
                || !matches!(ty, "i64" | "bool")
                || types.len() + usize::from(receiver == "shared") == 8
            {
                return None;
            }
            types.push(ty);
        }
    }
    Some(ScalarSignature {
        name,
        parameters: types,
        result,
        receiver,
    })
}

/// Describe the first reference rule that prevents this index item from
/// entering the scalar bridge. Metadata is still discovery data: these
/// classifications grant no loan, lifetime, or execution authority.
fn scalar_signature_refusal(signature: &str) -> &'static str {
    if signature.contains("for<") {
        return "higher-ranked Rust reference requires an unsupported lifetime relation";
    }
    if signature.contains("Pin<") || signature.contains("pin::Pin<") {
        return "pinned Rust reference requires an unsupported stable owner relation";
    }
    if signature.contains("UnsafeCell<")
        || signature.contains("Cell<")
        || signature.contains("RefCell<")
    {
        return "interior-mutable Rust reference requires an exclusive loan model";
    }
    if signature.contains("*const ") || signature.contains("*mut ") {
        return "raw Rust pointer has no verified provenance or initialized extent";
    }
    if signature.contains("&mut self") || signature.contains("&mut ") {
        return "mutable Rust reference requires a verified exclusive loan";
    }
    if let Some((_, result)) = signature.split_once(") -> ") {
        if result.starts_with('&') || result.starts_with("&'") {
            return if signature.contains("&self") {
                "returned Rust reference requires an owner-bound live view"
            } else {
                "returned Rust reference has no representable owner relation"
            };
        }
    }
    if signature.contains("&str") || (signature.contains("&'") && signature.contains(" str")) {
        return "borrowed Rust text requires an authenticated invocation loan";
    }
    if signature.contains("&[u8]") || (signature.contains("&'") && signature.contains(" [u8]")) {
        return "borrowed Rust bytes require an authenticated invocation loan";
    }
    "Rust API signature is unsupported by the scalar bridge"
}

/// Fills the short indexed import declaration from an already replayed and
/// selected API item. The caller must bind the returned declaration to the
/// selected index and package identity before it can reach code generation.
pub fn bind_selected_scalar_signature(
    import: &mut crate::ast::ImportDeclaration,
    signature: &str,
    index_digest: &str,
    receiver: &str,
) -> Result<(), Diagnostic> {
    use crate::ast::{ImportResult, Param, ParamMode, Type};
    if !import.native_rust || !import.index_selected || !valid_digest(index_digest) {
        return Err(error(
            "SPX-B142",
            "selected Rust import has invalid index identity",
            import.span,
        ));
    }
    let parsed = parse_scalar_signature(signature)
        .ok_or_else(|| error("SPX-B145", scalar_signature_refusal(signature), import.span))?;
    if parsed.receiver != receiver || !matches!(receiver, "none" | "shared") {
        return Err(error(
            "SPX-B144",
            "selected Rust method receiver is unsupported by the scalar bridge",
            import.span,
        ));
    }
    let path = import
        .rust_path
        .as_deref()
        .ok_or_else(|| error("SPX-B143", "selected Rust API path is missing", import.span))?;
    if !valid_rust_api_path(path) || path.rsplit("::").next() != Some(parsed.name) {
        return Err(error(
            "SPX-B143",
            "selected Rust API path disagrees with the selected signature",
            import.span,
        ));
    }
    import.params = parsed
        .parameters
        .iter()
        .enumerate()
        .map(|(index, ty)| Param {
            name: format!("arg{index}"),
            mode: ParamMode::Value,
            ty: if *ty == "i64" { Type::I64 } else { Type::Bool },
            span: import.span,
        })
        .collect();
    if receiver == "shared" {
        import.params.insert(
            0,
            Param {
                name: "receiver".to_owned(),
                mode: ParamMode::Value,
                ty: Type::I64,
                span: import.span,
            },
        );
    }
    import.result = match parsed.result {
        "i64" => ImportResult::I64,
        "bool" => ImportResult::Bool,
        "core::result::Result<i64, i64>" => ImportResult::ResultI64I64,
        _ => ImportResult::Unit,
    };
    import.selected_signature = Some(signature.to_owned());
    import.selected_index_digest = Some(index_digest.to_owned());
    import.selected_receiver = (receiver == "shared").then(|| "shared".to_owned());
    Ok(())
}
