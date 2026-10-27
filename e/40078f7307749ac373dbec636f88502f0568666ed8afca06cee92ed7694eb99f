use super::*;

pub(super) fn load_snapshot_building<T>(
    manifest_path: &Path,
    access: ProjectHostAccess,
    build: impl FnOnce(
        ProjectManifest,
        Vec<SemanticWorkspaceSource>,
    ) -> Result<(Arc<ProjectRevision>, T), Vec<Diagnostic>>,
) -> Result<(ProjectSnapshot, T), Vec<Diagnostic>> {
    load_snapshot_building_with_controls(manifest_path, access, |manifest, sources, _| {
        build(manifest, sources)
    })
}

pub(super) fn load_snapshot_building_with_controls<T, E: SnapshotFailure>(
    manifest_path: &Path,
    access: ProjectHostAccess,
    build: impl FnOnce(
        ProjectManifest,
        Vec<SemanticWorkspaceSource>,
        usize,
    ) -> Result<(Arc<ProjectRevision>, T), E>,
) -> Result<(ProjectSnapshot, T), E> {
    let manifest_selection = DeclaredPathSelection::open(manifest_path, "manifest")?;
    let manifest_path = manifest_selection.canonical_path.clone();
    if manifest_path.file_name().and_then(|name| name.to_str()) != Some(MANIFEST_FILE) {
        return Err(grammar("Project v1 manifest path must name semaprax.toml").into());
    }
    let root = manifest_path
        .parent()
        .ok_or_else(|| grammar("Project v1 manifest must have an explicit project root"))?
        .to_path_buf();
    let host_policy = host_policy::SelectedStrictLaw::open(&root)?;
    if host_policy.is_some() && access == ProjectHostAccess::Generic {
        return Err(host_policy::selected_route_refused().into());
    }
    if host_policy.is_none() && access == ProjectHostAccess::Strict {
        return Err(host_policy::strict_route_unselected().into());
    }
    let mut root_ancestors = root.ancestors().map(Path::to_path_buf).collect::<Vec<_>>();
    if root_ancestors.len() > MAX_HELD_DIRECTORIES {
        return Err(capacity("ancestor_directories", MAX_HELD_DIRECTORIES).into());
    }
    root_ancestors.reverse();
    let mut held_directories = root_ancestors
        .iter()
        .cloned()
        .map(HeldDirectory::open)
        .collect::<Result<Vec<_>, _>>()?;
    let mut held_manifest = HeldFile::open(manifest_path.clone(), MAX_MANIFEST_BYTES)?;
    if held_manifest.identity != manifest_selection.identity {
        return Err(authentication("Project v1 manifest selection changed while opening").into());
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
                        return Err(capacity("ancestor_directories", MAX_HELD_DIRECTORIES).into());
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
            ))
            .into());
        }
        if held.identity == held_manifest.identity
            || held_sources
                .iter()
                .any(|existing: &HeldFile| existing.identity == held.identity)
        {
            return Err(
                authentication("Project v1 source paths resolve to one physical file").into(),
            );
        }
        let source = held.utf8()?;
        total_source_bytes = total_source_bytes
            .checked_add(source.len())
            .ok_or_else(|| capacity("total_source_bytes", MAX_TOTAL_SOURCE_BYTES))?;
        if total_source_bytes > MAX_TOTAL_SOURCE_BYTES {
            return Err(capacity("total_source_bytes", MAX_TOTAL_SOURCE_BYTES).into());
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
                        return Err(capacity("ancestor_directories", MAX_HELD_DIRECTORIES).into());
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
            ))
            .into());
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
            ).into());
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
            return Err(capacity("total_source_bytes", MAX_TOTAL_SOURCE_BYTES).into());
        }
    }
    workspace_sources.extend(dependency_sources);
    if workspace_sources.len() > MAX_SOURCES {
        return Err(capacity("resolved_sources", MAX_SOURCES).into());
    }

    let declared_sources = manifest.sources().to_vec();
    // Form the actual borrowed mapper before core work. Its concrete closure
    // type, owned error and return transports are prepaid by the core owner.
    let hints = |errors| {
        let errors = source_hint::hint_unlisted_module(errors, &root, &declared_sources);
        source_hint::hint_importable_function(errors, &root, &declared_sources)
    };
    let (map_failure, mapper_controls) = prepare_mapper::<E, (Arc<ProjectRevision>, T), _>(hints);
    let callback_controls = E::build_controls::<T, _>(&build);
    let all_controls = match mapper_controls.checked_add(callback_controls) {
        Some(total) => total,
        None => usize::MAX,
    };
    let (revision, result) =
        build(manifest, workspace_sources, all_controls).map_err(map_failure)?;
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

/// The same authenticated loader is used by both outward error projections.
pub(super) trait SnapshotFailure: From<Vec<Diagnostic>> {
    fn map_existing(self, mapper: impl FnOnce(Vec<Diagnostic>) -> Vec<Diagnostic>) -> Self;
    fn mapper_controls<T, M, F>(callback: &F) -> usize;
    fn build_controls<T, F>(callback: &F) -> usize;
}
impl SnapshotFailure for Vec<Diagnostic> {
    fn build_controls<T, F>(_: &F) -> usize {
        0
    }
    // Legacy Vec publication retains its explicitly ordinary caller boundary.
    fn mapper_controls<T, M, F>(_: &F) -> usize {
        0
    }
    fn map_existing(self, mapper: impl FnOnce(Vec<Diagnostic>) -> Vec<Diagnostic>) -> Self {
        mapper(self)
    }
}
impl SnapshotFailure for crate::workspace_report::WorkspaceReportFailure {
    fn build_controls<T, F>(_: &F) -> usize {
        build_callback_bank::<Self, T, F>()
    }
    fn mapper_controls<T, M, F>(_: &F) -> usize {
        mapping_bank::<Self, T, M, F>()
    }
    fn map_existing(self, mapper: impl FnOnce(Vec<Diagnostic>) -> Vec<Diagnostic>) -> Self {
        self.map_legacy(mapper)
    }
}

/// Use the actual closure types, including their real borrowed captures. No
/// fabricated tuple, erased-closure layout or assumed enum padding supplies a fee.
pub(super) fn prepare_mapper<E, T, M>(hints: M) -> (impl FnOnce(E) -> E, usize)
where
    E: SnapshotFailure,
    M: FnOnce(Vec<Diagnostic>) -> Vec<Diagnostic>,
{
    let callback = move |failure: E| failure.map_existing(hints);
    let controls = E::mapper_controls::<T, M, _>(&callback);
    (callback, controls)
}

fn mapping_bank<E, T, M, F>() -> usize {
    use std::mem::size_of;
    const {
        let rows = [
            size_of::<M>(),               // prepare_mapper actual hint argument
            size_of::<M>(),               // prepare_mapper formal hint argument
            size_of::<F>(),               // initialized map_failure closure, including moved M
            size_of::<(F, usize)>(),      // returned prepared pair
            size_of::<(F, usize)>(),      // receiving pair before destructuring
            size_of::<&F>(),              // mapper_controls actual callback loan
            size_of::<&F>(),              // mapper_controls formal callback loan
            size_of::<usize>(),           // mapper_controls returned bank
            size_of::<usize>(),           // prepare_mapper receiving bank local
            size_of::<F>(),               // map_err actual callback
            size_of::<F>(),               // map_err formal callback
            size_of::<F>(),               // FnOnce consuming actual callback receiver
            size_of::<F>(),               // FnOnce consuming formal callback receiver
            size_of::<Result<T, E>>(),    // map_err actual Result receiver
            size_of::<Result<T, E>>(),    // map_err formal Result receiver
            size_of::<Result<T, E>>(),    // map_err returned Result
            size_of::<E>(),               // callback failure parameter
            size_of::<E>(),               // map_existing actual self
            size_of::<E>(),               // map_existing formal self
            size_of::<E>(),               // map_existing returned self
            size_of::<E>(),               // map_legacy actual self
            size_of::<E>(),               // map_legacy formal self
            size_of::<E>(),               // map_legacy returned self
            size_of::<E>(),               // callback returned failure
            size_of::<M>(),               // map_existing actual mapper
            size_of::<M>(),               // map_existing formal mapper
            size_of::<M>(),               // map_legacy actual mapper
            size_of::<M>(),               // map_legacy formal mapper
            size_of::<M>(),               // hint FnOnce actual receiver
            size_of::<M>(),               // hint FnOnce formal receiver
            size_of::<Vec<Diagnostic>>(), // hint actual report argument
            size_of::<Vec<Diagnostic>>(), // hint formal report argument
            size_of::<Vec<Diagnostic>>(), // hint returned report
            size_of::<Vec<Diagnostic>>(), // ExistingWithSite report initializer
            size_of::<usize>(),           // loader received mapper_controls
            size_of::<usize>(),           // build callback actual controls argument
            size_of::<usize>(),           // build callback formal controls argument
        ];
        let mut total = 0usize;
        let mut index = 0;
        while index < rows.len() {
            total = match total.checked_add(rows[index]) {
                Some(total) => total,
                None => {
                    total = usize::MAX;
                    break;
                }
            };
            index += 1;
        }
        total
    }
}

fn build_callback_bank<E, T, F>() -> usize {
    use std::mem::size_of;
    const {
        let rows = [
            size_of::<F>(),     // initialized Project build callback with its ACTUAL captures
            size_of::<F>(),     // loader's by-value formal callback
            size_of::<F>(),     // FnOnce actual callback receiver
            size_of::<F>(),     // FnOnce formal callback receiver
            size_of::<&F>(),    // build_controls actual callback loan
            size_of::<&F>(),    // build_controls formal callback loan
            size_of::<usize>(), // build_controls returned bank
            size_of::<usize>(), // callback_controls receiving local
            size_of::<(ProjectManifest, Vec<SemanticWorkspaceSource>, usize)>(), // actual call args
            size_of::<(ProjectManifest, Vec<SemanticWorkspaceSource>, usize)>(), // formal call args
            size_of::<Result<(Arc<ProjectRevision>, T), E>>(), // callback return
            size_of::<Result<(Arc<ProjectRevision>, T), E>>(), // received callback result
            size_of::<usize>(), // checked_add actual mapper bank receiver
            size_of::<usize>(), // checked_add formal mapper bank receiver
            size_of::<usize>(), // checked_add actual callback bank operand
            size_of::<usize>(), // checked_add formal callback bank operand
            size_of::<Option<usize>>(), // checked_add result
            size_of::<Option<usize>>(), // selected match receiver
            size_of::<usize>(), // matched complete bank
            size_of::<usize>(), // all_controls receiving local
        ];
        let mut total = 0usize;
        let mut index = 0;
        while index < rows.len() {
            total = match total.checked_add(rows[index]) {
                Some(sum) => sum,
                None => {
                    total = usize::MAX;
                    break;
                }
            };
            index += 1;
        }
        total
    }
}
