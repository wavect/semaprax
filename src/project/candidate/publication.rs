//! Explicit host bridge to managed Workspace publication. Ordinary candidates,
//! images, recovery capsules and transport remain without filesystem authority.
use std::cell::{Cell, RefCell};
use std::path::Path;

use serde_json::json;

use crate::diagnostic::Diagnostic;
use crate::project::{load_snapshot, ProjectSnapshot};
use crate::semantic_workspace_change::{
    self as change, SemanticWorkspaceChangeArtifacts, SemanticWorkspaceChangeFile,
    SemanticWorkspaceChangeSet, SemanticWorkspacePreparedChange,
};
use crate::workspace::{SemanticChangeApplyPoint, WorkspaceSemanticSource};

use super::{wire, ProjectCandidate};

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;
pub const PROJECT_CANDIDATE_PUBLICATION_SCHEMA: &str = "semaprax.project-candidate-publication.v1";
pub const MAX_PROJECT_CANDIDATE_PUBLICATION_BYTES: usize = 128 * 1024 * 1024;
const DOMAIN: &[u8] = b"semaprax.project-candidate-publication.artifact.v1\0";

/// Read-only, host-bound proposal/evidence bytes. This value is neither an
/// approval token nor a Workspace authority and cannot itself publish anything.
pub struct ProjectCandidatePublication {
    json: String,
    digest: String,
    proposal: String,
    evidence: String,
    candidate_workspace_revision: String,
}
impl ProjectCandidatePublication {
    pub fn to_json(&self) -> &str {
        &self.json
    }
    pub fn publication_digest(&self) -> &str {
        &self.digest
    }
    pub fn proposal(&self) -> &str {
        &self.proposal
    }
    pub fn workspace_change_evidence(&self) -> &str {
        &self.evidence
    }
    pub fn candidate_workspace_revision(&self) -> &str {
        &self.candidate_workspace_revision
    }
}

/// Independently replay a complete candidate under the existing shared managed
/// Workspace lock. No proposal file, cache, generation or staging object is made.
pub fn prepare_candidate_publication(
    candidate: &ProjectCandidate,
    approved_candidate_digest: &str,
    workspace_root: &Path,
    project_manifest: &Path,
    expected_workspace_revision: &str,
) -> Result<ProjectCandidatePublication> {
    let snapshot = RefCell::new(load_snapshot(project_manifest)?);
    validate_host(
        &snapshot.borrow(),
        candidate,
        workspace_root,
        project_manifest,
    )?;
    let result = change::with_project_candidate_change(
        workspace_root,
        |actual, sources| {
            derive(
                candidate,
                approved_candidate_digest,
                expected_workspace_revision,
                actual,
                sources,
                &mut snapshot.borrow_mut(),
            )
        },
        |prepared, artifacts| {
            snapshot.borrow_mut().recheck()?;
            render(
                candidate,
                workspace_root,
                project_manifest,
                prepared,
                artifacts,
            )
        },
    );
    let final_check = snapshot.borrow_mut().recheck();
    finish_read(result, final_check)
}

/// Separately authorized host invocation. Both digests are caller expectations,
/// not values read from a submitted proof. Only the existing Workspace authority
/// may publish one complete managed generation by replacing ACTIVE.
pub fn apply_candidate_publication(
    candidate: &ProjectCandidate,
    approved_candidate_digest: &str,
    workspace_root: &Path,
    project_manifest: &Path,
    expected_workspace_revision: &str,
    submitted_publication: &[u8],
) -> Result<String> {
    apply_with_hook(
        candidate,
        approved_candidate_digest,
        workspace_root,
        project_manifest,
        expected_workspace_revision,
        submitted_publication,
        |_| Ok(()),
    )
}

fn apply_with_hook(
    candidate: &ProjectCandidate,
    approved_candidate_digest: &str,
    workspace_root: &Path,
    project_manifest: &Path,
    expected_workspace_revision: &str,
    submitted_publication: &[u8],
    mut hook: impl FnMut(SemanticChangeApplyPoint) -> std::io::Result<()>,
) -> Result<String> {
    if submitted_publication.len() > MAX_PROJECT_CANDIDATE_PUBLICATION_BYTES {
        return Err(capacity(
            "candidate publication proof exceeds its byte bound",
        ));
    }
    let snapshot = RefCell::new(load_snapshot(project_manifest)?);
    validate_host(
        &snapshot.borrow(),
        candidate,
        workspace_root,
        project_manifest,
    )?;
    let published = Cell::new(false);
    let drift = RefCell::new(Vec::new());
    let result = change::apply_project_candidate_change(
        workspace_root,
        |actual, sources| {
            derive(
                candidate,
                approved_candidate_digest,
                expected_workspace_revision,
                actual,
                sources,
                &mut snapshot.borrow_mut(),
            )
        },
        |prepared, artifacts| {
            let expected = render(
                candidate,
                workspace_root,
                project_manifest,
                prepared,
                artifacts,
            )?;
            // No submitted JSON is deserialized as state, HIR or approval. Exact
            // bytes reject substitutions, reminted digests and noncanonical text.
            if expected.to_json().as_bytes() != submitted_publication {
                return Err(stale(
                    "candidate publication proof failed exact independent replay",
                ));
            }
            snapshot.borrow_mut().recheck()?;
            let receipt = render_receipt(candidate, workspace_root, prepared, &expected)?;
            Ok((expected.evidence, receipt))
        },
        |point, _, _, _| {
            if point == SemanticChangeApplyPoint::AfterActiveReplace {
                published.set(true);
            }
            hook(point)?;
            if matches!(
                point,
                SemanticChangeApplyPoint::AfterCandidatePrepared
                    | SemanticChangeApplyPoint::BeforeFirstFinalCheck
                    | SemanticChangeApplyPoint::BeforeSecondFinalCheck
                    | SemanticChangeApplyPoint::BeforeActiveReplace
                    | SemanticChangeApplyPoint::AfterActiveReplace
            ) {
                if let Err(diagnostics) = snapshot.borrow_mut().recheck() {
                    *drift.borrow_mut() = diagnostics;
                    return Err(std::io::Error::other(
                        "held Project inputs drifted at managed publication boundary",
                    ));
                }
            }
            Ok(())
        },
    );
    let final_check = snapshot.borrow_mut().recheck();
    let mut diagnostics = match result {
        Ok(receipt) if final_check.is_ok() && drift.borrow().is_empty() => return Ok(receipt),
        Ok(_) => Vec::new(),
        Err(diagnostics) => diagnostics,
    };
    diagnostics.extend(drift.into_inner());
    if let Err(final_drift) = final_check {
        diagnostics.extend(final_drift);
    }
    if published.get() {
        diagnostics.insert(0, Diagnostic::io("SPX-G248",
            "managed ACTIVE publication occurred; a later check failed, so do not assume unchanged state or retry blindly"));
    }
    Err(diagnostics)
}

fn validate_host(
    snapshot: &ProjectSnapshot,
    candidate: &ProjectCandidate,
    root: &Path,
    manifest: &Path,
) -> Result<()> {
    if !root.is_absolute() || root != snapshot.root() || manifest != root.join("semaprax.toml") {
        return Err(invalid("publication requires the exact absolute authenticated Project root and its manifest path"));
    }
    if root.to_str().is_none() || manifest.to_str().is_none() {
        return Err(invalid("publication host paths must be UTF-8"));
    }
    if snapshot.project_revision() != candidate.base.project_revision()
        || snapshot.manifest().to_canonical_toml() != candidate.base.manifest().to_canonical_toml()
        || candidate.revision.manifest().to_canonical_toml()
            != candidate.base.manifest().to_canonical_toml()
    {
        return Err(stale(
            "held source Project/manifest does not match the candidate's original base",
        ));
    }
    Ok(())
}

fn derive(
    candidate: &ProjectCandidate,
    approved: &str,
    expected_workspace: &str,
    actual_workspace: &str,
    sources: &[WorkspaceSemanticSource],
    snapshot: &mut ProjectSnapshot,
) -> Result<SemanticWorkspaceChangeSet> {
    wire::validate_digest(approved)?;
    wire::validate_digest(expected_workspace)?;
    if approved != candidate.candidate_digest() {
        return Err(stale(
            "host approval does not name this exact candidate digest",
        ));
    }
    if expected_workspace != actual_workspace
        || expected_workspace != candidate.base.workspace_revision()
    {
        return Err(stale(
            "managed ACTIVE does not match the exact expected candidate base workspace",
        ));
    }
    snapshot.recheck()?;
    if snapshot.project_revision() != candidate.base.project_revision() {
        return Err(stale(
            "held Project changed before candidate publication replay",
        ));
    }
    let base = candidate.base.sources();
    if sources.len() != base.len() || snapshot.sources().len() != base.len() {
        return Err(stale("managed and Project source inventories differ"));
    }
    for ((managed, base), held) in sources.iter().zip(base).zip(snapshot.sources()) {
        if managed.path != base.path()
            || held.path() != base.path()
            || managed.source != base.source()
            || held.source() != base.source()
            || managed.source_graph_schema != base.source_graph_schema()
            || managed.source_revision != base.source_revision()
            || managed.source_digest != base.source_digest()
        {
            return Err(stale(
                "managed source bytes/facts do not match the authenticated Project base",
            ));
        }
    }
    // This expensive semantic replay is inside the shared/exclusive permanent
    // Workspace lock. Candidate data alone never supplies publication authority.
    let replayed = ProjectCandidate::replay(
        snapshot.retain_revision(),
        snapshot.project_revision(),
        &candidate.changes,
        candidate.to_json().as_bytes(),
    )?;
    if replayed.candidate_digest() != approved
        || replayed.revision.project_revision() != candidate.revision.project_revision()
    {
        return Err(stale("candidate history replay differs from host approval"));
    }
    let after = replayed.revision.sources();
    if after.len() != base.len() {
        return Err(invalid(
            "managed publication cannot change the source path inventory",
        ));
    }
    let mut replacements = Vec::new();
    for (before, after) in base.iter().zip(after) {
        if before.path() != after.path() {
            return Err(invalid(
                "managed publication cannot create, delete or move source paths",
            ));
        }
        if before.source() != after.source() {
            replacements.push(SemanticWorkspaceChangeFile::new(
                before.path().to_owned(),
                before.source_graph_schema().to_owned(),
                before.source_revision().to_owned(),
                before.source_digest().to_owned(),
                after.source().to_owned(),
            )?);
        }
    }
    if replacements.len() < 2 {
        return Err(invalid("existing Change-v1 publication requires two to sixteen genuinely changed files; unchanged padding is forbidden"));
    }
    snapshot.recheck()?;
    SemanticWorkspaceChangeSet::new(
        expected_workspace.to_owned(),
        candidate.base.manifest().entry().to_owned(),
        replacements,
    )
}

fn render(
    candidate: &ProjectCandidate,
    root: &Path,
    manifest: &Path,
    prepared: &SemanticWorkspacePreparedChange,
    artifacts: &SemanticWorkspaceChangeArtifacts,
) -> Result<ProjectCandidatePublication> {
    if prepared.candidate_workspace_revision() != candidate.revision.workspace_revision() {
        return Err(stale(
            "managed candidate generation differs from independently replayed Project sources",
        ));
    }
    let value = json!({
        "schema":PROJECT_CANDIDATE_PUBLICATION_SCHEMA,
        "compiler":{"package":env!("CARGO_PKG_NAME"),"version":env!("CARGO_PKG_VERSION")},
        "workspace_root":root.to_str().ok_or_else(||invalid("workspace root must be UTF-8"))?,
        "project_manifest_path":manifest.to_str().ok_or_else(||invalid("manifest path must be UTF-8"))?,
        "base_project_revision":candidate.base.project_revision(),
        "candidate_project_revision":candidate.revision.project_revision(),
        "base_workspace_revision":prepared.base_workspace_revision(),
        "candidate_workspace_revision":prepared.candidate_workspace_revision(),
        "canonical_project_manifest":candidate.base.manifest().to_canonical_toml(),
        "approved_candidate_digest":candidate.candidate_digest(),
        "candidate_evidence":candidate.to_json(),
        "workspace_change_proposal":prepared.proposal_source(),
        "workspace_change_evidence":artifacts.evidence(),
        "nonclaims":["not_an_approval_or_signature","no_raw_source_or_git_write","managed_active_readers_only",
            "no_new_publication_authority","no_target_execution","no_rollback_or_cleanup_authority"],
        "max_publication_bytes":MAX_PROJECT_CANDIDATE_PUBLICATION_BYTES,
    });
    let json = wire::render(value, MAX_PROJECT_CANDIDATE_PUBLICATION_BYTES)
        .map_err(|_| capacity("candidate publication proof exceeds its output byte bound"))?;
    Ok(ProjectCandidatePublication {
        digest: wire::digest(DOMAIN, json.as_bytes()),
        json,
        proposal: prepared.proposal_source().to_owned(),
        evidence: artifacts.evidence().to_owned(),
        candidate_workspace_revision: prepared.candidate_workspace_revision().to_owned(),
    })
}
fn render_receipt(
    candidate: &ProjectCandidate,
    root: &Path,
    prepared: &SemanticWorkspacePreparedChange,
    publication: &ProjectCandidatePublication,
) -> Result<String> {
    wire::render(json!({"schema":"semaprax.project-candidate-publication-application.v1",
        "result":"managed_generation_published","workspace_root":root.to_str(),
        "base_workspace_revision":prepared.base_workspace_revision(),
        "candidate_workspace_revision":prepared.candidate_workspace_revision(),
        "candidate_digest":candidate.candidate_digest(),"publication_digest":publication.publication_digest(),
        "publication":"existing_workspace_active_pivot","raw_source_files":"unchanged",
        "git_commit":"not_performed"}),64*1024)
        .map_err(|_|capacity("candidate publication receipt exceeds its byte bound"))
}
fn finish_read<T>(result: Result<T>, check: Result<()>) -> Result<T> {
    match (result, check) {
        (result, Ok(())) => result,
        (Ok(_), Err(diagnostics)) => Err(diagnostics),
        (Err(mut diagnostics), Err(drift)) => {
            diagnostics.extend(drift);
            Err(diagnostics)
        }
    }
}
fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G245", message)]
}
fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G246", message)]
}
fn stale(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G247", message)]
}

#[cfg(test)]
mod publication_boundary_tests {
    use super::*;
    use crate::project::SemanticChange;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;
    static SERIAL: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn source_drift_after_active_pivot_reports_publication_uncertainty() {
        let root = std::env::temp_dir().join(format!(
            "spx-publication-boundary-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(example.join(path), root.join(path)).unwrap();
        }
        let root = root.canonicalize().unwrap();
        let manifest = root.join("semaprax.toml");
        let paths = root.join("paths.json");
        std::fs::write(&paths, "{\"schema\":\"semaprax.workspace-semantic-path-set.v1\",\"files\":[{\"path\":\"src/app.spx\"},{\"path\":\"src/core.spx\"},{\"path\":\"src/tests.spx\"}]}\n").unwrap();
        let workspace = crate::semantic_workspace::initialize(&root, &paths).unwrap();
        let snapshot = load_snapshot(&manifest).unwrap();
        let base = snapshot.retain_revision();
        let candidate = ProjectCandidate::open(Arc::clone(&base), base.project_revision()).unwrap();
        let change = SemanticChange::new(base.project_revision(), &json!({"kind":"change_function_signature",
            "target":"calculator.add","append_parameters":[{"name":"unused","type":"i64","argument":{"kind":"i64","value":0}}]})).unwrap();
        let candidate = candidate
            .apply(candidate.candidate_digest(), &change)
            .unwrap();
        let proof = prepare_candidate_publication(
            &candidate,
            candidate.candidate_digest(),
            &root,
            &manifest,
            &workspace,
        )
        .unwrap();
        let raw = root.join("src/core.spx");
        let original = std::fs::read_to_string(&raw).unwrap();
        let result = apply_with_hook(
            &candidate,
            candidate.candidate_digest(),
            &root,
            &manifest,
            &workspace,
            proof.to_json().as_bytes(),
            |point| {
                if point == SemanticChangeApplyPoint::AfterActiveReplace {
                    std::fs::write(&raw, format!("{original}\n"))?;
                }
                Ok(())
            },
        );
        let errors = result.expect_err("postpublication drift must fail explicitly");
        assert!(errors.iter().any(|error| error.code == "SPX-G248"));
        assert_eq!(
            crate::workspace::snapshot(&root)
                .unwrap()
                .workspace_revision(),
            proof.candidate_workspace_revision()
        );
        drop(snapshot);
        std::fs::remove_dir_all(root).unwrap();
    }
}
