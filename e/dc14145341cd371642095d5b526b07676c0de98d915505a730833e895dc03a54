use super::*;

pub(super) fn load_snapshot_building<T>(
    manifest_path: &Path,
    access: ProjectHostAccess,
    build: impl FnOnce(
        ProjectManifest,
        Vec<SemanticWorkspaceSource>,
    ) -> Result<(Arc<ProjectRevision>, T), Vec<Diagnostic>>,
) -> Result<(ProjectSnapshot, T), Vec<Diagnostic>> {
    let manifest_selection = DeclaredPathSelection::open(manifest_path, "manifest")?;
    let manifest_path = manifest_selection.canonical_path.clone();
    if manifest_path.file_name().and_then(|name| name.to_str()) != Some(MANIFEST_FILE) {
        return Err(grammar("Project v1 manifest path must name semaprax.toml"));
    }
    let root = manifest_path
        .parent()
        .ok_or_else(|| grammar("Project v1 manifest must have an explicit project root"))?
        .to_path_buf();
    let host_policy = host_policy::SelectedStrictLaw::open(&root)?;
    if host_policy.is_some() && access == ProjectHostAccess::Generic {
        return Err(host_policy::selected_route_refused());
    }
    if host_policy.is_none() && access == ProjectHostAccess::Strict {
        return Err(host_policy::strict_route_unselected());
    }
    let mut root_ancestors = root.ancestors().map(Path::to_path_buf).collect::<Vec<_>>();
    if root_ancestors.len() > MAX_HELD_DIRECTORIES {
        return Err(capacity("ancestor_directories", MAX_HELD_DIRECTORIES));
    }
    root_ancestors.reverse();
    let mut held_directories = root_ancestors
        .iter()
        .cloned()
        .map(HeldDirectory::open)
        .collect::<Result<Vec<_>, _>>()?;
    let mut held_manifest = HeldFile::open(manifest_path.clone(), MAX_MANIFEST_BYTES)?;
    if held_manifest.identity != manifest_selection.identity {
        return Err(authentication(
            "Project v1 manifest selection changed while opening",
        ));
    }
    let manifest_text = held_manifest.utf8()?;
    let manifest = ProjectManifest::parse(&manifest_text)?;

    let mut held_sources = Vec::with_capacity(manifest.sources().len());
    let mut declared_inputs = vec![manifest_selection];
    let mut workspace_sources = Vec::with_capacity(manifest.sources().len());
    let mut seen_directories = root_ancestors.into_iter().collect::<BTreeSet<_>>();
    let mut total_source_bytes = 0usize;
    for relative in manifest.sources() {
        let relative_path = Path::new(relative);
        let mut ancestor = root.clone();
        if let Some(parent) = relative_path.parent() {
            for component in parent.components() {
                ancestor.push(component.as_os_str());
                if seen_directories.insert(ancestor.clone()) {
                    if seen_directories.len() > MAX_HELD_DIRECTORIES {
                        return Err(capacity("ancestor_directories", MAX_HELD_DIRECTORIES));
                    }
                    held_directories.push(HeldDirectory::open(ancestor.clone())?);
                }
            }
        }
        let selection = DeclaredPathSelection::open(&root.join(relative_path), "source")?;
        let path = selection.canonical_path.clone();
        // Each source is bounded by the *remaining* shared budget, not the
        // whole aggregate constant, so one large source cannot consume the
        // entire multi-file allowance before the total check fires.
        let remaining_source_bytes = MAX_TOTAL_SOURCE_BYTES - total_source_bytes;
        let mut held = HeldFile::open(path, remaining_source_bytes)?;
        if held.identity != selection.identity {
            return Err(authentication(format!(
                "Project v1 source {relative} selection changed while opening"
            )));
        }
        if held.identity == held_manifest.identity
            || held_sources
                .iter()
                .any(|existing: &HeldFile| existing.identity == held.identity)
        {
            return Err(authentication(
                "Project v1 source paths resolve to one physical file",
            ));
        }
        let source = held.utf8()?;
        total_source_bytes = total_source_bytes
            .checked_add(source.len())
            .ok_or_else(|| capacity("total_source_bytes", MAX_TOTAL_SOURCE_BYTES))?;
        if total_source_bytes > MAX_TOTAL_SOURCE_BYTES {
            return Err(capacity("total_source_bytes", MAX_TOTAL_SOURCE_BYTES));
        }
        workspace_sources.push(SemanticWorkspaceSource {
            path: relative.clone(),
            source,
        });
        held_sources.push(held);
        declared_inputs.push(selection);
    }

    let mut held_dependency_sources = Vec::with_capacity(manifest.dependency_sources().len());
    let mut dependency_inputs = Vec::with_capacity(manifest.dependency_sources().len());
    let mut total_subject_bytes = 0usize;
    for dependency in manifest.dependency_sources() {
        let relative_path = Path::new(dependency.path());
        let mut ancestor = root.clone();
        if let Some(parent) = relative_path.parent() {
            for component in parent.components() {
                ancestor.push(component.as_os_str());
                if seen_directories.insert(ancestor.clone()) {
                    if seen_directories.len() > MAX_HELD_DIRECTORIES {
                        return Err(capacity("ancestor_directories", MAX_HELD_DIRECTORIES));
                    }
                    held_directories.push(HeldDirectory::open(ancestor.clone())?);
                }
            }
        }
        let selection =
            DeclaredPathSelection::open(&root.join(relative_path), "SEMAPRAX dependency subject")?;
        let path = selection.canonical_path.clone();
        let remaining_subject_bytes = crate::package_resolver_v2::MAX_TOTAL_SUBJECT_BYTES
            .checked_sub(total_subject_bytes)
            .ok_or_else(|| {
                capacity(
                    "dependency_subject_bytes",
                    crate::package_resolver_v2::MAX_TOTAL_SUBJECT_BYTES,
                )
            })?;
        let mut held = HeldFile::open(
            path,
            remaining_subject_bytes.min(crate::package_lock_v3::MAX_SUBJECT_BYTES),
        )?;
        if held.identity != selection.identity {
            return Err(authentication(format!(
                "SEMAPRAX dependency subject {} changed while opening",
                dependency.name()
            )));
        }
        if held.identity == held_manifest.identity
            || held_sources
                .iter()
                .any(|existing| existing.identity == held.identity)
            || held_dependency_sources
                .iter()
                .any(|existing: &HeldFile| existing.identity == held.identity)
        {
            return Err(authentication(
                "Project manifest, sources, and dependency subjects must be distinct physical files",
            ));
        }
        let bytes = held.utf8()?;
        total_subject_bytes = total_subject_bytes
            .checked_add(bytes.len())
            .ok_or_else(|| {
                capacity(
                    "dependency_subject_bytes",
                    crate::package_resolver_v2::MAX_TOTAL_SUBJECT_BYTES,
                )
            })?;
        dependency_inputs.push(external_dependencies::HeldDependencySubject {
            declared_name: dependency.name().to_owned(),
            bytes,
        });
        held_dependency_sources.push(held);
        declared_inputs.push(selection);
    }
    let dependency_sources = external_dependencies::resolve(&manifest, dependency_inputs)?;
    for source in &dependency_sources {
        total_source_bytes = total_source_bytes
            .checked_add(source.source.len())
            .ok_or_else(|| capacity("total_source_bytes", MAX_TOTAL_SOURCE_BYTES))?;
        if total_source_bytes > MAX_TOTAL_SOURCE_BYTES {
            return Err(capacity("total_source_bytes", MAX_TOTAL_SOURCE_BYTES));
        }
    }
    workspace_sources.extend(dependency_sources);
    if workspace_sources.len() > MAX_SOURCES {
        return Err(capacity("resolved_sources", MAX_SOURCES));
    }

    let declared_sources = manifest.sources().to_vec();
    let (revision, result) = build(manifest, workspace_sources).map_err(|errors| {
        let errors = source_hint::hint_unlisted_module(errors, &root, &declared_sources);
        source_hint::hint_importable_function(errors, &root, &declared_sources)
    })?;
    let mut snapshot = ProjectSnapshot {
        root,
        revision,
        host_policy,
        declared_inputs,
        held_manifest,
        held_sources,
        held_dependency_sources,
        held_directories,
        published_subject: None,
        request_invalidation: None,
    };
    snapshot.recheck()?;
    Ok((snapshot, result))
}
