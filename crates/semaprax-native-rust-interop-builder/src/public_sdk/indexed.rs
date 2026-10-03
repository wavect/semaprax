//! Public, narrow RI-04 admission. The generated adapter remains caller-held
//! until a versioned package profile can publish it with the inner bundle.

use super::*;
use crate::indexed_binding::{
    prepare_indexed_scalar_binding, render_checked_scalar_adapter, SelectedPackage,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexedScalarBuild {
    output_directory: PathBuf,
    bundle_manifest_digest: String,
    adapter_source: String,
    adapter_sha256: String,
    physical_symbol: String,
}

impl IndexedScalarBuild {
    pub fn output_directory(&self) -> &Path {
        &self.output_directory
    }
    pub fn bundle_manifest_digest(&self) -> &str {
        &self.bundle_manifest_digest
    }
    pub fn adapter_source(&self) -> &str {
        &self.adapter_source
    }
    pub fn adapter_sha256(&self) -> &str {
        &self.adapter_sha256
    }
    pub fn physical_symbol(&self) -> &str {
        &self.physical_symbol
    }
}

/// Checks exact source and index identities before any build or publication.
/// The `package_source_bytes` must be the exact bytes named by the selected
/// package digest, not merely a caller assertion about their identity.
pub fn build_indexed_scalar_native_rust(
    source: &str,
    source_path: &Path,
    options: NativeRustSdkOptions,
    index_bytes: &[u8],
    package: SelectedPackage<'_>,
    package_source_bytes: &[u8],
    output: &Path,
) -> Result<IndexedScalarBuild, Vec<Diagnostic>> {
    if source.len() > MAX_SOURCE_BYTES || package_source_bytes.len() > MAX_SOURCE_BYTES {
        return Err(vec![sdk_error("indexed Rust source exceeds its bound")]);
    }
    let program = semaprax::check(source, source_path)?;
    let options = NativeRustSdkOptions {
        exports: canonical_values(options.exports, MAX_EXPORTS).map_err(|error| vec![error])?,
        imports: canonical_values(options.imports, MAX_IMPORTS).map_err(|error| vec![error])?,
        capabilities: canonical_values(options.capabilities, MAX_EFFECTS)
            .map_err(|error| vec![error])?,
    };
    if options.imports.len() != 1 || options.exports.is_empty() {
        return Err(vec![sdk_error(
            "indexed scalar build requires one selected import and an export",
        )]);
    }
    let resolved = semaprax::hir::resolve(&program)?;
    let import = resolved
        .interfaces
        .iter()
        .flat_map(|interface| &interface.imports)
        .find(|import| import.id.as_str() == options.imports[0])
        .ok_or_else(|| vec![sdk_error("indexed scalar import selection is missing")])?;
    let item_path = import.rust_path.as_deref().ok_or_else(|| {
        vec![Diagnostic::error(
            "SPX-B143",
            "indexed Rust API path is missing",
            import.span,
        )]
    })?;
    if raw_digest(package_source_bytes) != package.source_sha256 {
        return Err(vec![Diagnostic::error(
            "SPX-B142",
            "Rust API package source bytes disagree with the selected digest",
            import.span,
        )]);
    }
    let plan = prepare_indexed_scalar_binding(import, index_bytes, package, item_path)
        .map_err(|error| vec![error])?;
    let target =
        target_triple().ok_or_else(|| vec![sdk_error("indexed scalar target is unsupported")])?;
    if plan.target != target {
        return Err(vec![Diagnostic::error(
            "SPX-B142",
            "Rust API index target disagrees with the selected native target",
            import.span,
        )]);
    }
    let canonical_source = semaprax::format::canonical(&program);
    let revision = domain_digest(SOURCE_DOMAIN, canonical_source.as_bytes());
    let spec = descriptor::canonical_spec(&program.module, &revision, target, &options)
        .map_err(|error| vec![error])?;
    let method =
        crate::implementation::indexed_scalar_rust_method(&program, spec.as_bytes(), &plan)?;
    let adapter_source =
        render_checked_scalar_adapter(import, &plan, &method).map_err(|error| vec![error])?;
    if adapter_source.len() > MAX_GENERATED_RUST_BYTES {
        return Err(vec![sdk_error("indexed scalar adapter exceeds its bound")]);
    }
    let adapter_sha256 = raw_digest(adapter_source.as_bytes());
    let facts = crate::implementation::build_indexed_native_rust_interop_bundle(
        &program,
        spec.as_bytes(),
        &[plan.clone()],
        output,
    )?;
    Ok(IndexedScalarBuild {
        output_directory: facts.output_directory().to_path_buf(),
        bundle_manifest_digest: facts.manifest_digest().to_owned(),
        adapter_source,
        adapter_sha256,
        physical_symbol: plan.physical_symbol,
    })
}
