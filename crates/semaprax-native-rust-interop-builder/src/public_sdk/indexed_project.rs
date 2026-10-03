//! Authenticated Project publication with explicit RI-03 selected imports.

use super::*;
use crate::indexed_binding::prepare_indexed_scalar_binding;
use semaprax::project::ProjectIndexedRustImport;
use semaprax_rust_api_index::{ItemKind, Receiver, RustApiIndex};

/// An explicit selected API plus the exact canonical Project source that owns
/// its import. The authenticated loader independently checks the source bytes.
#[derive(Clone, Copy)]
pub struct IndexedProjectScalarSelection<'a> {
    pub source_path: &'a str,
    pub source: &'a str,
    pub selection: IndexedScalarSelection<'a>,
}

/// Publish a native SDK from held Project files and exact selected package
/// instances. All ordinary Project checks and final source rechecks remain in
/// force; the Rust compiler verifies the real selected implementations.
pub fn build_indexed_project_native_rust_sdk(
    manifest_path: &Path,
    selections: &[IndexedProjectScalarSelection<'_>],
    output: &Path,
) -> Result<ProjectNativeRustSdkBundle, Vec<Diagnostic>> {
    let bindings = prepare_project_bindings(selections)?;
    semaprax::project::with_authenticated_indexed_rust_project(
        manifest_path,
        &bindings,
        |snapshot| {
            snapshot.with_authenticated_native_rust_sdk_subject(|input| {
                let subject = project::ProjectSdkSubject::from_authenticated(&input)?;
                project::verify_project_subject(subject.canonical.as_bytes(), &subject)
                    .map_err(|error| vec![error])?;
                let mut ordered = selections.iter().collect::<Vec<_>>();
                ordered.sort_by(|a, b| a.selection.import_id.cmp(b.selection.import_id));
                if ordered.len() != subject.imports.len()
                    || ordered
                        .iter()
                        .zip(&subject.imports)
                        .any(|(selection, id)| selection.selection.import_id != id)
                {
                    return Err(vec![sdk_error(
                        "indexed Project selections must cover the exact linked import set",
                    )]);
                }
                if input
                    .program()
                    .interfaces
                    .iter()
                    .flat_map(|i| &i.imports)
                    .any(|i| {
                        matches!(
                            i.result.kind,
                            semaprax::hir::ResolvedImportResultKind::OwnedResource { .. }
                        )
                    })
                {
                    let sdk = indexed_owner::build(input.program(), &subject, selections, output)?;
                    return Ok(ProjectNativeRustSdkBundle {
                        sdk,
                        project_revision: subject.project_revision.clone(),
                        workspace_revision: subject.workspace_revision.clone(),
                        subject_digest: subject.digest.clone(),
                    });
                }
                let mut plans = Vec::with_capacity(ordered.len());
                let mut sources = Vec::with_capacity(ordered.len());
                for selected in ordered {
                    let selection = selected.selection;
                    let import = input
                        .program()
                        .interfaces
                        .iter()
                        .flat_map(|interface| &interface.imports)
                        .find(|import| import.id.as_str() == selection.import_id)
                        .ok_or_else(|| {
                            vec![sdk_error(
                                "selected Project import is absent from linked HIR",
                            )]
                        })?;
                    let plan = prepare_indexed_scalar_binding(
                        import,
                        selection.index_bytes,
                        selection.package,
                        import.rust_path.as_deref().expect("checked indexed path"),
                    )
                    .map_err(|error| vec![error.at_path(selected.source_path)])?;
                    if plan.target != target_triple().unwrap_or("") {
                        return Err(vec![Diagnostic::error(
                            "SPX-B142",
                            "selected Project Rust target disagrees with native target",
                            import.span,
                        )
                        .at_path(selected.source_path)]);
                    }
                    plans.push(plan);
                    sources.push(
                        std::str::from_utf8(selection.package_source_bytes)
                            .expect("prepared UTF-8 source"),
                    );
                }
                let sdk = authority::build_indexed_project_sdk_inner(
                    input.program(),
                    &subject,
                    &plans,
                    &sources,
                    selections[0].selection.package.stable_rustc_version,
                    output,
                )
                .map_err(PublicBuildError::into_diagnostics)?;
                Ok(ProjectNativeRustSdkBundle {
                    sdk,
                    project_revision: subject.project_revision.clone(),
                    workspace_revision: subject.workspace_revision.clone(),
                    subject_digest: subject.digest.clone(),
                })
            })
        },
    )
}

pub(super) fn prepare_project_bindings(
    selections: &[IndexedProjectScalarSelection<'_>],
) -> Result<Vec<ProjectIndexedRustImport>, Vec<Diagnostic>> {
    if selections.is_empty() || selections.len() > MAX_IMPORTS {
        return Err(vec![sdk_error(
            "indexed Project selections exceed their bounds",
        )]);
    }
    let mut bindings = Vec::with_capacity(selections.len());
    for (position, selected) in selections.iter().enumerate() {
        let selection = selected.selection;
        let package = selection.package;
        if selected.source.len() > MAX_SOURCE_BYTES || selection.package_source_bytes.len() > 65_536
        {
            return Err(vec![sdk_error("indexed Project source exceeds its bound")]);
        }
        let program = semaprax::parse(selected.source, Path::new(selected.source_path))
            .map_err(|error| vec![error])?;
        let import = program
            .interfaces
            .iter()
            .flat_map(|interface| &interface.imports)
            .find(|import| import.stable_id == selection.import_id)
            .ok_or_else(|| vec![sdk_error("indexed Project import declaration is missing")])?;
        let located = |code, message| {
            vec![Diagnostic::error(code, message, import.span).at_path(selected.source_path)]
        };
        if !import.index_selected {
            return Err(located(
                "SPX-B140",
                "indexed Project selection requires a selected Rust declaration",
            ));
        }
        if raw_digest(selection.package_source_bytes) != package.source_sha256
            || package.stable_rustc_version != selections[0].selection.package.stable_rustc_version
        {
            return Err(located(
                "SPX-B142",
                "selected Project Rust source or compiler identity has drifted",
            ));
        }
        for previous in &selections[..position] {
            if previous.selection.package.cargo_alias == package.cargo_alias
                && (previous.selection.index_bytes != selection.index_bytes
                    || previous.selection.package_source_bytes != selection.package_source_bytes)
            {
                return Err(located(
                    "SPX-B142",
                    "Project Rust alias selects conflicting package instances",
                ));
            }
        }
        let source = std::str::from_utf8(selection.package_source_bytes)
            .map_err(|_| located("SPX-B142", "indexed Project Rust source must be UTF-8"))?;
        indexed::validate_embedded_scalar_source(source)
            .map_err(|message| located("SPX-B142", message))?;
        let index = RustApiIndex::replay(selection.index_bytes)
            .map_err(|_| located("SPX-B142", "selected Project Rust index replay failed"))?;
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
                    "selected Project Rust index identity has drifted",
                )
            })?;
        let path = import
            .rust_path
            .as_deref()
            .expect("selected source path parsed");
        let type_path = path.rsplit_once("::").map(|(owner, _)| owner).unwrap_or("");
        let regex_error = type_path
            .rsplit_once("::")
            .map(|(prefix, _)| format!("{prefix}::Error"))
            .unwrap_or_default();
        let item = match index.select_supported(&[path]) {
            Ok(items) => items[0],
            Err(_) if path.ends_with("::Regex::new") && !regex_error.is_empty() => index
                .select_closed_owner_result(path, type_path, &regex_error)
                .map_err(|_| {
                    located(
                        "SPX-B141",
                        "selected Project Regex constructor is unavailable",
                    )
                })?,
            Err(_) => {
                return Err(located(
                    "SPX-B141",
                    "selected Project Rust item is unavailable",
                ))
            }
        };
        if !matches!(
            (item.kind, item.receiver),
            (
                ItemKind::Function | ItemKind::InherentMethod,
                Receiver::None
            ) | (ItemKind::InherentMethod, Receiver::Shared | Receiver::Owned)
        ) {
            return Err(located(
                "SPX-B144",
                "selected Project Rust receiver is unsupported",
            ));
        }
        let owner_result = item
            .signature
            .rsplit_once(") -> ")
            .is_some_and(|(_, result)| {
                result == "Self" || Some(result) == path.rsplit_once("::").map(|p| p.0)
            });
        if item.receiver == Receiver::Owned || owner_result {
            indexed_owner::require_owner_type(&index, item)
                .map_err(|message| located("SPX-B145", message))?;
        }
        bindings.push(ProjectIndexedRustImport {
            source_path: selected.source_path.into(),
            source_sha256: raw_digest(selected.source.as_bytes()),
            import_id: selection.import_id.into(),
            rust_path: path.into(),
            signature: item.signature.clone(),
            index_digest: index.digest().into(),
            receiver: if item.receiver == Receiver::Owned {
                "owned"
            } else if item.receiver == Receiver::Shared {
                "shared"
            } else {
                "none"
            }
            .into(),
        });
    }
    Ok(bindings)
}
