//! Opt-in Project Rename Transaction v1 session state machine.

use std::path::Path;

use serde_json::{Map, Value};

use super::{
    codec, invalidates, reject_unknown, take_exact_revisions, take_string, RequestId,
    ServerProfile, Session, SessionState, APPLICATION_ERROR, METHOD_NOT_FOUND,
};
use crate::diagnostic::{quote_json, Diagnostic};
use crate::project::{PreparedProjectRename, ProjectSnapshot};

trait RenameRuntime {
    fn before_a0(&mut self) -> Result<(), Vec<Diagnostic>> {
        Ok(())
    }

    fn after_a0(&mut self) -> Result<(), Vec<Diagnostic>> {
        Ok(())
    }

    fn commit(
        &mut self,
        owned: crate::patch::A0OwnedPreparedCommit,
    ) -> Result<String, Vec<Diagnostic>>;

    fn reload(&mut self, manifest_path: &Path) -> Result<ProjectSnapshot, Vec<Diagnostic>>;
}

struct ProductionRuntime;

impl RenameRuntime for ProductionRuntime {
    fn commit(
        &mut self,
        owned: crate::patch::A0OwnedPreparedCommit,
    ) -> Result<String, Vec<Diagnostic>> {
        crate::patch::commit_owned_a0(owned)
    }

    fn reload(&mut self, manifest_path: &Path) -> Result<ProjectSnapshot, Vec<Diagnostic>> {
        crate::project::load_snapshot(manifest_path)
    }
}

impl Session {
    pub(super) fn rename_preview(
        &mut self,
        id: &RequestId,
        params: Option<Map<String, Value>>,
    ) -> Vec<u8> {
        if self.profile != ServerProfile::ProjectRenameV1 {
            return self.error(id, METHOD_NOT_FOUND, "method not found: rename/preview");
        }
        if self.state != SessionState::Open {
            return self.lifecycle_error(id);
        }
        let mut params = params.unwrap_or_default();
        let snapshot = self
            .snapshot
            .as_ref()
            .expect("an open rename session retains its authenticated snapshot");
        if let Err(message) = take_exact_revisions(snapshot, &mut params) {
            return self.error(id, codec::INVALID_PARAMS, &message);
        }
        let target_id = match take_string(&mut params, "target_id") {
            Ok(value) => value,
            Err(error) => return self.finish(id, Err(error)),
        };
        let from = match take_string(&mut params, "from") {
            Ok(value) => value,
            Err(error) => return self.finish(id, Err(error)),
        };
        let to = match take_string(&mut params, "to") {
            Ok(value) => value,
            Err(error) => return self.finish(id, Err(error)),
        };
        if let Err(error) = reject_unknown(&params) {
            return self.finish(id, Err(error));
        }
        let result = self
            .snapshot
            .as_mut()
            .expect("an open rename session retains its authenticated snapshot")
            .with_authenticated_request(|snapshot| snapshot.prepare_rename(&target_id, &from, &to));
        match result {
            Ok(prepared) => {
                let response = codec::bounded_success_response(
                    id,
                    &format!("{{\"preview\":{}}}", prepared.preview()),
                    self.limits.response_bytes(),
                );
                if codec::is_overflow_response(&response) {
                    return response;
                }
                self.pending_rename = Some(prepared);
                self.state = SessionState::Prepared;
                response
            }
            Err(diagnostics) => {
                if invalidates(&diagnostics) {
                    self.state = SessionState::Invalidated;
                }
                self.finish(id, Err(diagnostics))
            }
        }
    }

    pub(super) fn rename_apply(
        &mut self,
        id: &RequestId,
        params: Option<Map<String, Value>>,
    ) -> Vec<u8> {
        self.rename_apply_with_runtime(id, params, &mut ProductionRuntime)
    }

    fn rename_apply_with_runtime(
        &mut self,
        id: &RequestId,
        params: Option<Map<String, Value>>,
        runtime: &mut impl RenameRuntime,
    ) -> Vec<u8> {
        if self.profile != ServerProfile::ProjectRenameV1 {
            return self.error(id, METHOD_NOT_FOUND, "method not found: rename/apply");
        }
        if self.state != SessionState::Prepared {
            return self.lifecycle_error(id);
        }
        let mut params = params.unwrap_or_default();
        let snapshot = self
            .snapshot
            .as_ref()
            .expect("a prepared rename session retains its authenticated snapshot");
        if let Err(message) = take_exact_revisions(snapshot, &mut params) {
            return self.error(id, codec::INVALID_PARAMS, &message);
        }
        let preview_digest = match take_string(&mut params, "preview_digest") {
            Ok(value) => value,
            Err(error) => return self.finish(id, Err(error)),
        };
        if let Err(error) = reject_unknown(&params) {
            return self.finish(id, Err(error));
        }
        let prepared = self
            .pending_rename
            .as_ref()
            .expect("prepared state retains one rename plan");
        if preview_digest != prepared.preview_digest() {
            return self.error(
                id,
                codec::INVALID_PARAMS,
                "preview_digest does not match the retained Project rename plan",
            );
        }

        // Both possible post-effect responses are rendered and bounded before
        // acquiring commit authority. No write can occur if success cannot be
        // represented under the negotiated response limit.
        let success = codec::bounded_success_response(
            id,
            &render_rename_receipt(prepared),
            self.limits.response_bytes(),
        );
        if codec::is_overflow_response(&success) {
            return success;
        }
        let uncertainty = codec::bounded_error_response(
            Some(id),
            APPLICATION_ERROR,
            "SPX-J110: Project rename publication outcome is uncertain; stop and inspect the bound project",
            self.limits.response_bytes(),
        );
        if codec::is_overflow_response(&uncertainty) {
            return uncertainty;
        }

        if let Err(diagnostics) = self
            .snapshot
            .as_mut()
            .expect("prepared state retains its authenticated snapshot")
            .with_authenticated_request(|_| Ok(()))
        {
            self.state = SessionState::Invalidated;
            return self.finish(id, Err(diagnostics));
        }
        if let Err(diagnostics) = runtime.before_a0() {
            return self.finish(id, Err(diagnostics));
        }
        let owned = match prepared.acquire_a0() {
            Ok(owned) => owned,
            Err(diagnostics) => return self.finish(id, Err(diagnostics)),
        };
        if let Err(diagnostics) = runtime.after_a0() {
            return self.finish(id, Err(diagnostics));
        }
        let prepared = self
            .pending_rename
            .take()
            .expect("prepared state retains one rename plan");

        // Release Project-held Windows file handles only after A0 owns the
        // exact source lock and snapshot. The consuming final recheck closes
        // the identity-continuity interval without a path-only reopen gap.
        let old_snapshot = self
            .snapshot
            .take()
            .expect("prepared state retains its authenticated snapshot");
        if let Err(diagnostics) = old_snapshot.finish_session() {
            self.state = SessionState::Invalidated;
            self.terminal_diagnostics = Some(diagnostics.clone());
            return self.finish(id, Err(diagnostics));
        }
        self.state = SessionState::Applying;
        let commit_result = runtime.commit(owned);
        let reloaded = runtime.reload(&self.manifest_path);
        match (commit_result, reloaded) {
            (Ok(committed_revision), Ok(snapshot))
                if committed_revision == prepared.candidate_source().source_revision()
                    && snapshot_matches_candidate(&snapshot, &prepared) =>
            {
                self.snapshot = Some(snapshot);
                self.state = SessionState::Open;
                success
            }
            (Err(diagnostics), Ok(snapshot)) if snapshot_matches_base(&snapshot, &prepared) => {
                self.snapshot = Some(snapshot);
                self.state = SessionState::Open;
                self.finish(id, Err(diagnostics))
            }
            (commit, reload) => {
                let mut diagnostics = vec![Diagnostic::io(
                    "SPX-J110",
                    "Project rename publication outcome is uncertain after the A0 commit boundary",
                )];
                if let Err(mut commit) = commit {
                    diagnostics.append(&mut commit);
                }
                if let Err(mut reload) = reload {
                    diagnostics.append(&mut reload);
                }
                self.snapshot = None;
                self.state = SessionState::Uncertain;
                self.terminal_diagnostics = Some(diagnostics);
                uncertainty
            }
        }
    }
}

fn render_rename_receipt(prepared: &PreparedProjectRename) -> String {
    format!(
        "{{\"applied\":true,\"preview_digest\":{},\"base_project_revision\":{},\"candidate_project_revision\":{},\"base_workspace_revision\":{},\"candidate_workspace_revision\":{},\"candidate_source_revision\":{},\"candidate_project_graph_digest\":{}}}",
        quote_json(prepared.preview_digest()),
        quote_json(prepared.base_project_revision()),
        quote_json(prepared.candidate_project_revision()),
        quote_json(prepared.base_workspace_revision()),
        quote_json(prepared.candidate_workspace_revision()),
        quote_json(prepared.candidate_source().source_revision()),
        quote_json(prepared.candidate_project_graph_digest()),
    )
}

fn snapshot_matches_candidate(
    snapshot: &ProjectSnapshot,
    prepared: &PreparedProjectRename,
) -> bool {
    snapshot.project_revision() == prepared.candidate_project_revision()
        && snapshot.workspace_revision() == prepared.candidate_workspace_revision()
        && snapshot.semantic_graph() == prepared.candidate_project_graph()
        && snapshot.sources().iter().any(|source| {
            source.path() == prepared.candidate_source().path()
                && source == prepared.candidate_source()
        })
}

fn snapshot_matches_base(snapshot: &ProjectSnapshot, prepared: &PreparedProjectRename) -> bool {
    snapshot.project_revision() == prepared.base_project_revision()
        && snapshot.workspace_revision() == prepared.base_workspace_revision()
        && snapshot.sources().iter().any(|source| {
            source.path() == prepared.base_source().path() && source == prepared.base_source()
        })
}

#[cfg(test)]
mod tests {
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

        fn session(&self) -> (Session, String, String) {
            let snapshot = crate::project::load_snapshot(&self.manifest()).unwrap();
            let project = snapshot.project_revision().to_owned();
            let workspace = snapshot.workspace_revision().to_owned();
            (
                Session {
                    snapshot: Some(snapshot),
                    state: SessionState::Open,
                    limits: crate::project_transport::framing::StdioLimits::default(),
                    profile: ServerProfile::ProjectRenameV1,
                    manifest_path: self.manifest(),
                    pending_rename: None,
                    terminal_diagnostics: None,
                },
                project,
                workspace,
            )
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
            codec::bounded_success_response(
                &id,
                &render_rename_receipt(&prepared),
                success_required
            ),
            success
        );
        assert!(codec::is_overflow_response(
            &codec::bounded_success_response(
                &id,
                &render_rename_receipt(&prepared),
                success_required - 1
            )
        ));

        let message = "SPX-J110: Project rename publication outcome is uncertain; stop and inspect the bound project";
        let uncertainty =
            codec::bounded_error_response(Some(&id), APPLICATION_ERROR, message, usize::MAX);
        let uncertainty_required = uncertainty.len() + 1;
        assert_eq!(
            codec::bounded_error_response(
                Some(&id),
                APPLICATION_ERROR,
                message,
                uncertainty_required
            ),
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
}
