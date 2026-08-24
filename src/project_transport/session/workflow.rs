//! Opt-in Project Agent Workflow v1 read/plan/build methods.
//!
//! Requests select no path, source, patch, output, tool, or environment.
//! The only retained mutation plan is derived from authenticated Project
//! meaning, and Web or Project-v2 npm builds are returned inline without
//! physical effects.

use serde_json::{Map, Value};

use super::{
    codec, invalidates, reject_unknown, take_exact_revisions, take_optional_usize, take_string,
    RequestId, ServerProfile, Session, SessionState, METHOD_NOT_FOUND,
};

const DEFAULT_INLINE_BUILD_BYTES: usize = 512 * 1024;

impl Session {
    pub(super) fn rename_derive(
        &mut self,
        id: &RequestId,
        params: Option<Map<String, Value>>,
    ) -> Vec<u8> {
        if self.profile != ServerProfile::ProjectWorkflowV1 {
            return self.error(id, METHOD_NOT_FOUND, "method not found: rename/derive");
        }
        if self.state != SessionState::Open {
            return self.lifecycle_error(id);
        }
        let mut params = params.unwrap_or_default();
        let snapshot = self
            .snapshot
            .as_ref()
            .expect("an open workflow session retains its authenticated snapshot");
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
            .expect("an open workflow session retains its authenticated snapshot")
            .with_authenticated_request(|snapshot| {
                let prepared = snapshot.prepare_rename(&target_id, &from, &to)?;
                let rendered = format!("{{\"derivation\":{}}}", prepared.derivation());
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
                self.state = SessionState::Derived;
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

    pub(super) fn change_preview(
        &mut self,
        id: &RequestId,
        params: Option<Map<String, Value>>,
    ) -> Vec<u8> {
        if self.profile != ServerProfile::ProjectWorkflowV1 {
            return self.error(id, METHOD_NOT_FOUND, "method not found: change/preview");
        }
        if self.state != SessionState::Derived {
            return self.lifecycle_error(id);
        }
        let mut params = params.unwrap_or_default();
        let snapshot = self
            .snapshot
            .as_ref()
            .expect("a derived workflow session retains its authenticated snapshot");
        if let Err(message) = take_exact_revisions(snapshot, &mut params) {
            return self.error(id, codec::INVALID_PARAMS, &message);
        }
        let derivation_digest = match take_string(&mut params, "derivation_digest") {
            Ok(value) => value,
            Err(error) => return self.finish(id, Err(error)),
        };
        if let Err(error) = reject_unknown(&params) {
            return self.finish(id, Err(error));
        }
        let prepared = self
            .pending_rename
            .as_ref()
            .expect("derived state retains one Project change plan");
        if derivation_digest != prepared.derivation_digest() {
            return self.error(
                id,
                codec::INVALID_PARAMS,
                "derivation_digest does not match the retained Project change plan",
            );
        }
        let prepared = self
            .pending_rename
            .as_ref()
            .expect("derived state retains one Project change plan");
        let rendered = self
            .snapshot
            .as_mut()
            .expect("derived state retains its authenticated snapshot")
            .with_authenticated_request(|_| Ok(render_change_result(prepared)));
        let rendered = match rendered {
            Ok(rendered) => rendered,
            Err(diagnostics) => {
                self.state = SessionState::Invalidated;
                return self.finish(id, Err(diagnostics));
            }
        };
        let response = codec::bounded_success_response(id, &rendered, self.limits.response_bytes());
        if codec::is_overflow_response(&response) {
            return response;
        }
        self.state = SessionState::Prepared;
        response
    }

    pub(super) fn change_impact(
        &mut self,
        id: &RequestId,
        params: Option<Map<String, Value>>,
    ) -> Vec<u8> {
        self.change_artifact(id, params, "impact")
    }

    pub(super) fn change_review(
        &mut self,
        id: &RequestId,
        params: Option<Map<String, Value>>,
    ) -> Vec<u8> {
        self.change_artifact(id, params, "review")
    }

    fn change_artifact(
        &mut self,
        id: &RequestId,
        params: Option<Map<String, Value>>,
        artifact: &str,
    ) -> Vec<u8> {
        if self.profile != ServerProfile::ProjectWorkflowV1 {
            return self.error(
                id,
                METHOD_NOT_FOUND,
                &format!("method not found: {artifact}"),
            );
        }
        if self.state != SessionState::Prepared {
            return self.lifecycle_error(id);
        }
        let mut params = params.unwrap_or_default();
        let snapshot = self
            .snapshot
            .as_ref()
            .expect("a prepared workflow session retains its authenticated snapshot");
        if let Err(message) = take_exact_revisions(snapshot, &mut params) {
            return self.error(id, codec::INVALID_PARAMS, &message);
        }
        let change_preview_digest = match take_string(&mut params, "change_preview_digest") {
            Ok(value) => value,
            Err(error) => return self.finish(id, Err(error)),
        };
        if let Err(error) = reject_unknown(&params) {
            return self.finish(id, Err(error));
        }
        let prepared = self
            .pending_rename
            .as_ref()
            .expect("prepared state retains one Project change plan");
        if change_preview_digest != prepared.change_preview_digest() {
            return self.error(
                id,
                codec::INVALID_PARAMS,
                "change_preview_digest does not match the retained Project change plan",
            );
        }
        let prepared = self
            .pending_rename
            .as_ref()
            .expect("prepared state retains one Project change plan");
        let value = if artifact == "impact" {
            prepared.impact()
        } else {
            prepared.review()
        };
        let rendered = self
            .snapshot
            .as_mut()
            .expect("prepared state retains its authenticated snapshot")
            .with_authenticated_request(|_| Ok(format!("{{\"{artifact}\":{value}}}")));
        match rendered {
            Ok(rendered) => self.finish(id, Ok(rendered)),
            Err(diagnostics) => {
                self.state = SessionState::Invalidated;
                self.finish(id, Err(diagnostics))
            }
        }
    }

    pub(super) fn build(&mut self, id: &RequestId, params: Option<Map<String, Value>>) -> Vec<u8> {
        if self.profile != ServerProfile::ProjectWorkflowV1 {
            return self.error(id, METHOD_NOT_FOUND, "method not found: build");
        }
        self.subject(id, params, |snapshot, mut params| {
            let target = take_string(&mut params, "target")?;
            let max_bytes = take_optional_usize(&mut params, "max_bytes")?
                .unwrap_or(DEFAULT_INLINE_BUILD_BYTES);
            reject_unknown(&params)?;
            let envelope = match target.as_str() {
                "web" => {
                    if snapshot.manifest().is_v2() {
                        let build = snapshot.build_npm_inline(max_bytes)?;
                        build.verify().map_err(|error| vec![error])?;
                        build.envelope().to_owned()
                    } else {
                        let build = snapshot.build_web_inline(max_bytes)?;
                        build.verify().map_err(|error| vec![error])?;
                        build.envelope().to_owned()
                    }
                }
                "npm" => {
                    let build = snapshot.build_npm_inline(max_bytes)?;
                    build.verify().map_err(|error| vec![error])?;
                    build.envelope().to_owned()
                }
                _ => {
                    return Err(super::parameter_diagnostic(
                        "target must be web or npm; native and Rust builds are outside daemon authority",
                    ));
                }
            };
            Ok(format!("{{\"build\":{envelope}}}"))
        })
    }
}

fn render_change_result(prepared: &crate::project::PreparedProjectRename) -> String {
    format!("{{\"change\":{}}}", prepared.change_preview())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn assert_exact_response_boundary(id: RequestId, result: &str) {
        let unconstrained = codec::bounded_success_response(&id, result, usize::MAX);
        let exact = unconstrained.len() + 1;
        assert_eq!(
            codec::bounded_success_response(&id, result, exact),
            unconstrained
        );
        assert!(codec::is_overflow_response(
            &codec::bounded_success_response(&id, result, exact - 1)
        ));
    }

    #[test]
    fn every_complete_v4_planning_and_build_response_has_an_exact_minimum_boundary() {
        let manifest =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project/semaprax.toml");
        let snapshot = crate::project::load_snapshot(&manifest).unwrap();
        let prepared = snapshot
            .prepare_rename("calculator.add", "add", "sum")
            .unwrap();
        let build = snapshot
            .build_web_inline(DEFAULT_INLINE_BUILD_BYTES)
            .unwrap();
        for (id, result) in [
            (811, format!("{{\"derivation\":{}}}", prepared.derivation())),
            (812, render_change_result(&prepared)),
            (813, format!("{{\"impact\":{}}}", prepared.impact())),
            (814, format!("{{\"review\":{}}}", prepared.review())),
            (815, format!("{{\"build\":{}}}", build.envelope())),
        ] {
            assert_exact_response_boundary(RequestId::Number(id), &result);
        }
    }
}
