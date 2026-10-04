//! Authenticated Project publication with explicit RI-03 selected imports.

use super::*;
use crate::indexed_binding::prepare_indexed_scalar_binding;
use semaprax::assurance_manifest::law_set::{strict::StrictLawPolicy, LawSet};
use semaprax::hir::ResolvedImportResultKind;
use semaprax::native_rust_binding::foreign_law::{
    DeclaredForeignSummary, ForeignLawFrontier, ForeignLawRequest,
};
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

type IndexedProjectBuild = (
    ProjectNativeRustSdkBundle,
    Option<ForeignLawFrontier>,
    Option<(super::GuardedForeignCallerEvidence, String)>,
);

/// Publish a native SDK from held Project files and exact selected package
/// instances. All ordinary Project checks and final source rechecks remain in
/// force; the Rust compiler verifies the real selected implementations.
pub fn build_indexed_project_native_rust_sdk(
    manifest_path: &Path,
    selections: &[IndexedProjectScalarSelection<'_>],
    output: &Path,
) -> Result<ProjectNativeRustSdkBundle, Vec<Diagnostic>> {
    build_indexed_project_native_rust_sdk_inner(manifest_path, selections, None, None, output)
        .map(|(bundle, _, _)| bundle)
}

/// One conditional foreign declaration retained in a generated SDK return
/// guard. The declaration is still an assumption about the Rust implementation.
#[derive(Clone, Copy)]
pub struct GuardedForeignLawSelection<'a> {
    pub import_id: &'a str,
    pub declared: &'a DeclaredForeignSummary,
    pub law: &'a ForeignLawRequest,
}

/// Publish an authenticated Project SDK with one exact i64 return guard.
/// The returned frontier binds the guard to the published SDK manifest digest.
pub fn build_guarded_indexed_project_native_rust_sdk(
    manifest_path: &Path,
    selections: &[IndexedProjectScalarSelection<'_>],
    guard: GuardedForeignLawSelection<'_>,
    output: &Path,
) -> Result<(ProjectNativeRustSdkBundle, ForeignLawFrontier), Vec<Diagnostic>> {
    let (bundle, frontier, _) = build_indexed_project_native_rust_sdk_inner(
        manifest_path,
        selections,
        Some(guard),
        None,
        output,
    )?;
    Ok((bundle, frontier.expect("guarded build retains frontier")))
}

/// Selected Project SDK publication: the host's exact conditional law policy
/// is checked against the authenticated staged manifest before the final
/// no-clobber publish. The returned opaque token and report are replayed after
/// publication; a failed preflight leaves the requested output absent.
pub fn build_guarded_indexed_project_native_rust_sdk_with_law_policy(
    manifest_path: &Path,
    selections: &[IndexedProjectScalarSelection<'_>],
    guard: GuardedForeignLawSelection<'_>,
    caller_id: &str,
    laws: &LawSet,
    policy: &StrictLawPolicy,
    output: &Path,
) -> Result<
    (
        ProjectNativeRustSdkBundle,
        ForeignLawFrontier,
        super::GuardedForeignCallerEvidence,
        String,
    ),
    Vec<Diagnostic>,
> {
    let (bundle, frontier, selected) = build_indexed_project_native_rust_sdk_inner(
        manifest_path,
        selections,
        Some(guard),
        Some((caller_id, laws, policy)),
        output,
    )?;
    let (evidence, report) = selected.expect("selected guarded build retains conditional law");
    Ok((
        bundle,
        frontier.expect("guarded build retains frontier"),
        evidence,
        report,
    ))
}

fn build_indexed_project_native_rust_sdk_inner(
    manifest_path: &Path,
    selections: &[IndexedProjectScalarSelection<'_>],
    guard: Option<GuardedForeignLawSelection<'_>>,
    selected_policy: Option<(&str, &LawSet, &StrictLawPolicy)>,
    output: &Path,
) -> Result<IndexedProjectBuild, Vec<Diagnostic>> {
    let bindings = prepare_project_bindings(selections)?;
    semaprax::project::with_authenticated_indexed_rust_project(
        manifest_path,
        &bindings,
        |snapshot| {
            let revision = snapshot.retain_revision();
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
                    if guard.is_some() {
                        return Err(vec![sdk_error("foreign i64 return guard requires scalar indexed Project imports")]);
                    }
                    let sdk = indexed_owner::build(input.program(), &subject, selections, output)?;
                    return Ok((ProjectNativeRustSdkBundle {
                        sdk,
                        project_revision: subject.project_revision.clone(),
                        workspace_revision: subject.workspace_revision.clone(),
                        subject_digest: subject.digest.clone(),
                        guarded_frontier: None,
                    }, None, None));
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
                let checked_guard = if let Some(guard) = guard {
                    let plan = plans.iter().find(|plan| plan.import_id == guard.import_id)
                        .ok_or_else(|| vec![sdk_error("foreign guard import is absent from exact Project selections")])?;
                    let import = input.program().interfaces.iter().flat_map(|interface| &interface.imports)
                        .find(|import| import.id.as_str() == guard.import_id)
                        .ok_or_else(|| vec![sdk_error("foreign guard import is absent from linked HIR")])?;
                    if import.result.kind != ResolvedImportResultKind::I64
                        || !guard.law.require_return_guard
                        || guard.declared.return_i64_range.is_none()
                    {
                        return Err(vec![sdk_error("foreign guard requires an i64 import and explicit retained return range")]);
                    }
                    // Check all law conditions before publication. The final
                    // frontier is rederived with the published artifact digest.
                    revision.foreign_law_frontier(
                        plan, plan.target.as_str(),
                        "sha256:0000000000000000000000000000000000000000000000000000000000000000",
                        guard.declared, guard.law,
                    )?;
                    let (minimum, maximum) = guard.declared.return_i64_range.expect("checked range");
                    Some(package::ForeignReturnGuard {
                        import_id: guard.import_id,
                        assumption_id: &guard.declared.assumption_id,
                        proposition_digest: &guard.declared.proposition_digest,
                        minimum,
                        maximum,
                    })
                } else {
                    None
                };
                if let Some((caller_id, laws, policy)) = selected_policy {
                    let guard = guard.expect("selected foreign guard");
                    let plan = plans.iter().find(|plan| plan.import_id == guard.import_id)
                        .expect("checked guard selection");
                    let Some(semaprax::assurance_manifest::law_set::strict::RequiredLawEvidence::ForeignConditionalGuard { adapter_digest, .. }) = policy.requirements().get(&guard.law.law_id) else {
                        return Err(vec![sdk_error("selected foreign law policy lacks exact conditional requirement")]);
                    };
                    let caller = revision.foreign_caller_certificate(
                        caller_id, plan, plan.target.as_str(), adapter_digest,
                        guard.declared, guard.law,
                    )?;
                    super::foreign_law::preflight_conditional_strict_law(
                        &caller, adapter_digest, &revision, laws, policy,
                    )?;
                }
                let prepublish = |digest: &str| -> Result<(), Diagnostic> {
                    let (caller_id, laws, policy) = selected_policy.expect("selected prepublish policy");
                    let guard = guard.expect("selected foreign guard");
                    let plan = plans.iter().find(|plan| plan.import_id == guard.import_id)
                        .expect("checked guard selection");
                    let caller = revision.foreign_caller_certificate(
                        caller_id, plan, plan.target.as_str(), digest, guard.declared, guard.law,
                    ).map_err(|errors| errors.into_iter().next().unwrap_or_else(|| sdk_error("conditional foreign caller preflight failed")))?;
                    super::foreign_law::preflight_conditional_strict_law(
                        &caller, digest, &revision, laws, policy,
                    ).map_err(|errors| errors.into_iter().next().unwrap_or_else(|| sdk_error("conditional foreign law preflight failed")))
                };
                let policy_hook = selected_policy.map(|_| &prepublish as &dyn Fn(&str) -> Result<(), Diagnostic>);
                let sdk = authority::build_indexed_project_sdk_inner(
                    input.program(),
                    &subject,
                    &plans,
                    &sources,
                    selections[0].selection.package.stable_rustc_version,
                    checked_guard,
                    policy_hook,
                    output,
                )
                .map_err(PublicBuildError::into_diagnostics)?;
                let frontier = if let Some(guard) = guard {
                    let plan = plans.iter().find(|plan| plan.import_id == guard.import_id)
                        .expect("checked guard selection");
                    Some(revision.foreign_law_frontier(
                        plan, plan.target.as_str(), sdk.manifest_digest(), guard.declared, guard.law,
                    )?)
                } else {
                    None
                };
                let bundle = ProjectNativeRustSdkBundle {
                    sdk,
                    project_revision: subject.project_revision.clone(),
                    workspace_revision: subject.workspace_revision.clone(),
                    subject_digest: subject.digest.clone(),
                    guarded_frontier: frontier.clone(),
                };
                let selected = if let Some((caller_id, laws, policy)) = selected_policy {
                    let guard = guard.expect("selected foreign guard");
                    let plan = plans.iter().find(|plan| plan.import_id == guard.import_id)
                        .expect("checked guard selection");
                    let caller = revision.foreign_caller_certificate(
                        caller_id, plan, plan.target.as_str(), bundle.manifest_digest(),
                        guard.declared, guard.law,
                    )?;
                    let evidence = bundle.bind_guarded_foreign_caller(&revision, caller)?;
                    let report = evidence.derive_conditional_strict_law_report(&revision, laws, policy)?;
                    Some((evidence, report))
                } else {
                    None
                };
                Ok((bundle, frontier, selected))
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

/// Registry-backed selected import facts. Unlike `IndexedProjectScalarSelection`,
/// this intentionally carries no embedded crate source: the registry closure is
/// authenticated by RI-11's lock/source records.
#[derive(Clone, Copy)]
pub struct IndexedProjectRegexRegistrySelection<'a> {
    pub source_path: &'a str,
    pub source: &'a str,
    pub import_id: &'a str,
    pub index_bytes: &'a [u8],
    pub package: crate::indexed_binding::SelectedPackage<'a>,
}

pub(super) fn indexed_regex_project_bindings<'a>(
    selections: &[IndexedProjectRegexRegistrySelection<'a>],
) -> Result<
    (
        IndexedProjectRegexRegistrySelection<'a>,
        Vec<semaprax::project::ProjectIndexedRustImport>,
        Vec<String>,
    ),
    Vec<semaprax::diagnostic::Diagnostic>,
> {
    use semaprax::project::ProjectIndexedRustImport;
    use semaprax_rust_api_index::RustApiIndex;
    if selections.len() != 2
        || selections
            .iter()
            .any(|selection| selection.source.len() > MAX_SOURCE_BYTES)
    {
        return Err(vec![sdk_error(
            "selected Regex Project requires exactly constructor and matcher bindings",
        )]);
    }
    let first = selections[0];
    if selections.iter().any(|selection| {
        selection.package.cargo_alias != first.package.cargo_alias
            || selection.package.name != first.package.name
            || selection.package.version != first.package.version
            || selection.package.source_sha256 != first.package.source_sha256
            || selection.package.target != first.package.target
            || selection.package.feature_digest != first.package.feature_digest
            || selection.package.stable_rustc_version != first.package.stable_rustc_version
            || selection.index_bytes != first.index_bytes
    }) {
        return Err(vec![sdk_error(
            "selected Regex Project bindings must share one exact registry package",
        )]);
    }
    let index = RustApiIndex::replay(first.index_bytes)
        .map_err(|_| vec![sdk_error("selected Regex Project index replay failed")])?;
    index
        .require_package_identity(
            first.package.name,
            first.package.version,
            first.package.source_sha256,
            first.package.target,
            first.package.feature_digest,
        )
        .and_then(|_| index.require_cargo_alias_identity(first.package.cargo_alias))
        .and_then(|_| index.require_stable_compiler_identity(first.package.stable_rustc_version))
        .map_err(|_| {
            vec![sdk_error(
                "selected Regex Project registry identity drifted",
            )]
        })?;
    let mut bindings = Vec::with_capacity(2);
    let mut ids = Vec::with_capacity(2);
    for selection in selections {
        let program = semaprax::parse(
            selection.source,
            std::path::Path::new(selection.source_path),
        )
        .map_err(|error| vec![error])?;
        let import = program
            .interfaces
            .iter()
            .flat_map(|interface| &interface.imports)
            .find(|import| import.stable_id == selection.import_id)
            .ok_or_else(|| {
                vec![sdk_error(
                    "selected Regex Project import declaration is missing",
                )]
            })?;
        let located = |code, message| {
            vec![
                semaprax::diagnostic::Diagnostic::error(code, message, import.span)
                    .at_path(selection.source_path),
            ]
        };
        if !import.index_selected || import.rust_path.is_none() {
            return Err(located(
                "SPX-B140",
                "selected Regex Project import must name an indexed Rust declaration",
            ));
        }
        let path = import.rust_path.as_deref().expect("checked");
        let item = match path {
            "regex_alias::Regex::new" => index.select_closed_owner_result(
                "regex::Regex::new",
                "regex::Regex",
                "regex::Error",
            ),
            "regex_alias::Regex::is_match" => index
                .select_supported(&["regex::Regex::is_match"])
                .map(|items| items[0]),
            _ => {
                return Err(located(
                    "SPX-B141",
                    "selected Regex Project API is outside the RI-06 profile",
                ))
            }
        }
        .map_err(|_| located("SPX-B141", "selected Regex Project API is unavailable"))?;
        let expected = if path.ends_with("::new") {
            "fn new(re: &str) -> core::result::Result<regex::Regex, regex::Error>"
        } else {
            "fn is_match(&self, haystack: &str) -> bool"
        };
        if item.signature != expected {
            return Err(located(
                "SPX-B145",
                "selected Regex Project signature is outside the RI-06 profile",
            ));
        }
        ids.push(selection.import_id.to_owned());
        bindings.push(ProjectIndexedRustImport {
            source_path: selection.source_path.into(),
            source_sha256: raw_digest(selection.source.as_bytes()),
            import_id: selection.import_id.into(),
            rust_path: path.into(),
            signature: item.signature.clone(),
            index_digest: index.digest().into(),
            receiver: if path.ends_with("::new") {
                "none"
            } else {
                "shared"
            }
            .into(),
        });
    }
    ids.sort();
    Ok((first, bindings, ids))
}

pub fn prepare_indexed_regex_project_package(
    manifest_path: &std::path::Path,
    selections: &[IndexedProjectRegexRegistrySelection<'_>],
    cargo_lock: &[u8],
) -> Result<PreparedRegexProjectPackage, Vec<semaprax::diagnostic::Diagnostic>> {
    let (first, bindings, ids) = indexed_regex_project_bindings(selections)?;
    semaprax::project::with_authenticated_indexed_rust_project(
        manifest_path,
        &bindings,
        |snapshot| {
            snapshot.with_authenticated_native_rust_sdk_subject(|input| {
                let subject = project::ProjectSdkSubject::from_authenticated(&input)?;
                project::verify_project_subject(subject.canonical.as_bytes(), &subject)
                    .map_err(|error| vec![error])?;
                if subject.imports != ids {
                    return Err(vec![sdk_error(
                        "selected Regex Project bindings do not cover the authenticated import set",
                    )]);
                }
                if subject.exports.len() != 1 {
                    return Err(vec![sdk_error(
                        "selected Regex Project requires one exact scalar export",
                    )]);
                }
                regex_project_package::prepare_regex_project_package(
                    input.program(),
                    &subject.exports[0].id,
                    subject.canonical.as_bytes(),
                    &subject.digest,
                    &subject.manifest,
                    first.index_bytes,
                    first.package.cargo_alias,
                    first.package.source_sha256,
                    first.package.target,
                    first.package.stable_rustc_version,
                    cargo_lock,
                    false,
                )
            })
        },
    )
}
