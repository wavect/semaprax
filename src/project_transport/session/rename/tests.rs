use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-project-rename-response-boundary-{}-{}",
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
        Self(root.canonicalize().unwrap())
    }

    fn plan(&self) -> PreparedProjectRename {
        let snapshot = crate::project::load_snapshot(&self.0.join("semaprax.toml")).unwrap();
        snapshot
            .prepare_rename("calculator.add", "add", "sum")
            .unwrap()
    }

    fn manifest(&self) -> PathBuf {
        self.0.join("semaprax.toml")
    }

    fn source(&self, relative: &str) -> PathBuf {
        self.0.join(relative)
    }

    fn session_with_profile(&self, profile: ServerProfile) -> (Session, String, String) {
        let snapshot = crate::project::load_snapshot(&self.manifest()).unwrap();
        let project = snapshot.project_revision().to_owned();
        let workspace = snapshot.workspace_revision().to_owned();
        (
            Session {
                snapshot: Some(snapshot),
                state: SessionState::Open,
                limits: crate::project_transport::framing::StdioLimits::default(),
                profile,
                manifest_path: self.manifest(),
                pending_rename: None,
                terminal_diagnostics: None,
            },
            project,
            workspace,
        )
    }

    fn session(&self) -> (Session, String, String) {
        self.session_with_profile(ServerProfile::ProjectRenameV1)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn apply_success_and_uncertainty_responses_have_exact_minimum_boundaries() {
    let fixture = Fixture::new();
    let prepared = fixture.plan();
    let id = RequestId::Number(71);

    let success =
        codec::bounded_success_response(&id, &render_rename_receipt(&prepared), usize::MAX);
    let success_required = success.len() + 1;
    assert_eq!(
        codec::bounded_success_response(&id, &render_rename_receipt(&prepared), success_required),
        success
    );
    assert!(codec::is_overflow_response(
        &codec::bounded_success_response(
            &id,
            &render_rename_receipt(&prepared),
            success_required - 1
        )
    ));

    let change_success =
        codec::bounded_success_response(&id, &render_change_receipt(&prepared), usize::MAX);
    let change_success_required = change_success.len() + 1;
    assert_eq!(
        codec::bounded_success_response(
            &id,
            &render_change_receipt(&prepared),
            change_success_required
        ),
        change_success
    );
    assert!(codec::is_overflow_response(
        &codec::bounded_success_response(
            &id,
            &render_change_receipt(&prepared),
            change_success_required - 1
        )
    ));

    let message = "SPX-J110: Project rename publication outcome is uncertain; stop and inspect the bound project";
    let uncertainty =
        codec::bounded_error_response(Some(&id), APPLICATION_ERROR, message, usize::MAX);
    let uncertainty_required = uncertainty.len() + 1;
    assert_eq!(
        codec::bounded_error_response(Some(&id), APPLICATION_ERROR, message, uncertainty_required),
        uncertainty
    );
    assert!(codec::is_overflow_response(&codec::bounded_error_response(
        Some(&id),
        APPLICATION_ERROR,
        message,
        uncertainty_required - 1
    )));
}

fn params(entries: impl IntoIterator<Item = (&'static str, String)>) -> Map<String, Value> {
    entries
        .into_iter()
        .map(|(name, value)| (name.to_owned(), Value::String(value)))
        .collect()
}

fn prepare_session(fixture: &Fixture) -> (Session, String, String, String) {
    let (mut session, project, workspace) = fixture.session();
    let response = session.rename_preview(
        &RequestId::Number(1),
        Some(params([
            ("project_revision", project.clone()),
            ("workspace_revision", workspace.clone()),
            ("target_id", "calculator.add".to_owned()),
            ("from", "add".to_owned()),
            ("to", "sum".to_owned()),
        ])),
    );
    let response: Value = serde_json::from_slice(&response).unwrap();
    let digest = response["result"]["preview"]["preview_digest"]
        .as_str()
        .unwrap()
        .to_owned();
    (session, project, workspace, digest)
}

fn apply_params(project: String, workspace: String, digest: String) -> Map<String, Value> {
    params([
        ("project_revision", project),
        ("workspace_revision", workspace),
        ("preview_digest", digest),
    ])
}

fn prepare_workflow_session(fixture: &Fixture) -> (Session, String, String, String) {
    let (mut session, project, workspace) =
        fixture.session_with_profile(ServerProfile::ProjectWorkflowV1);
    let response = session.rename_derive(
        &RequestId::Number(11),
        Some(params([
            ("project_revision", project.clone()),
            ("workspace_revision", workspace.clone()),
            ("target_id", "calculator.add".to_owned()),
            ("from", "add".to_owned()),
            ("to", "sum".to_owned()),
        ])),
    );
    let response: Value = serde_json::from_slice(&response).unwrap();
    let derivation = response["result"]["derivation"]["artifact_digest"]
        .as_str()
        .unwrap()
        .to_owned();
    let response = session.change_preview(
        &RequestId::Number(12),
        Some(params([
            ("project_revision", project.clone()),
            ("workspace_revision", workspace.clone()),
            ("derivation_digest", derivation),
        ])),
    );
    let response: Value = serde_json::from_slice(&response).unwrap();
    let digest = response["result"]["change"]["artifact_digest"]
        .as_str()
        .unwrap()
        .to_owned();
    (session, project, workspace, digest)
}

fn change_apply_params(project: String, workspace: String, digest: String) -> Map<String, Value> {
    params([
        ("project_revision", project),
        ("workspace_revision", workspace),
        ("change_preview_digest", digest),
    ])
}

struct CommitRejectRuntime;

impl RenameRuntime for CommitRejectRuntime {
    fn commit(
        &mut self,
        _owned: crate::patch::A0OwnedPreparedCommit,
    ) -> Result<String, Vec<Diagnostic>> {
        Err(vec![Diagnostic::io(
            "SPX-I204",
            "injected pre-rename failure",
        )])
    }

    fn reload(&mut self, manifest_path: &Path) -> Result<ProjectSnapshot, Vec<Diagnostic>> {
        crate::project::load_snapshot(manifest_path)
    }
}

struct ReloadRejectRuntime;

impl RenameRuntime for ReloadRejectRuntime {
    fn commit(
        &mut self,
        owned: crate::patch::A0OwnedPreparedCommit,
    ) -> Result<String, Vec<Diagnostic>> {
        crate::patch::commit_owned_a0(owned)
    }

    fn reload(&mut self, _manifest_path: &Path) -> Result<ProjectSnapshot, Vec<Diagnostic>> {
        Err(vec![Diagnostic::io(
            "SPX-J102",
            "injected post-commit reload rejection",
        )])
    }
}

#[test]
fn rejected_commit_reloads_exact_base_and_returns_to_open() {
    let fixture = Fixture::new();
    let before = std::fs::read(fixture.source("src/core.spx")).unwrap();
    let (mut session, project, workspace, digest) = prepare_session(&fixture);
    let response = session.rename_apply_with_runtime(
        &RequestId::Number(2),
        Some(apply_params(project, workspace, digest)),
        &mut CommitRejectRuntime,
    );
    let response: Value = serde_json::from_slice(&response).unwrap();
    assert_eq!(response["error"]["code"], APPLICATION_ERROR);
    assert!(response["error"]["message"]
        .as_str()
        .unwrap()
        .contains("SPX-I204"));
    assert_eq!(session.state, SessionState::Open);
    assert!(session.pending_rename.is_none());
    assert_eq!(
        std::fs::read(fixture.source("src/core.spx")).unwrap(),
        before
    );
}

#[test]
fn post_commit_reload_rejection_is_correlated_terminal_uncertainty() {
    let fixture = Fixture::new();
    let (mut session, project, workspace, digest) = prepare_session(&fixture);
    let response = session.rename_apply_with_runtime(
        &RequestId::Number(2),
        Some(apply_params(project, workspace, digest)),
        &mut ReloadRejectRuntime,
    );
    let response: Value = serde_json::from_slice(&response).unwrap();
    assert_eq!(response["error"]["code"], APPLICATION_ERROR);
    assert!(response["error"]["message"]
        .as_str()
        .unwrap()
        .contains("SPX-J110"));
    assert_eq!(session.state, SessionState::Uncertain);
    assert!(session.snapshot.is_none());
    assert!(session.terminal_diagnostics.as_ref().is_some_and(|items| {
        items.first().is_some_and(|item| item.code == "SPX-J110")
            && items.iter().any(|item| item.code == "SPX-J102")
    }));
    assert!(std::fs::read_to_string(fixture.source("src/core.spx"))
        .unwrap()
        .contains("fn sum("));
}

#[test]
fn workflow_reload_uncertainty_is_terminal_and_blocks_every_later_build() {
    let fixture = Fixture::new();
    let (mut session, project, workspace, digest) = prepare_workflow_session(&fixture);
    let response = session.change_apply_with_runtime(
        &RequestId::Number(13),
        Some(change_apply_params(project, workspace, digest)),
        &mut ReloadRejectRuntime,
    );
    let response: Value = serde_json::from_slice(&response).unwrap();
    assert_eq!(response["error"]["code"], APPLICATION_ERROR);
    assert!(response["error"]["message"]
        .as_str()
        .unwrap()
        .contains("SPX-J110"));
    assert_eq!(session.state, SessionState::Uncertain);
    assert!(session.snapshot.is_none());

    let build = session.build(&RequestId::Number(14), None);
    let build: Value = serde_json::from_slice(&build).unwrap();
    assert_eq!(build["error"]["code"], APPLICATION_ERROR);
    assert!(build["error"]["message"]
        .as_str()
        .unwrap()
        .contains("project session is uncertain"));
    assert!(build.get("result").is_none());
}

#[cfg(unix)]
struct SubstituteRuntime {
    boundary: &'static str,
    path: PathBuf,
}

#[cfg(unix)]
impl SubstituteRuntime {
    fn substitute(&self) {
        let replacement = self.path.with_extension("replacement");
        std::fs::write(&replacement, std::fs::read(&self.path).unwrap()).unwrap();
        std::fs::rename(replacement, &self.path).unwrap();
    }
}

#[cfg(unix)]
impl RenameRuntime for SubstituteRuntime {
    fn before_a0(&mut self) -> Result<(), Vec<Diagnostic>> {
        if self.boundary == "before_a0" {
            self.substitute();
        }
        Ok(())
    }

    fn after_a0(&mut self) -> Result<(), Vec<Diagnostic>> {
        if self.boundary == "after_a0" {
            self.substitute();
        }
        Ok(())
    }

    fn commit(
        &mut self,
        _owned: crate::patch::A0OwnedPreparedCommit,
    ) -> Result<String, Vec<Diagnostic>> {
        panic!("identity drift must stop before commit")
    }

    fn reload(&mut self, _manifest_path: &Path) -> Result<ProjectSnapshot, Vec<Diagnostic>> {
        panic!("identity drift must stop before reload")
    }
}

#[cfg(unix)]
#[test]
fn target_and_foreign_identity_drift_stop_across_the_a0_handoff() {
    for (boundary, relative) in [("before_a0", "src/core.spx"), ("after_a0", "src/tests.spx")] {
        let fixture = Fixture::new();
        let target_before = std::fs::read(fixture.source("src/core.spx")).unwrap();
        let (mut session, project, workspace, digest) = prepare_session(&fixture);
        let mut runtime = SubstituteRuntime {
            boundary,
            path: fixture.source(relative),
        };
        let response = session.rename_apply_with_runtime(
            &RequestId::Number(2),
            Some(apply_params(project, workspace, digest)),
            &mut runtime,
        );
        let response: Value = serde_json::from_slice(&response).unwrap();
        assert_eq!(response["error"]["code"], APPLICATION_ERROR, "{boundary}");
        assert_eq!(session.state, SessionState::Invalidated, "{boundary}");
        assert!(session.snapshot.is_none(), "{boundary}");
        assert!(session.terminal_diagnostics.is_some(), "{boundary}");
        assert_eq!(
            std::fs::read(fixture.source("src/core.spx")).unwrap(),
            target_before,
            "{boundary}"
        );
    }
}
