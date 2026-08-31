//! Read-only, directional experiments through the ordinary candidate merge.

use std::sync::Arc;

use serde_json::{json, Value};

use super::{wire, ProjectCandidate, ProjectCandidateRebase, ProjectRevision};
use crate::diagnostic::Diagnostic;

pub const PROJECT_CANDIDATE_MERGE_PREVIEW_SCHEMA: &str =
    "semaprax.project-candidate-merge-preview.v1";
pub const PROJECT_CANDIDATE_MERGE_PREVIEW_VERIFICATION_SCHEMA: &str =
    "semaprax.project-candidate-merge-preview-verification.v1";
pub const MAX_PROJECT_CANDIDATE_MERGE_PREVIEW_BYTES: usize = 256 * 1024;
const MAX_DIAGNOSTICS: usize = 64;
const MAX_DIAGNOSTIC_BYTES: usize = 16 * 1024;
const REPORT_DOMAIN: &[u8] = b"semaprax.project-candidate-merge-preview.v1\0";
type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

impl ProjectCandidate {
    /// Try both merge orders without retaining either resulting candidate.
    /// Rejection can indicate an unsupported case or bound, not incompatibility.
    pub fn merge_preview(
        &self,
        expected_candidate: &str,
        other: &Self,
        expected_other: &str,
    ) -> Result<String> {
        self.preview_parents(expected_candidate, other, expected_other)?;
        let prefix = self
            .changes
            .iter()
            .zip(&other.changes)
            .take_while(|(left, right)| left.to_json() == right.to_json())
            .count();
        // Existing merge applies the argument's history before the receiver's
        // suffix. Keep the report's order literal, including shared prefixes.
        let left_then_right = other.merge(expected_other, self, expected_candidate);
        let right_then_left = self.merge(expected_candidate, other, expected_other);
        let same_source = match (&left_then_right, &right_then_left) {
            (Ok(left), Ok(right)) => Some(exact_source(
                left.candidate().revision(),
                right.candidate().revision(),
            )),
            _ => None,
        };
        render(json!({
            "schema":PROJECT_CANDIDATE_MERGE_PREVIEW_SCHEMA,
            "base_revision":self.base.project_revision(),
            "left_candidate_revision":expected_candidate,
            "right_candidate_revision":expected_other,
            "left_then_right":direction(&left_then_right, prefix)?,
            "right_then_left":direction(&right_then_left, prefix)?,
            "same_source":same_source,
            "tests":"not_run",
            "source_authority":false,
            "candidate_retained":false,
            "validation":"ordinary_merge_with_full_candidate_admission",
            "nonclaims":["not_behavioral_equivalence","not_runtime_or_test_execution",
                "not_external_consumer_compatibility",
                "not_permission_to_publish_or_retain_candidates",
                "directional_rejection_may_be_a_conservative_or_capacity_limit"]
        }))
    }

    /// Replay both complete parent histories and recompute exact report bytes.
    /// Submitted bytes cannot create a candidate or supply merge authority.
    pub fn verify_merge_preview(
        &self,
        expected_candidate: &str,
        other: &Self,
        expected_other: &str,
        bytes: &[u8],
    ) -> Result<String> {
        self.preview_parents(expected_candidate, other, expected_other)?;
        if bytes.len() > MAX_PROJECT_CANDIDATE_MERGE_PREVIEW_BYTES {
            return Err(capacity(
                "candidate merge preview input exceeds its byte bound",
            ));
        }
        let left = Self::replay(
            Arc::clone(&self.base),
            self.base.project_revision(),
            &self.changes,
            self.to_json().as_bytes(),
        )?;
        let right = Self::replay(
            Arc::clone(&other.base),
            other.base.project_revision(),
            &other.changes,
            other.to_json().as_bytes(),
        )?;
        if left
            .merge_preview(expected_candidate, &right, expected_other)?
            .as_bytes()
            != bytes
        {
            return Err(conflict(
                "candidate merge preview does not match exact recomputation",
            ));
        }
        render(json!({
            "schema":PROJECT_CANDIDATE_MERGE_PREVIEW_VERIFICATION_SCHEMA,
            "result":"exact_source_history_recomputation",
            "base_revision":self.base.project_revision(),
            "left_candidate_revision":expected_candidate,
            "right_candidate_revision":expected_other,
            "report_digest":wire::digest(REPORT_DOMAIN, bytes),
            "tests":"not_run","source_authority":false,"candidate_retained":false
        }))
    }

    fn preview_parents(
        &self,
        expected_candidate: &str,
        other: &Self,
        expected_other: &str,
    ) -> Result<()> {
        self.require_candidate(expected_candidate)?;
        other.require_candidate(expected_other)?;
        if self.base.project_revision() != other.base.project_revision()
            || !exact_source(&self.base, &other.base)
        {
            return Err(conflict(
                "candidate merge preview requires the same exact original Project base",
            ));
        }
        Ok(())
    }
}

fn exact_source(left: &ProjectRevision, right: &ProjectRevision) -> bool {
    left.manifest().to_canonical_toml() == right.manifest().to_canonical_toml()
        && left.sources().len() == right.sources().len()
        && left
            .sources()
            .iter()
            .zip(right.sources())
            .all(|(left, right)| left.path() == right.path() && left.source() == right.source())
}

fn direction(outcome: &Result<ProjectCandidateRebase>, prefix: usize) -> Result<Value> {
    match outcome {
        Ok(result) => {
            let candidate = result.candidate();
            let sources = candidate.revision().sources();
            let source_bytes = sources.iter().try_fold(0usize, |bytes, source| {
                bytes
                    .checked_add(source.source().len())
                    .ok_or_else(|| capacity("candidate merge preview source byte count overflow"))
            })?;
            Ok(json!({
                "status":"accepted",
                "result_project_revision":candidate.revision().project_revision(),
                "result_candidate_revision":candidate.candidate_digest(),
                "shared_history_prefix":prefix,
                "source_file_count":sources.len(),"source_bytes":source_bytes
            }))
        }
        Err(diagnostics) => {
            if diagnostics.is_empty() || diagnostics.len() > MAX_DIAGNOSTICS {
                return Err(capacity(
                    "candidate merge preview diagnostic count exceeds its bound",
                ));
            }
            let mut remaining = MAX_DIAGNOSTIC_BYTES;
            for diagnostic in diagnostics {
                for text in [diagnostic.code, diagnostic.message.as_str()] {
                    remaining = remaining.checked_sub(text.len()).ok_or_else(|| {
                        capacity("candidate merge preview diagnostic text exceeds its bound")
                    })?;
                }
            }
            Ok(json!({
                "status":"rejected",
                "diagnostics":diagnostics.iter().map(|diagnostic| json!({
                    "code":diagnostic.code,"message":diagnostic.message
                })).collect::<Vec<_>>(),
                "interpretation":"merge_rejected_not_proof_of_incompatibility"
            }))
        }
    }
}

fn render(value: Value) -> Result<String> {
    wire::render(value, MAX_PROJECT_CANDIDATE_MERGE_PREVIEW_BYTES)
        .map_err(|_| capacity("candidate merge preview report exceeds its byte bound"))
}

fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G226", message)]
}

fn conflict(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G235", message)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejection_projection_is_closed_and_never_truncates_diagnostics() {
        let diagnostic =
            Diagnostic::io("SPX-G235", "conservative conflict").at_path("private/source.spx");
        let accepted = direction(&Err(vec![diagnostic.clone(); MAX_DIAGNOSTICS]), 0).unwrap();
        assert_eq!(
            accepted["diagnostics"].as_array().unwrap().len(),
            MAX_DIAGNOSTICS
        );
        assert_eq!(accepted["diagnostics"][0].as_object().unwrap().len(), 2);
        assert!(accepted["diagnostics"][0].get("path").is_none());
        let rejected = direction(&Err(vec![diagnostic; MAX_DIAGNOSTICS + 1]), 0).unwrap_err();
        assert_eq!(rejected[0].code, "SPX-G226");
        assert_eq!(
            direction(&Err(Vec::new()), 0).unwrap_err()[0].code,
            "SPX-G226"
        );
    }

    #[test]
    fn diagnostic_budget_counts_utf8_and_both_code_and_message() {
        let code = "SPX-G235";
        let message = "é".repeat((MAX_DIAGNOSTIC_BYTES - code.len()) / 2);
        let mut diagnostics = vec![Diagnostic::io(code, message)];
        assert!(direction(&Err(diagnostics.clone()), 0).is_ok());
        diagnostics[0].message.push('x');
        assert_eq!(
            direction(&Err(diagnostics), 0).unwrap_err()[0].code,
            "SPX-G226"
        );
        assert_eq!(
            render(json!({"text":"x".repeat(MAX_PROJECT_CANDIDATE_MERGE_PREVIEW_BYTES)}))
                .unwrap_err()[0]
                .code,
            "SPX-G226"
        );
    }
}
