//! Authority-neutral complete Project v1 construction from owned source bytes.

use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;
use crate::semantic_workspace::{self, SemanticWorkspaceSource};

use super::{admission, semantic, ProjectManifest, ProjectSource, PublicApiSubject};

pub(super) struct BuiltProject {
    pub(super) sources: Vec<ProjectSource>,
    pub(super) workspace_manifest: String,
    pub(super) workspace_revision: String,
    pub(super) project_revision: String,
    pub(super) entry_program: crate::hir::ResolvedProgram,
    pub(super) test_program: crate::hir::ResolvedProgram,
    pub(super) semantic: semantic::ProjectSemanticState,
    pub(super) profile_admission: admission::PreparedProjectAdmission,
}

/// Build and validate the complete manifest-owned Project from already-owned
/// bytes. This helper has no path, handle, read, write, or commit authority.
pub(super) fn build_owned(
    manifest: &ProjectManifest,
    mut sources: Vec<SemanticWorkspaceSource>,
) -> Result<BuiltProject, Vec<Diagnostic>> {
    super::standard_dependencies::extend_sources(manifest, &mut sources)?;
    sources.sort_by(|left, right| left.path.cmp(&right.path));
    let paths = sources
        .iter()
        .map(|source| source.path.clone())
        .collect::<Vec<_>>();
    let path_set = semantic_workspace::render_path_set(&paths)?;
    let preflight = semantic_workspace::preflight_owned(&path_set, sources)?;
    finish_build(manifest, preflight)
}

pub(super) fn build_owned_with_frontend(
    manifest: &ProjectManifest,
    mut sources: Vec<SemanticWorkspaceSource>,
    frontend: &mut super::incremental::FrontendPass,
) -> Result<BuiltProject, Vec<Diagnostic>> {
    super::standard_dependencies::extend_sources(manifest, &mut sources)?;
    sources.sort_by(|left, right| left.path.cmp(&right.path));
    let paths = sources
        .iter()
        .map(|source| source.path.clone())
        .collect::<Vec<_>>();
    let path_set = semantic_workspace::render_path_set(&paths)?;
    let preflight =
        semantic_workspace::preflight_owned_with_frontend(&path_set, sources, frontend)?;
    finish_build(manifest, preflight)
}

fn finish_build(
    manifest: &ProjectManifest,
    preflight: semantic_workspace::SemanticWorkspacePreflight,
) -> Result<BuiltProject, Vec<Diagnostic>> {
    let (files, workspace_manifest, workspace_revision, graph) = preflight.into_snapshot_parts();
    let canonical_manifest = manifest.to_canonical_toml();
    let project_revision = project_revision(&canonical_manifest, &workspace_revision);
    let graph_source_facts = files
        .iter()
        .map(|file| crate::workspace_graph::ProjectGraphSourceFact {
            path: file.path().to_owned(),
            source_graph_schema: file.source_graph_schema().to_owned(),
            source_revision: file.source_revision().to_owned(),
            source_digest: file.source_digest().to_owned(),
        })
        .collect();
    let semantic_parts = graph.into_project_semantic_parts(
        &workspace_revision,
        graph_source_facts,
        canonical_manifest.len(),
        manifest.entry(),
        manifest.test_module(),
        crate::workspace_graph::ProjectWebRoots {
            stable_ids: manifest.web_exports(),
            profile: manifest.project_profile(),
            dependency_anchors: !manifest.dependency_sources().is_empty(),
        },
    )?;
    let entry_program = semantic_parts.entry_program;
    let test_program = semantic_parts.test_program;
    let semantic = semantic::ProjectSemanticState::new(
        semantic_parts.projection,
        manifest.schema(),
        manifest.name(),
        &project_revision,
        manifest.test_module(),
    )?;
    // This is the complete public target admission gate used by ordinary
    // Project loading. Candidate planning must not validate a weaker profile,
    // and every additive schema must pass this one exhaustive dispatcher.
    let profile_admission = admission::prepare(
        manifest,
        &entry_program,
        PublicApiSubject {
            project_schema: manifest.schema(),
            project_revision: &project_revision,
            workspace_revision: &workspace_revision,
            project_graph_digest: semantic.graph_digest(),
        },
    )
    .map_err(|error| vec![error])?;
    let sources = files
        .into_iter()
        .map(|file| {
            let (path, source_graph_schema, source_revision, source_digest, source) =
                file.into_parts();
            ProjectSource {
                path,
                source_graph_schema,
                source_revision,
                source_digest,
                source,
            }
        })
        .collect();
    Ok(BuiltProject {
        sources,
        workspace_manifest,
        workspace_revision,
        project_revision,
        entry_program,
        test_program,
        semantic,
        profile_admission,
    })
}

fn project_revision(manifest: &str, workspace_revision: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(b"semaprax.project-revision.v1\0");
    digest.update((manifest.len() as u64).to_le_bytes());
    digest.update(manifest.as_bytes());
    digest.update((workspace_revision.len() as u64).to_le_bytes());
    digest.update(workspace_revision.as_bytes());
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(digest.finalize())
    )
}
