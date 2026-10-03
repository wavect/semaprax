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
    if !matches!(item.kind, "function" | "inherent_method") || item.receiver != "none" {
        return Err(error(
            "SPX-B144",
            "Rust API receiver or item kind is unsupported by the scalar bridge",
            span,
        ));
    }
    let signature = parse_scalar_signature(item.signature).ok_or_else(|| {
        error(
            "SPX-B145",
            "Rust API signature is unsupported by the scalar bridge",
            span,
        )
    })?;
    if signature.name != *path_segments.last().unwrap()
        || signature.parameters.len() != import.parameters.len()
        || signature
            .parameters
            .iter()
            .zip(&import.parameters)
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
            kind: "function",
            receiver: "none",
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
    path.len() <= 512 && path.split("::").count() >= 2 && path.split("::").all(valid_alias)
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
    }
}

struct ScalarSignature<'a> {
    name: &'a str,
    parameters: Vec<&'a str>,
    result: &'a str,
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
    if !matches!(result, "()" | "i64" | "bool") {
        return None;
    }
    let mut types = Vec::new();
    if !parameters.is_empty() {
        for parameter in parameters.split(", ") {
            let (name, ty) = parameter.split_once(": ")?;
            if !valid_alias(name) || !matches!(ty, "i64" | "bool") || types.len() == 8 {
                return None;
            }
            types.push(ty);
        }
    }
    Some(ScalarSignature {
        name,
        parameters: types,
        result,
    })
}
