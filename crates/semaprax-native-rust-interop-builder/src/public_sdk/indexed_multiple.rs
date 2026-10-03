//! Exact, independently selected package instances in one callable scalar SDK.

use super::*;
use crate::indexed_binding::{prepare_indexed_scalar_binding, SelectedPackage};
use semaprax::native_rust_binding::ScalarBindingPlan;
use semaprax_rust_api_index::{ItemKind, Receiver, RustApiIndex};

/// One explicit persistent import ID and its exact index/package source bytes.
/// Multiple selections may name different versions of the same package, but
/// each Cargo alias must identify one package instance throughout the build.
#[derive(Clone, Copy)]
pub struct IndexedScalarSelection<'a> {
    pub import_id: &'a str,
    pub index_bytes: &'a [u8],
    pub package: SelectedPackage<'a>,
    pub package_source_bytes: &'a [u8],
}

type PreparedIndexedScalars<'a> = (
    crate::ast::Program,
    semaprax::hir::ResolvedProgram,
    Vec<ScalarBindingPlan>,
    Vec<&'a str>,
);

/// Publishes a callable SDK for 1–32 independently selected scalar imports.
/// Sources retain the single-file, dependency-free profile; no Cargo resolution
/// or foreign invocation occurs during admission or signature compilation.
/// Selection order does not affect generated identities or package bytes.
pub fn build_indexed_scalars_native_rust_sdk(
    source: &str,
    source_path: &Path,
    options: NativeRustSdkOptions,
    selections: &[IndexedScalarSelection<'_>],
    output: &Path,
) -> Result<NativeRustSdkBundle, Vec<Diagnostic>> {
    let (program, _, plans, sources) =
        prepare_indexed_scalars(source, source_path, &options, selections)?;
    authority::build_indexed_scalars_sdk_inner(
        &program,
        &plans,
        &sources,
        selections[0].package.stable_rustc_version,
        options,
        output,
    )
    .map_err(PublicBuildError::into_diagnostics)
}

pub(super) fn prepare_indexed_scalars<'a>(
    source: &str,
    source_path: &Path,
    options: &NativeRustSdkOptions,
    selections: &[IndexedScalarSelection<'a>],
) -> Result<PreparedIndexedScalars<'a>, Vec<Diagnostic>> {
    if source.len() > MAX_SOURCE_BYTES
        || selections.is_empty()
        || selections.len() > MAX_IMPORTS
        || selections
            .iter()
            .any(|item| item.package_source_bytes.len() > 65_536)
    {
        return Err(vec![sdk_error(
            "indexed scalar selections exceed their bounds",
        )]);
    }
    let imports =
        canonical_values(options.imports.clone(), MAX_IMPORTS).map_err(|error| vec![error])?;
    let exports =
        canonical_values(options.exports.clone(), MAX_EXPORTS).map_err(|error| vec![error])?;
    canonical_values(options.capabilities.clone(), MAX_EFFECTS).map_err(|error| vec![error])?;
    let mut selections = selections.to_vec();
    selections.sort_by(|left, right| left.import_id.cmp(right.import_id));
    if exports.is_empty()
        || imports.len() != selections.len()
        || imports
            .iter()
            .zip(&selections)
            .any(|(id, selection)| id != selection.import_id)
    {
        return Err(vec![sdk_error(
            "indexed scalar selections must exactly match requested imports",
        )]);
    }
    let mut program = semaprax::parse(source, source_path).map_err(|error| vec![error])?;
    let located = |code, message, span| {
        vec![Diagnostic::error(code, message, span).at_path(source_path.display().to_string())]
    };
    // No selected declaration can remain unbound or fall back to a callback.
    for import in program
        .interfaces
        .iter()
        .flat_map(|interface| &interface.imports)
    {
        if import.index_selected && !imports.iter().any(|id| id == &import.stable_id) {
            return Err(located(
                "SPX-B140",
                "selected Rust import is missing its package selection",
                import.span,
            ));
        }
    }
    let mut sources = Vec::with_capacity(selections.len());
    for (position, selection) in selections.iter().enumerate() {
        let import = program
            .interfaces
            .iter_mut()
            .flat_map(|interface| &mut interface.imports)
            .find(|import| import.stable_id == selection.import_id)
            .ok_or_else(|| vec![sdk_error("indexed scalar import selection is missing")])?;
        let package = selection.package;
        if package.stable_rustc_version != selections[0].package.stable_rustc_version
            || raw_digest(selection.package_source_bytes) != package.source_sha256
        {
            return Err(located(
                "SPX-B142",
                "Rust API package bytes or compiler identity disagree with the selection",
                import.span,
            ));
        }
        // One module name cannot silently resolve to two package instances.
        for previous in &selections[..position] {
            if previous.package.cargo_alias == package.cargo_alias
                && (previous.index_bytes != selection.index_bytes
                    || previous.package.name != package.name
                    || previous.package.version != package.version
                    || previous.package.source_sha256 != package.source_sha256
                    || previous.package.feature_digest != package.feature_digest
                    || previous.package.target != package.target
                    || previous.package_source_bytes != selection.package_source_bytes)
            {
                return Err(located(
                    "SPX-B142",
                    "Rust Cargo alias selects conflicting package instances",
                    import.span,
                ));
            }
        }
        let package_source = std::str::from_utf8(selection.package_source_bytes).map_err(|_| {
            located(
                "SPX-B142",
                "indexed scalar source must be UTF-8",
                import.span,
            )
        })?;
        indexed::validate_embedded_scalar_source(package_source)
            .map_err(|message| located("SPX-B142", message, import.span))?;
        sources.push(package_source);
        let index = RustApiIndex::replay(selection.index_bytes).map_err(|_| {
            located(
                "SPX-B142",
                "selected Rust API index replay failed",
                import.span,
            )
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
                located(
                    "SPX-B142",
                    "selected Rust API identity disagrees with the prepared index",
                    import.span,
                )
            })?;
        let path = import
            .rust_path
            .as_deref()
            .ok_or_else(|| located("SPX-B143", "indexed Rust API path is missing", import.span))?;
        let items = index.select_supported(&[path]).map_err(|_| {
            located(
                "SPX-B141",
                "selected Rust API item is unavailable",
                import.span,
            )
        })?;
        let item = items[0];
        if !matches!(
            (item.kind, item.receiver),
            (
                ItemKind::Function | ItemKind::InherentMethod,
                Receiver::None
            ) | (ItemKind::InherentMethod, Receiver::Shared)
        ) {
            return Err(located(
                "SPX-B144",
                "Rust API receiver or item kind is unsupported by the scalar bridge",
                import.span,
            ));
        }
        if import.index_selected {
            semaprax::native_rust_binding::bind_selected_scalar_signature(
                import,
                &item.signature,
                index.digest(),
                if item.receiver == Receiver::Shared {
                    "shared"
                } else {
                    "none"
                },
            )
            .map_err(|error| vec![error.at_path(source_path.display().to_string())])?;
        }
    }
    let diagnostics = semaprax::verify::verify(&program);
    if diagnostics.iter().any(|item| item.severity.is_error()) {
        return Err(diagnostics);
    }
    let resolved = semaprax::hir::resolve(&program)?;
    let target =
        target_triple().ok_or_else(|| vec![sdk_error("indexed scalar target is unsupported")])?;
    let mut plans = Vec::with_capacity(selections.len());
    for selection in &selections {
        let import = resolved
            .interfaces
            .iter()
            .flat_map(|interface| &interface.imports)
            .find(|import| import.id.as_str() == selection.import_id)
            .expect("selected import was bound");
        let plan = prepare_indexed_scalar_binding(
            import,
            selection.index_bytes,
            selection.package,
            import
                .rust_path
                .as_deref()
                .expect("selected path was checked"),
        )
        .map_err(|error| vec![error.at_path(source_path.display().to_string())])?;
        if plan.target != target {
            return Err(located(
                "SPX-B142",
                "Rust API index target disagrees with the selected native target",
                import.span,
            ));
        }
        plans.push(plan);
    }
    Ok((program, resolved, plans, sources))
}
