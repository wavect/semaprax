//! Read-only Project Rename Planning v1.
//!
//! Planning selects one explicit monomorphic Web export from retained typed
//! Project state, derives one canonical Patch-v1 buffer, and validates one
//! complete candidate Project overlay. It owns no path or commit authority.

use std::path::PathBuf;

use sha2::{Digest as _, Sha256};

use crate::diagnostic::{quote_json, Diagnostic};
use crate::semantic_workspace::SemanticWorkspaceSource;

use super::{build, ProjectSnapshot, ProjectSource, MAX_STABLE_ID_BYTES};

pub(crate) const PROJECT_RENAME_PREVIEW_SCHEMA: &str = "semaprax.project-rename-preview.v1";
const MAX_RENAME_NAME_BYTES: usize = 128;
const MAX_PREVIEW_BYTES: usize = 64 * 1024;
const PREVIEW_DIGEST_DOMAIN: &[u8] = b"semaprax.project-rename-preview.payload.v1\0";
const PATCH_DIGEST_DOMAIN: &[u8] = b"semaprax.project-rename-preview.patch.v1\0";

pub(crate) struct PreparedProjectRename {
    target_path: PathBuf,
    patch_bytes: String,
    preview: String,
    preview_digest: String,
    base_workspace_revision: String,
    candidate_workspace_revision: String,
    base_project_revision: String,
    candidate_project_revision: String,
    base_source: ProjectSource,
    candidate_source: ProjectSource,
    candidate_project_graph: String,
    candidate_project_graph_digest: String,
}

/// Opaque capability for the planner's syntax-only Project-module patch pass.
/// Only this module can construct it, before the complete candidate Project is
/// admitted and promoted to [`PreparedProjectRename`].
pub(crate) struct ProjectRenameDerivation {
    source: String,
    patch_bytes: String,
    diagnostic_path: PathBuf,
}

impl ProjectRenameDerivation {
    pub(crate) fn source(&self) -> &str {
        &self.source
    }

    pub(crate) fn patch_bytes(&self) -> &str {
        &self.patch_bytes
    }

    pub(crate) fn diagnostic_path(&self) -> &std::path::Path {
        &self.diagnostic_path
    }
}

impl PreparedProjectRename {
    pub(crate) fn patch_bytes(&self) -> &str {
        &self.patch_bytes
    }

    pub(crate) fn preview(&self) -> &str {
        &self.preview
    }

    pub(crate) fn preview_digest(&self) -> &str {
        &self.preview_digest
    }

    pub(crate) fn base_workspace_revision(&self) -> &str {
        &self.base_workspace_revision
    }

    pub(crate) fn candidate_workspace_revision(&self) -> &str {
        &self.candidate_workspace_revision
    }

    pub(crate) fn base_project_revision(&self) -> &str {
        &self.base_project_revision
    }

    pub(crate) fn candidate_project_revision(&self) -> &str {
        &self.candidate_project_revision
    }

    pub(crate) fn base_source(&self) -> &ProjectSource {
        &self.base_source
    }

    pub(crate) fn candidate_source(&self) -> &ProjectSource {
        &self.candidate_source
    }

    pub(crate) fn candidate_project_graph(&self) -> &str {
        &self.candidate_project_graph
    }

    pub(crate) fn candidate_project_graph_digest(&self) -> &str {
        &self.candidate_project_graph_digest
    }

    /// Acquire the ordinary A0 lock and exact authenticated source handoff for
    /// this completely validated plan. No raw Project-module deferred-profile
    /// constructor is exposed to the transport or other crate callers.
    pub(crate) fn acquire_a0(
        &self,
    ) -> Result<crate::patch::A0OwnedPreparedCommit, Vec<Diagnostic>> {
        crate::patch::acquire_prepared_project_rename(self)
    }

    pub(crate) fn target_path(&self) -> &std::path::Path {
        &self.target_path
    }
}

pub(super) fn prepare(
    snapshot: &ProjectSnapshot,
    target_id: &str,
    from: &str,
    to: &str,
) -> Result<PreparedProjectRename, Vec<Diagnostic>> {
    validate_request_text("target_id", target_id, MAX_STABLE_ID_BYTES)?;
    validate_request_text("from", from, MAX_RENAME_NAME_BYTES)?;
    validate_request_text("to", to, MAX_RENAME_NAME_BYTES)?;
    if from == to {
        return Err(rename_error("Project rename must change the display name"));
    }

    let selected = snapshot
        .semantic
        .rename_function(target_id)
        .ok_or_else(|| rename_error("Project rename target is not a retained Project function"))?;
    if selected.origin != crate::hir::IdentityOrigin::Explicit {
        return Err(rename_error(
            "Project rename target must be an explicitly identified monomorphic function",
        ));
    }
    if !snapshot
        .manifest
        .web_exports()
        .iter()
        .any(|export| export == target_id)
    {
        return Err(rename_error(
            "Project rename target must be selected by manifest web_exports",
        ));
    }
    if selected.name != from {
        return Err(rename_error(
            "Project rename `from` does not match the authenticated function display name",
        ));
    }
    let base_source = snapshot
        .sources
        .iter()
        .find(|source| source.path == selected.path)
        .cloned()
        .ok_or_else(|| rename_error("Project rename source path is absent from the snapshot"))?;

    let patch_bytes = format!(
        "base {}\nrename {target_id} to {to}\n",
        base_source.source_revision
    );
    let target_path = diagnostic_path(snapshot, &selected.path);
    let derivation = ProjectRenameDerivation {
        source: base_source.source.clone(),
        patch_bytes: patch_bytes.clone(),
        diagnostic_path: target_path.clone(),
    };
    let preflight = crate::patch::preflight_project_rename_owned(&derivation)?;
    let candidate_text = preflight.canonical_candidate().to_owned();
    let candidate_total = snapshot
        .sources
        .iter()
        .try_fold(0usize, |total, source| {
            let bytes = if source.path == selected.path {
                candidate_text.len()
            } else {
                source.source.len()
            };
            total.checked_add(bytes)
        })
        .ok_or_else(|| rename_error("Project rename candidate source size overflow"))?;
    if candidate_total > super::MAX_TOTAL_SOURCE_BYTES {
        return Err(rename_error(
            "Project rename candidate exceeds the complete Project source bound",
        ));
    }
    let overlay = snapshot
        .sources
        .iter()
        .map(|source| SemanticWorkspaceSource {
            path: source.path.clone(),
            source: if source.path == selected.path {
                candidate_text.clone()
            } else {
                source.source.clone()
            },
        })
        .collect();
    // Exactly one complete candidate Phase-A/closure/graph/Web-admission build.
    let candidate = build::build_owned(&snapshot.manifest, overlay)?;
    let candidate_source = candidate
        .sources
        .iter()
        .find(|source| source.path == selected.path)
        .cloned()
        .ok_or_else(|| rename_error("Project rename candidate source fact is absent"))?;
    if candidate_source.source != candidate_text
        || candidate_source.source_revision != preflight.candidate_revision()
    {
        return Err(rename_error(
            "Project rename pure patch and complete candidate build disagree",
        ));
    }
    let patch_digest = domain_digest(PATCH_DIGEST_DOMAIN, patch_bytes.as_bytes());
    let candidate_graph_digest = candidate.semantic.graph_digest().to_owned();
    let payload = format!(
        "{{\"schema\":{},\"project_schema\":{},\"base_project_revision\":{},\"candidate_project_revision\":{},\"base_workspace_revision\":{},\"candidate_workspace_revision\":{},\"target\":{{\"stable_id\":{},\"from\":{},\"to\":{},\"path\":{}}},\"patch\":{{\"schema\":\"semaprax.semantic-patch.v1\",\"digest\":{},\"bytes\":{}}},\"base_source\":{},\"candidate_source\":{},\"candidate_project_graph\":{{\"schema\":{},\"digest\":{}}},\"limits\":{{\"max_preview_bytes\":{},\"max_target_id_bytes\":{},\"max_name_bytes\":{}}},\"nonclaims\":[\"read_only_plan_no_commit_authority\",\"no_request_selected_path_or_source_bytes\",\"no_multi_file_or_import_alias_rename\",\"no_build_target_or_test_execution\",\"no_provenance_approval_or_exactly_once_effect\"]}}",
        quote_json(PROJECT_RENAME_PREVIEW_SCHEMA),
        quote_json(super::PROJECT_SCHEMA),
        quote_json(snapshot.project_revision()),
        quote_json(&candidate.project_revision),
        quote_json(snapshot.workspace_revision()),
        quote_json(&candidate.workspace_revision),
        quote_json(target_id),
        quote_json(from),
        quote_json(to),
        quote_json(&selected.path),
        quote_json(&patch_digest),
        patch_bytes.len(),
        source_fact_json(&base_source),
        source_fact_json(&candidate_source),
        quote_json(super::PROJECT_SEMANTIC_GRAPH_SCHEMA),
        quote_json(&candidate_graph_digest),
        MAX_PREVIEW_BYTES,
        MAX_STABLE_ID_BYTES,
        MAX_RENAME_NAME_BYTES,
    );
    let preview_digest = domain_digest(PREVIEW_DIGEST_DOMAIN, payload.as_bytes());
    let preview = format!(
        "{},\"preview_digest\":{}}}",
        payload
            .strip_suffix('}')
            .expect("Project rename payload is an object"),
        quote_json(&preview_digest),
    );
    if preview.len() > MAX_PREVIEW_BYTES {
        return Err(rename_error(
            "Project rename preview exceeds its exact byte bound",
        ));
    }
    Ok(PreparedProjectRename {
        target_path,
        patch_bytes,
        preview,
        preview_digest,
        base_workspace_revision: snapshot.workspace_revision().to_owned(),
        candidate_workspace_revision: candidate.workspace_revision,
        base_project_revision: snapshot.project_revision().to_owned(),
        candidate_project_revision: candidate.project_revision,
        base_source,
        candidate_source,
        candidate_project_graph: candidate.semantic.graph().to_owned(),
        candidate_project_graph_digest: candidate_graph_digest,
    })
}

fn validate_request_text(name: &str, value: &str, max_bytes: usize) -> Result<(), Vec<Diagnostic>> {
    if value.is_empty() || value.len() > max_bytes || value.chars().any(char::is_control) {
        return Err(rename_error(format!(
            "Project rename {name} must be nonempty, at most {max_bytes} bytes, and contain no control characters"
        )));
    }
    Ok(())
}

fn source_fact_json(source: &ProjectSource) -> String {
    format!(
        "{{\"path\":{},\"source_graph_schema\":{},\"source_revision\":{},\"source_digest\":{},\"bytes\":{}}}",
        quote_json(&source.path),
        quote_json(&source.source_graph_schema),
        quote_json(&source.source_revision),
        quote_json(&source.source_digest),
        source.source.len(),
    )
}

fn diagnostic_path(snapshot: &ProjectSnapshot, relative: &str) -> PathBuf {
    snapshot.root.join(relative)
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(digest.finalize())
    )
}

fn rename_error(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-J109", message)]
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static SERIAL: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn fixture() -> Fixture {
        let root = std::env::temp_dir().join(format!(
            "semaprax-project-rename-unit-{}-{}",
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
        Fixture(root.canonicalize().unwrap())
    }

    fn inventory(root: &Path) -> BTreeMap<String, Vec<u8>> {
        fn visit(root: &Path, directory: &Path, result: &mut BTreeMap<String, Vec<u8>>) {
            for entry in std::fs::read_dir(directory).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                let relative = path.strip_prefix(root).unwrap().to_string_lossy();
                if entry.file_type().unwrap().is_dir() {
                    result.insert(format!("directory:{relative}"), Vec::new());
                    visit(root, &path, result);
                } else {
                    result.insert(format!("file:{relative}"), std::fs::read(path).unwrap());
                }
            }
        }
        let mut result = BTreeMap::new();
        visit(root, root, &mut result);
        result
    }

    fn assert_rename_error(result: Result<PreparedProjectRename, Vec<Diagnostic>>, text: &str) {
        let diagnostics = result.err().expect("rename unexpectedly succeeded");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "SPX-J109" && diagnostic.message.contains(text)
        }));
    }

    #[test]
    fn stable_export_plan_is_deterministic_digest_bound_and_read_only() {
        let fixture = fixture();
        let before = inventory(&fixture.0);
        let snapshot = super::super::load_snapshot(&fixture.0.join("semaprax.toml")).unwrap();
        let first = snapshot
            .prepare_rename("calculator.add", "add", "sum")
            .unwrap();
        let second = snapshot
            .prepare_rename("calculator.add", "add", "sum")
            .unwrap();

        assert_eq!(first.candidate_source().path(), "src/core.spx");
        assert_eq!(first.preview(), second.preview());
        assert_eq!(first.preview_digest(), second.preview_digest());
        assert_eq!(first.patch_bytes(), second.patch_bytes());
        assert!(first
            .patch_bytes()
            .starts_with(&format!("base {}\n", first.base_source().source_revision())));
        assert!(first
            .patch_bytes()
            .ends_with("rename calculator.add to sum\n"));
        assert!(first
            .candidate_source()
            .source()
            .contains("@id(\"calculator.add\")\nfn sum("));
        assert_ne!(
            first.base_workspace_revision(),
            first.candidate_workspace_revision()
        );
        assert_ne!(
            first.base_project_revision(),
            first.candidate_project_revision()
        );
        let graph: serde_json::Value =
            serde_json::from_str(first.candidate_project_graph()).unwrap();
        assert_eq!(
            graph["graph_digest"],
            first.candidate_project_graph_digest()
        );
        assert!(first.candidate_project_graph().contains("calculator.add"));

        let marker = format!(",\"preview_digest\":\"{}\"}}", first.preview_digest());
        let payload = first.preview().strip_suffix(&marker).unwrap().to_owned() + "}";
        assert_eq!(
            domain_digest(PREVIEW_DIGEST_DOMAIN, payload.as_bytes()),
            first.preview_digest()
        );
        assert_eq!(before, inventory(&fixture.0));
    }

    #[test]
    fn wrong_from_non_export_collision_and_invalid_complete_candidate_fail_closed() {
        let fixture = fixture();
        let before = inventory(&fixture.0);
        let snapshot = super::super::load_snapshot(&fixture.0.join("semaprax.toml")).unwrap();

        assert_rename_error(
            snapshot.prepare_rename("calculator.add", "wrong", "sum"),
            "does not match",
        );
        assert_rename_error(
            snapshot.prepare_rename("calculator.tests.main", "main", "renamed"),
            "web_exports",
        );
        assert!(snapshot
            .prepare_rename("calculator.add", "add", "divide")
            .is_err());
        assert!(snapshot
            .prepare_rename("calculator.add", "add", "main")
            .is_err());
        assert_eq!(before, inventory(&fixture.0));
    }

    #[test]
    fn automatic_function_identity_is_rejected_before_planning() {
        let fixture = fixture();
        let core = fixture.0.join("src/core.spx");
        let mut source = std::fs::read_to_string(&core).unwrap();
        source.push_str("\nfn helper(value: i64) -> i64\n{\n    value\n}\n");
        std::fs::write(&core, source).unwrap();
        let diagnostics = super::super::load_snapshot(&fixture.0.join("semaprax.toml"))
            .err()
            .expect("automatic function entered a plannable Project snapshot");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "SPX-W115" && diagnostic.message.contains("explicit stable identity")
        }));
    }

    #[test]
    fn sealed_plan_acquires_a0_for_validated_imported_module_without_main() {
        let fixture = fixture();
        let manifest_path = fixture.0.join("semaprax.toml");
        let manifest = std::fs::read_to_string(&manifest_path).unwrap().replace(
            "\"src/core.spx\", \"src/tests.spx\"",
            "\"src/core.spx\", \"src/helpers.spx\", \"src/tests.spx\"",
        );
        std::fs::write(&manifest_path, manifest).unwrap();
        std::fs::write(
            fixture.0.join("src/helpers.spx"),
            "module calculator.helpers;\n\n@id(\"calculator.identity\")\nfn identity(value: i64) -> i64\n{\n    value\n}\n",
        )
        .unwrap();
        let core_path = fixture.0.join("src/core.spx");
        let core = std::fs::read_to_string(&core_path)
            .unwrap()
            .replace(
                "module calculator.core;\n",
                "module calculator.core;\nuse function @id(\"calculator.identity\") from calculator.helpers as identity;\n",
            )
            .replacen("left + right", "identity(left) + right", 1);
        std::fs::write(&core_path, core).unwrap();
        let before = inventory(&fixture.0);

        let snapshot = super::super::load_snapshot(&manifest_path).unwrap();
        let prepared = snapshot
            .prepare_rename("calculator.add", "add", "sum")
            .unwrap();
        let authority = prepared.acquire_a0().unwrap();
        drop(authority);

        assert_eq!(before, inventory(&fixture.0));
    }
}
