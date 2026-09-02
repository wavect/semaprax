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
            .with_authenticated_request(|snapshot| {
                let prepared = snapshot.prepare_rename(&target_id, &from, &to)?;
                let rendered = format!("{{\"preview\":{}}}", prepared.preview());
                Ok((prepared, rendered))
            });
        match result {
            Ok((prepared, rendered)) => {
                let response =
                    codec::bounded_success_response(id, &rendered, self.limits.response_bytes());
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
        self.apply_with_runtime(id, params, "rename/apply", &mut ProductionRuntime)
    }

    pub(super) fn change_apply(
        &mut self,
        id: &RequestId,
        params: Option<Map<String, Value>>,
    ) -> Vec<u8> {
        self.apply_with_runtime(id, params, "change/apply", &mut ProductionRuntime)
    }

    #[cfg(test)]
    fn rename_apply_with_runtime(
        &mut self,
        id: &RequestId,
        params: Option<Map<String, Value>>,
        runtime: &mut impl RenameRuntime,
    ) -> Vec<u8> {
        self.apply_with_runtime(id, params, "rename/apply", runtime)
    }

    #[cfg(test)]
    fn change_apply_with_runtime(
        &mut self,
        id: &RequestId,
        params: Option<Map<String, Value>>,
        runtime: &mut impl RenameRuntime,
    ) -> Vec<u8> {
        self.apply_with_runtime(id, params, "change/apply", runtime)
    }

    fn apply_with_runtime(
        &mut self,
        id: &RequestId,
        params: Option<Map<String, Value>>,
        method: &str,
        runtime: &mut impl RenameRuntime,
    ) -> Vec<u8> {
        let allowed = match method {
            "rename/apply" => matches!(self.profile, ServerProfile::ProjectRenameV1),
            "change/apply" => self.profile == ServerProfile::ProjectWorkflowV1,
            _ => false,
        };
        if !allowed {
            return self.error(id, METHOD_NOT_FOUND, &format!("method not found: {method}"));
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
        let digest_parameter = if method == "change/apply" {
            "change_preview_digest"
        } else {
            "preview_digest"
        };
        let submitted_digest = match take_string(&mut params, digest_parameter) {
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
        let expected_digest = if method == "change/apply" {
            prepared.change_preview_digest()
        } else {
            prepared.preview_digest()
        };
        if submitted_digest != expected_digest {
            return self.error(
                id,
                codec::INVALID_PARAMS,
                &format!("{digest_parameter} does not match the retained Project change plan"),
            );
        }

        // Both possible post-effect responses are rendered and bounded before
        // acquiring commit authority. No write can occur if success cannot be
        // represented under the negotiated response limit.
        let receipt = if method == "change/apply" {
            render_change_receipt(prepared)
        } else {
            render_rename_receipt(prepared)
        };
        let success = codec::bounded_success_response(id, &receipt, self.limits.response_bytes());
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

fn render_change_receipt(prepared: &PreparedProjectRename) -> String {
    format!(
        "{{\"applied\":true,\"change_preview_digest\":{},\"rename_preview_digest\":{},\"impact_digest\":{},\"review_digest\":{},\"base_project_revision\":{},\"candidate_project_revision\":{},\"base_workspace_revision\":{},\"candidate_workspace_revision\":{},\"candidate_source_revision\":{},\"candidate_project_graph_digest\":{}}}",
        quote_json(prepared.change_preview_digest()),
        quote_json(prepared.preview_digest()),
        quote_json(prepared.impact_digest()),
        quote_json(prepared.review_digest()),
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
#[path = "rename/tests.rs"]
mod tests;
