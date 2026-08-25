//! Authority-neutral complete Project v1 construction from owned source bytes.

use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;
use crate::semantic_workspace::{self, SemanticWorkspaceSource};

use super::{semantic, ProjectManifest, ProjectSource};

pub(super) struct BuiltProject {
    pub(super) sources: Vec<ProjectSource>,
    pub(super) workspace_manifest: String,
    pub(super) workspace_revision: String,
    pub(super) project_revision: String,
    pub(super) entry_program: crate::hir::ResolvedProgram,
    pub(super) test_program: crate::hir::ResolvedProgram,
    pub(super) semantic: semantic::ProjectSemanticState,
}

/// Build and validate the complete manifest-owned Project from already-owned
/// bytes. This helper has no path, handle, read, write, or commit authority.
pub(super) fn build_owned(
    manifest: &ProjectManifest,
    sources: Vec<SemanticWorkspaceSource>,
) -> Result<BuiltProject, Vec<Diagnostic>> {
    let path_set = semantic_workspace::render_path_set(manifest.sources())?;
    let preflight = semantic_workspace::preflight_owned(&path_set, sources)?;
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
    // This is the complete public Web-export admission gate used by ordinary
    // Project loading. Candidate planning must not validate a weaker profile.
    match manifest.project_profile() {
        super::ProjectProfile::ScalarV1 => crate::wasm::emit_resolved_module_with_scalar_exports(
            &entry_program,
            manifest.web_exports(),
        ),
        super::ProjectProfile::UsefulTextConsumerV1 => {
            crate::wasm::emit_resolved_module_with_text_exports(
                &entry_program,
                manifest.web_exports(),
            )
        }
        super::ProjectProfile::UsefulDataV1 => crate::wasm::emit_resolved_module_with_byte_exports(
            &entry_program,
            manifest.web_exports(),
        ),
    }
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
