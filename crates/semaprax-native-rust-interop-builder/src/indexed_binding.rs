//! Inert, source-bound scalar selection from an exact replayed RI-03 index.
//! This path neither builds a crate nor authorizes a foreign call.

use semaprax::diagnostic::Diagnostic;
use semaprax::hir::ResolvedImport;
use semaprax::hir::{ResolvedImportResultKind, ResolvedType};
use semaprax::native_rust_binding::{
    prepare_scalar_binding, rust_api_path_tokens, verify_scalar_binding, ScalarBindingPlan,
    SelectedRustItem,
};
use semaprax_rust_api_index::{IndexError, ItemKind, Receiver, RustApiIndex};

#[derive(Clone, Copy)]
pub struct SelectedPackage<'a> {
    pub cargo_alias: &'a str,
    pub name: &'a str,
    pub version: &'a str,
    pub source_sha256: &'a str,
    pub target: &'a str,
    pub feature_digest: &'a str,
    pub stable_rustc_version: &'a str,
}

/// Replays index bytes and checks exact package, target, and feature identity
/// before selecting one supported item. Rust still has to type-check the
/// generated wrapper against the actual crate before invocation.
pub fn prepare_indexed_scalar_binding(
    import: &ResolvedImport,
    index_bytes: &[u8],
    package: SelectedPackage<'_>,
    item_path: &str,
) -> Result<ScalarBindingPlan, Diagnostic> {
    let index = RustApiIndex::replay(index_bytes).map_err(|error| diagnostic(import, error))?;
    index
        .require_package_identity(
            package.name,
            package.version,
            package.source_sha256,
            package.target,
            package.feature_digest,
        )
        .map_err(|error| diagnostic(import, error))?;
    index
        .require_cargo_alias_identity(package.cargo_alias)
        .map_err(|error| diagnostic(import, error))?;
    index
        .require_stable_compiler_identity(package.stable_rustc_version)
        .map_err(|error| diagnostic(import, error))?;
    let selected = index
        .select_supported(&[item_path])
        .map_err(|error| diagnostic(import, error))?;
    let item = selected[0];
    let kind = match item.kind {
        ItemKind::Function => "function",
        ItemKind::InherentMethod => "inherent_method",
        ItemKind::TraitMethod => "trait_method",
        ItemKind::AssociatedType => "associated_type",
    };
    let receiver = match item.receiver {
        Receiver::None => "none",
        Receiver::Shared => "shared",
        Receiver::Mutable => "mutable",
        Receiver::Owned => "owned",
    };
    prepare_scalar_binding(
        import,
        SelectedRustItem {
            cargo_alias: package.cargo_alias,
            package_name: &index.package().name,
            package_version: &index.package().version,
            package_source_sha256: &index.package().source_sha256,
            index_digest: index.digest(),
            target: index.target(),
            feature_digest: index.feature_digest(),
            path: &item.path,
            kind,
            receiver,
            signature: &item.signature,
            supported: true,
        },
    )
}

/// Renders a safe Rust trait implementation for the v1 scalar carrier. The
/// typed function-pointer assignment is intentionally retained: the selected
/// stable Rust compiler must check the actual crate function before a bridge
/// executable can be linked or run.
pub fn render_checked_scalar_adapter(
    import: &ResolvedImport,
    plan: &ScalarBindingPlan,
    rust_method: &str,
) -> Result<String, Diagnostic> {
    let (wrapper, method) = render_checked_scalar_adapter_parts(import, plan, rust_method)?;
    Ok(format!("{wrapper}struct GeneratedIndexedAdapter;\nimpl NativeRustImports for GeneratedIndexedAdapter{{{method}}}\n"))
}

/// Assemble all selected methods into one complete trait implementation.
/// Each wrapper retains its package/signature/import-derived physical name.
pub(crate) fn render_checked_scalar_adapters(
    imports: &[(&ResolvedImport, &ScalarBindingPlan, &str)],
) -> Result<String, Diagnostic> {
    let mut wrappers = String::new();
    let mut methods = String::new();
    for &(import, plan, method) in imports {
        let (wrapper, method) = render_checked_scalar_adapter_parts(import, plan, method)?;
        wrappers.push_str(&wrapper);
        methods.push_str(&method);
    }
    Ok(format!("{wrappers}struct GeneratedIndexedAdapter;\nimpl NativeRustImports for GeneratedIndexedAdapter{{{methods}}}\n"))
}

fn render_checked_scalar_adapter_parts(
    import: &ResolvedImport,
    plan: &ScalarBindingPlan,
    rust_method: &str,
) -> Result<(String, String), Diagnostic> {
    verify_scalar_binding(import, plan)?;
    if rust_method.is_empty()
        || rust_method.len() > 128
        || !rust_method
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        || !rust_method.as_bytes()[0].is_ascii_alphabetic()
    {
        return Err(Diagnostic::error(
            "SPX-B145",
            "generated Rust import method is invalid",
            import.span,
        ));
    }
    let parameters = import
        .parameters
        .iter()
        .map(|parameter| match parameter.ty {
            ResolvedType::I64 => Ok("i64"),
            ResolvedType::Bool => Ok("bool"),
            _ => Err(Diagnostic::error(
                "SPX-B145",
                "Rust API signature is unsupported by the scalar bridge",
                import.span,
            )),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let result = match &import.result.kind {
        ResolvedImportResultKind::Unit => "()",
        ResolvedImportResultKind::I64 => "i64",
        ResolvedImportResultKind::Bool => "bool",
        ResolvedImportResultKind::ResultI64I64 => "core::result::Result<i64,i64>",
        ResolvedImportResultKind::BorrowedStr { .. }
        | ResolvedImportResultKind::OwnedResource { .. }
        | ResolvedImportResultKind::OwnedResultResourceI64 { .. }
        | ResolvedImportResultKind::OwnedString
        | ResolvedImportResultKind::OwnedOptionString
        | ResolvedImportResultKind::OwnedResultStringI64
        | ResolvedImportResultKind::OwnedResultStringOptionI64 => {
            return Err(Diagnostic::error(
                "SPX-B145",
                "Rust API signature is unsupported by the scalar bridge",
                import.span,
            ));
        }
    };
    let arguments = (0..parameters.len())
        .map(|index| format!("arg_{index}"))
        .collect::<Vec<_>>();
    let declarations = arguments
        .iter()
        .zip(&parameters)
        .map(|(argument, ty)| format!("{argument}:{ty}"))
        .collect::<Vec<_>>()
        .join(",");
    let function_type = parameters.join(",");
    let call_arguments = arguments.join(",");
    let rust_path = rust_api_path_tokens(&plan.rust_path).ok_or_else(|| {
        Diagnostic::error(
            "SPX-B143",
            "selected Rust API path cannot be emitted",
            import.span,
        )
    })?;
    let method = format!(
        "fn {rust_method}(&mut self{}{})->NativeRustImportResult<{result}>{{NativeRustImportResult::Success({}({call_arguments}))}}",
        if declarations.is_empty() { "" } else { "," },
        declarations,
        plan.physical_symbol,
    );
    let wrapper = if plan.receiver == "shared" {
        let receiver_type = rust_path
            .rsplit_once("::")
            .ok_or_else(|| {
                Diagnostic::error(
                    "SPX-B143",
                    "selected Rust method has no receiver type path",
                    import.span,
                )
            })?
            .0;
        let method_types = parameters
            .iter()
            .skip(1)
            .copied()
            .collect::<Vec<_>>()
            .join(",");
        let method_arguments = arguments
            .iter()
            .skip(1)
            .cloned()
            .collect::<Vec<_>>()
            .join(",");
        let comma = if method_types.is_empty() { "" } else { "," };
        let call_comma = if method_arguments.is_empty() { "" } else { "," };
        format!(
            "fn {}({declarations})->{result}{{let receiver:{receiver_type}=<{receiver_type} as core::convert::From<i64>>::from(arg_0);let target:fn(&{receiver_type}{comma}{method_types})->{result}={};target(&receiver{call_comma}{method_arguments})}}\n",
            plan.physical_symbol, rust_path,
        )
    } else {
        format!(
            "fn {}({declarations})->{result}{{let target:fn({function_type})->{result}={};target({call_arguments})}}\n",
            plan.physical_symbol, rust_path,
        )
    };
    Ok((wrapper, method))
}

fn diagnostic(import: &ResolvedImport, error: IndexError) -> Diagnostic {
    let (code, message) = match error {
        IndexError::Malformed => ("SPX-B147", "Rust API index is malformed or noncanonical"),
        IndexError::SetupRequired => ("SPX-B148", "Rust API index extractor setup is required"),
        IndexError::IdentityMismatch => {
            ("SPX-B142", "Rust API package or index identity has drifted")
        }
        IndexError::ItemUnavailable => ("SPX-B141", "selected Rust API item is unavailable"),
    };
    Diagnostic::error(code, message, import.span)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    const INDEX: &[u8] =
        include_bytes!("../../semaprax-rust-api-index/fixtures/protocol-envelope-example.json");
    const SOURCE: &str = "module binding.test; @id(\"binding.host\") interface Host permits {  } { @id(\"binding.simple\") import rust fn simple(value: i64) -> bool effects {  } failure infallible; } @id(\"binding.main\") fn main() -> i64 { 1 }";

    #[test]
    fn exact_index_replay_precedes_signature_admission() {
        let program = semaprax::parse(SOURCE, Path::new("indexed-binding.spx")).unwrap();
        let resolved = semaprax::hir::resolve(&program).unwrap();
        let import = &resolved.interfaces[0].imports[0];
        let package = SelectedPackage {
            cargo_alias: "local_api_fixture",
            name: "local_api_fixture",
            version: "0.0.0",
            source_sha256:
                "sha256:dbc31a9272b4e500ca6363d633d8c7b5dac727ce7279cc65391cff5f377010dd",
            target: "aarch64-apple-darwin",
            feature_digest:
                "sha256:d3e066ea11e87bd8665f8fdca51534758ab735c2346340a1f64895fdd81c5a69",
            stable_rustc_version: "rustc 1.98.0 (88d9e12ae 2026-08-18) (Homebrew)",
        };
        let rejected = prepare_indexed_scalar_binding(
            import,
            INDEX,
            package,
            "local_api_fixture::MacroGenerated::answer",
        )
        .unwrap_err();
        assert_eq!(rejected.code, "SPX-B145");
        assert_eq!(rejected.span, Some(import.span));
        let mut stale = package;
        stale.version = "1.0.0";
        let mut wrong_compiler = package;
        wrong_compiler.stable_rustc_version = "rustc 1.97.1";
        let mut wrong_alias = package;
        wrong_alias.cargo_alias = "other_alias";
        for selected in [stale, wrong_compiler, wrong_alias] {
            let error = prepare_indexed_scalar_binding(
                import,
                INDEX,
                selected,
                "local_api_fixture::MacroGenerated::answer",
            )
            .unwrap_err();
            assert_eq!(error.code, "SPX-B142");
            assert_eq!(error.span, Some(import.span));
        }
        let malformed = prepare_indexed_scalar_binding(
            import,
            b"{}\n",
            INDEX_PACKAGE,
            "local_api_fixture::MacroGenerated::answer",
        )
        .unwrap_err();
        assert_eq!(malformed.code, "SPX-B147");
        assert_eq!(malformed.span, Some(import.span));
    }

    const INDEX_PACKAGE: SelectedPackage<'static> = SelectedPackage {
        cargo_alias: "local_api_fixture",
        name: "local_api_fixture",
        version: "0.0.0",
        source_sha256: "sha256:dbc31a9272b4e500ca6363d633d8c7b5dac727ce7279cc65391cff5f377010dd",
        target: "aarch64-apple-darwin",
        feature_digest: "sha256:d3e066ea11e87bd8665f8fdca51534758ab735c2346340a1f64895fdd81c5a69",
        stable_rustc_version: "rustc 1.98.0 (88d9e12ae 2026-08-18) (Homebrew)",
    };
}
