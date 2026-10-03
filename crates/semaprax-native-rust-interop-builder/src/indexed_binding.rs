//! Inert, source-bound scalar selection from an exact replayed RI-03 index.
//! This path neither builds a crate nor authorizes a foreign call.

use semaprax::diagnostic::Diagnostic;
use semaprax::hir::ResolvedImport;
use semaprax::native_rust_binding::{prepare_scalar_binding, ScalarBindingPlan, SelectedRustItem};
use semaprax_rust_api_index::{IndexError, ItemKind, Receiver, RustApiIndex};

#[derive(Clone, Copy)]
pub struct SelectedPackage<'a> {
    pub cargo_alias: &'a str,
    pub name: &'a str,
    pub version: &'a str,
    pub source_sha256: &'a str,
    pub target: &'a str,
    pub feature_digest: &'a str,
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
    const SOURCE: &str = "module binding.test; @id(\"binding.host\") interface Host permits {  } { @id(\"binding.simple\") import rust fn simple(value: i64) -> bool effects {  } failure infallible; }";

    #[test]
    fn exact_index_replay_precedes_receiver_admission() {
        let program = semaprax::parse(SOURCE, Path::new("indexed-binding.spx")).unwrap();
        let resolved = semaprax::hir::resolve(&program).unwrap();
        let import = &resolved.interfaces[0].imports[0];
        let package = SelectedPackage {
            cargo_alias: "fixture_api",
            name: "fixture_api",
            version: "0.0.0",
            source_sha256:
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            target: "x86_64-unknown-linux-gnu",
            feature_digest:
                "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        };
        let rejected =
            prepare_indexed_scalar_binding(import, INDEX, package, "fixture_api::Example::simple")
                .unwrap_err();
        assert_eq!(rejected.code, "SPX-B144");
        assert_eq!(rejected.span, Some(import.span));
        let mut stale = package;
        stale.version = "1.0.0";
        assert_eq!(
            prepare_indexed_scalar_binding(import, INDEX, stale, "fixture_api::Example::simple")
                .unwrap_err()
                .code,
            "SPX-B142"
        );
        assert_eq!(
            prepare_indexed_scalar_binding(
                import,
                b"{}\n",
                INDEX_PACKAGE,
                "fixture_api::Example::simple"
            )
            .unwrap_err()
            .code,
            "SPX-B147"
        );
    }

    const INDEX_PACKAGE: SelectedPackage<'static> = SelectedPackage {
        cargo_alias: "fixture_api",
        name: "fixture_api",
        version: "0.0.0",
        source_sha256: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        target: "x86_64-unknown-linux-gnu",
        feature_digest: "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    };
}
