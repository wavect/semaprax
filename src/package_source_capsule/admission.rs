use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostic::Diagnostic;
use crate::package_lock_v2::Coordinate;
use crate::package_resolver::{ResolutionInput, ResolutionOptions};
use crate::workspace_graph::WorkspaceSource;

use super::model::{
    BuiltCapsule, LinkedPackageImportFact, LinkedPackageSourceFact, PackageSource,
    SourceCapsuleOptions, VerifiedSourceCapsule, MAX_OUTPUT_BYTES, MAX_PACKAGES, MAX_SOURCE_BYTES,
    MAX_TOTAL_SOURCE_BYTES, MIN_OUTPUT_BYTES, MIN_PACKAGES,
};

pub(crate) fn validate_options(options: &SourceCapsuleOptions) -> Result<(), Diagnostic> {
    if !(MIN_OUTPUT_BYTES..=MAX_OUTPUT_BYTES).contains(&options.max_bytes) {
        return Err(super::option_error(
            "package-source capsule max_bytes is outside the frozen range",
        ));
    }
    if options.root_package.len() > 255
        || crate::workspace_graph::validate_entry_module(&options.root_package).is_err()
    {
        return Err(super::option_error(
            "package-source root package is outside the canonical module grammar",
        ));
    }
    Ok(())
}

pub(crate) fn build(
    sources: &[PackageSource],
    resolution_evidence: &str,
    resolution_input: &ResolutionInput,
    resolution_options: &ResolutionOptions,
    options: &SourceCapsuleOptions,
) -> Result<BuiltCapsule, Diagnostic> {
    validate_options(options)?;
    validate_source_input(sources)?;
    if resolution_input.target != "wasm32" || !resolution_input.allowed_capabilities.is_empty() {
        return Err(super::profile_error(
            "package-source resolution must select wasm32 with an empty capability allowlist",
        ));
    }
    let selected = crate::package_resolver::verify_for_package_source(
        resolution_evidence,
        resolution_input,
        resolution_options,
    )
    .map_err(|error| super::map_nested_error(&error))?;
    if selected.resolution.packages.len() < MIN_PACKAGES
        || selected.resolution.packages.len() > MAX_PACKAGES
        || selected.resolution.packages.len() != sources.len()
        || selected.selected_subjects.len() != sources.len()
    {
        return Err(super::association_error(
            "package-source selected packages and source inventory disagree",
        ));
    }
    if !selected
        .resolution
        .packages
        .iter()
        .any(|coordinate| coordinate.package == options.root_package)
    {
        return Err(super::association_error(
            "package-source root package is not resolver-selected",
        ));
    }

    let mut workspace_sources = Vec::with_capacity(sources.len());
    let mut coordinates = BTreeMap::new();
    for (index, ((source, coordinate), subject)) in sources
        .iter()
        .zip(&selected.resolution.packages)
        .zip(&selected.selected_subjects)
        .enumerate()
    {
        if source.package != coordinate.package
            || subject.coordinate != *coordinate
            || source.report != subject.report
        {
            return Err(super::association_error(
                "package-source source, selected coordinate, subject, or report disagree",
            ));
        }
        if !subject.capabilities.is_empty() {
            return Err(super::profile_error(
                "package-source selected capabilities must be empty",
            ));
        }
        coordinates.insert(coordinate.package.clone(), coordinate.clone());
        workspace_sources.push(WorkspaceSource {
            path: format!("package-{index:03}.spx"),
            source: source.source.clone(),
        });
    }

    let workspace = crate::workspace_graph::build_package_scalar_sources(
        workspace_sources,
        &options.root_package,
    )
    .map_err(|errors| {
        errors.into_iter().next().map_or_else(
            || super::profile_error("package-source link failed"),
            super::map_graph_error,
        )
    })?;
    if workspace.imports.len() > super::MAX_IMPORTS {
        return Err(super::limit_error(
            "package-source function import inventory exceeds the frozen bound",
        ));
    }
    let modules = workspace
        .modules
        .iter()
        .map(|module| (module.package.as_str(), module))
        .collect::<BTreeMap<_, _>>();
    if modules.len() != sources.len() {
        return Err(super::association_error(
            "package-source linked module inventory disagrees with selected packages",
        ));
    }
    for subject in &selected.selected_subjects {
        let module = modules
            .get(subject.coordinate.package.as_str())
            .ok_or_else(|| {
                super::association_error("package-source selected package has no linked module")
            })?;
        if module.interface.functions != subject.interface.functions
            || module.interface.digest != subject.interface.digest
        {
            return Err(super::association_error(
                "package-source implementation interface differs from selected Report v2",
            ));
        }
    }

    let actual_dependencies = workspace
        .imports
        .iter()
        .map(|import| (import.dependent.clone(), import.dependency.clone()))
        .collect::<BTreeSet<_>>();
    let expected_dependencies = selected
        .selected_subjects
        .iter()
        .flat_map(|subject| {
            subject.dependencies.iter().map(|dependency| {
                (
                    subject.coordinate.package.clone(),
                    dependency.package.clone(),
                )
            })
        })
        .collect::<BTreeSet<_>>();
    if actual_dependencies != expected_dependencies {
        return Err(super::association_error(
            "package-source direct imports differ from selected dependency metadata",
        ));
    }
    let import_facts = workspace
        .imports
        .iter()
        .map(|import| {
            Ok(LinkedPackageImportFact {
                dependent: coordinates
                    .get(&import.dependent)
                    .cloned()
                    .ok_or_else(|| super::association_error("import dependent is unselected"))?,
                dependency: coordinates
                    .get(&import.dependency)
                    .cloned()
                    .ok_or_else(|| super::association_error("import dependency is unselected"))?,
                target: import.target.clone(),
                alias: import.alias.clone(),
                ordinal: import.ordinal,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;

    let program = workspace.link_root_exports().map_err(|errors| {
        errors.into_iter().next().map_or_else(
            || super::profile_error("package-source export link failed"),
            super::map_graph_error,
        )
    })?;
    let linked_function_ids = program
        .functions
        .iter()
        .map(|function| function.id.as_str().to_owned())
        .collect::<Vec<_>>();

    let (rendered, overflowed) = crate::bounded_output::with_limit(super::MAX_RENDER_BYTES, || {
        let mut package_facts = Vec::with_capacity(sources.len());
        for ((source, coordinate), subject) in sources
            .iter()
            .zip(&selected.resolution.packages)
            .zip(&selected.selected_subjects)
        {
            package_facts.push(LinkedPackageSourceFact {
                coordinate: Coordinate {
                    package: crate::bounded_output::budgeted_clone(&coordinate.package),
                    version: crate::bounded_output::budgeted_clone(&coordinate.version),
                },
                subject_digest: crate::bounded_output::budgeted_clone(&subject.subject_digest),
                report_digest: crate::bounded_output::budgeted_clone(&subject.report_digest),
                interface_digest: crate::bounded_output::budgeted_clone(&subject.interface.digest),
                interface_source_revision: crate::bounded_output::budgeted_clone(
                    &subject.interface.source_revision,
                ),
                source_revision: crate::bounded_output::budgeted_clone(
                    &crate::graph::revision_from_canonical_source(&source.source),
                ),
                source_digest: super::wire::source_digest(&source.source),
                source_bytes: source.source.len(),
            });
        }
        let source_set_rows = package_facts
            .iter()
            .zip(sources)
            .map(|(fact, source)| (&fact.coordinate, source.source.as_str()))
            .collect::<Vec<_>>();
        let source_set_digest = super::wire::source_set_digest(&source_set_rows);
        let link_digest = super::wire::link_digest(
            &source_set_digest,
            &options.root_package,
            &workspace.modules,
            &workspace.imports,
            &linked_function_ids,
        );
        let resolution_digest = super::wire::wrapper_digest(resolution_evidence)?;
        let lock_digest = super::wire::wrapper_digest(&selected.resolution.lock)?;
        let json = super::wire::render(super::wire::RenderInput {
            resolution_digest: &resolution_digest,
            resolution_bytes: resolution_evidence.len(),
            lock_digest: &lock_digest,
            lock_bytes: selected.resolution.lock.len(),
            options,
            facts: &package_facts,
            imports: &import_facts,
            linked_function_ids: &linked_function_ids,
            sources,
            source_set_digest: &source_set_digest,
            link_digest: &link_digest,
        })?;
        let digest = super::wire::wrapper_digest(&json)?;
        Ok((json, digest, package_facts, source_set_digest, link_digest))
    });
    if overflowed {
        return Err(super::limit_error(
            "package-source capsule render budget exceeded",
        ));
    }
    let (json, digest, package_facts, source_set_digest, link_digest) = rendered?;
    if json.len() > options.max_bytes || json.len() > MAX_OUTPUT_BYTES {
        return Err(super::limit_error(
            "package-source capsule exceeds output bound",
        ));
    }
    let packages = selected.resolution.packages.clone();
    let source_revisions = package_facts
        .iter()
        .map(|fact| (fact.coordinate.clone(), fact.source_revision.clone()))
        .collect();
    let exports = workspace.root_exports.clone();
    let receipt = VerifiedSourceCapsule::new(super::model::VerifiedSourceCapsuleFacts {
        digest,
        bytes: json.len(),
        source_set_digest,
        link_digest,
        root_package: options.root_package.clone(),
        packages,
        source_revisions,
        exports,
    });
    Ok(BuiltCapsule {
        json,
        receipt,
        program,
        selected_subjects: selected.selected_subjects,
        package_facts,
        import_facts,
    })
}

fn validate_source_input(sources: &[PackageSource]) -> Result<(), Diagnostic> {
    if !(MIN_PACKAGES..=MAX_PACKAGES).contains(&sources.len()) {
        return Err(super::option_error(
            "package-source input must contain 2..=4 packages",
        ));
    }
    let mut previous = None;
    let mut total = 0usize;
    for source in sources {
        if source.package.len() > 255
            || crate::workspace_graph::validate_entry_module(&source.package).is_err()
        {
            return Err(super::option_error(
                "package-source package is outside the canonical module grammar",
            ));
        }
        if previous.is_some_and(|value: &String| value.as_bytes() >= source.package.as_bytes()) {
            return Err(super::option_error(
                "package-source packages must be strictly byte-sorted and unique",
            ));
        }
        previous = Some(&source.package);
        if source.source.len() > MAX_SOURCE_BYTES {
            return Err(super::limit_error(
                "package-source source exceeds per-source bound",
            ));
        }
        total = total.checked_add(source.source.len()).ok_or_else(|| {
            super::limit_error("package-source total source byte accounting overflowed")
        })?;
        if total > MAX_TOTAL_SOURCE_BYTES {
            return Err(super::limit_error(
                "package-source total source bytes exceed bound",
            ));
        }
    }
    Ok(())
}
