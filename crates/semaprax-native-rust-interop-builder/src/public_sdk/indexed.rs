//! Public, narrow RI-04 admission for caller-held adapter source and for the
//! single-file package profile that publishes an authenticated callable SDK.

use super::*;
use crate::indexed_binding::{
    prepare_indexed_scalar_binding, render_checked_scalar_adapter, SelectedPackage,
};
use semaprax_rust_api_index::{ItemKind, Receiver, RustApiIndex};

/// Publishes an authenticated nine-file SDK package with one embedded,
/// dependency-free Rust source module and its generated scalar adapter.
/// The selected package source is deliberately limited to a single UTF-8 file.
pub fn build_indexed_scalar_native_rust_sdk(
    source: &str,
    source_path: &Path,
    options: NativeRustSdkOptions,
    index_bytes: &[u8],
    package: SelectedPackage<'_>,
    package_source_bytes: &[u8],
    output: &Path,
) -> Result<NativeRustSdkBundle, Vec<Diagnostic>> {
    if package_source_bytes.len() > 65_536 {
        return Err(vec![sdk_error(
            "indexed scalar single-file source exceeds its bound",
        )]);
    }
    let (program, resolved, plan, _) = prepare_indexed_scalar(
        source,
        source_path,
        &options,
        index_bytes,
        package,
        package_source_bytes,
    )?;
    let import_span = resolved
        .interfaces
        .iter()
        .flat_map(|interface| &interface.imports)
        .find(|import| import.id.as_str() == plan.import_id)
        .expect("prepared import")
        .span;
    let package_source = std::str::from_utf8(package_source_bytes).map_err(|_| {
        vec![Diagnostic::error(
            "SPX-B142",
            "indexed scalar source must be UTF-8",
            import_span,
        )
        .at_path(source_path.display().to_string())]
    })?;
    validate_embedded_scalar_source(package_source).map_err(|message| {
        vec![Diagnostic::error("SPX-B142", message, import_span)
            .at_path(source_path.display().to_string())]
    })?;
    authority::build_indexed_scalar_sdk_inner(
        &program,
        &plan,
        package_source,
        package.stable_rustc_version,
        options,
        output,
    )
    .map_err(PublicBuildError::into_diagnostics)
}

/// The selected file is embedded verbatim in both Rust compilations. Keep this
/// first profile to ASCII, ordinary source tokens: no macro expansion,
/// attributes, external modules, paths, imports, or literal file reads can
/// introduce bytes outside the selected package digest.
fn validate_embedded_scalar_source(source: &str) -> Result<(), &'static str> {
    let bytes = source.as_bytes();
    let forbidden_pair = bytes
        .windows(2)
        .any(|pair| pair == b"::" || pair == b"/*" || pair == b"//");
    let forbidden_byte = bytes.iter().any(|byte| {
        !byte.is_ascii()
            || matches!(
                *byte,
                b'!' | b'#' | b';' | b'"' | b'\'' | b'[' | b']' | b'\\'
            )
    });
    let forbidden_ident = source
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .any(|ident| {
            matches!(
                ident,
                "mod" | "extern" | "use" | "unsafe" | "std" | "core" | "include" | "env" | "asm"
            )
        });
    if forbidden_pair || forbidden_byte || forbidden_ident {
        return Err("indexed scalar source uses syntax outside the self-contained profile");
    }
    Ok(())
}

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
    let (program, resolved, plan, spec) = prepare_indexed_scalar(
        source,
        source_path,
        &options,
        index_bytes,
        package,
        package_source_bytes,
    )?;
    let import = resolved
        .interfaces
        .iter()
        .flat_map(|interface| &interface.imports)
        .find(|import| import.id.as_str() == plan.import_id)
        .expect("validated import");
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

pub(super) fn prepare_indexed_scalar(
    source: &str,
    source_path: &Path,
    options: &NativeRustSdkOptions,
    index_bytes: &[u8],
    package: SelectedPackage<'_>,
    package_source_bytes: &[u8],
) -> Result<
    (
        crate::ast::Program,
        semaprax::hir::ResolvedProgram,
        semaprax::native_rust_binding::ScalarBindingPlan,
        String,
    ),
    Vec<Diagnostic>,
> {
    if source.len() > MAX_SOURCE_BYTES || package_source_bytes.len() > MAX_SOURCE_BYTES {
        return Err(vec![sdk_error("indexed Rust source exceeds its bound")]);
    }
    let mut program = semaprax::parse(source, source_path).map_err(|error| vec![error])?;
    let options = NativeRustSdkOptions {
        exports: canonical_values(options.exports.clone(), MAX_EXPORTS)
            .map_err(|error| vec![error])?,
        imports: canonical_values(options.imports.clone(), MAX_IMPORTS)
            .map_err(|error| vec![error])?,
        capabilities: canonical_values(options.capabilities.clone(), MAX_EFFECTS)
            .map_err(|error| vec![error])?,
    };
    if options.imports.len() != 1 || options.exports.is_empty() {
        return Err(vec![sdk_error(
            "indexed scalar build requires one selected import and an export",
        )]);
    }
    let selected = program
        .interfaces
        .iter()
        .flat_map(|interface| &interface.imports)
        .filter(|import| import.index_selected)
        .count();
    if selected > 0 {
        if selected != 1 {
            return Err(vec![sdk_error(
                "indexed scalar build requires exactly one selected import",
            )]);
        }
        let import = program
            .interfaces
            .iter_mut()
            .flat_map(|interface| &mut interface.imports)
            .find(|import| import.index_selected)
            .expect("one selected import");
        if import.stable_id != options.imports[0] {
            return Err(vec![Diagnostic::error(
                "SPX-B140",
                "selected Rust import disagrees with the requested import identity",
                import.span,
            )]);
        }
        let index = RustApiIndex::replay(index_bytes).map_err(|_| {
            vec![Diagnostic::error(
                "SPX-B142",
                "selected Rust API index replay failed",
                import.span,
            )]
        })?;
        index
            .require_package_identity(
                package.name,
                package.version,
                package.source_sha256,
                package.target,
                package.feature_digest,
            )
            .and_then(|_| index.require_cargo_alias_identity(package.cargo_alias))
            .and_then(|_| index.require_stable_compiler_identity(package.stable_rustc_version))
            .map_err(|_| {
                vec![Diagnostic::error(
                    "SPX-B142",
                    "selected Rust API identity disagrees with the prepared index",
                    import.span,
                )]
            })?;
        let path = import.rust_path.as_deref().expect("selected path parsed");
        let items = index.select_supported(&[path]).map_err(|_| {
            vec![Diagnostic::error(
                "SPX-B141",
                "selected Rust API item is unavailable",
                import.span,
            )]
        })?;
        let item = items[0];
        if !matches!(item.kind, ItemKind::Function | ItemKind::InherentMethod)
            || item.receiver != Receiver::None
        {
            return Err(vec![Diagnostic::error(
                "SPX-B144",
                "Rust API receiver or item kind is unsupported by the scalar bridge",
                import.span,
            )]);
        }
        semaprax::native_rust_binding::bind_selected_scalar_signature(
            import,
            &item.signature,
            index.digest(),
        )
        .map_err(|error| vec![error])?;
    }
    let diagnostics = semaprax::verify::verify(&program);
    if diagnostics.iter().any(|item| item.severity.is_error()) {
        return Err(diagnostics);
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
    Ok((program, resolved, plan, spec))
}
