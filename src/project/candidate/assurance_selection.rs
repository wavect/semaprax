//! Compact candidate-bound selection of independently replayed assurance input.
//!
//! This module deliberately delegates all envelope replay, source rebinding,
//! obligation derivation, and assurance classification to
//! [`ProjectCandidate::candidate_assurance_summary`]. It only records which
//! verified inputs were selected and a compact coverage/limitation projection.
//! The record is evidence, never candidate acceptance, test execution, effect
//! observation, source authority, or publication authority.

use std::collections::BTreeSet;

use serde_json::{json, Value};

use crate::diagnostic::Diagnostic;

use super::{wire, CandidateAssuranceInput, ProjectCandidate};

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

pub const PROJECT_CANDIDATE_ASSURANCE_SELECTION_SCHEMA: &str =
    "semaprax.project-candidate-assurance-selection.v1";
pub const PROJECT_CANDIDATE_ASSURANCE_SELECTION_VERIFICATION_SCHEMA: &str =
    "semaprax.project-candidate-assurance-selection-verification.v1";
pub const MAX_PROJECT_CANDIDATE_ASSURANCE_SELECTION_BYTES: usize = 8 * 1024;

const SELECTION_DOMAIN: &[u8] = b"semaprax.project-candidate-assurance-selection.v1\0";

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G934", message)]
}
fn stale(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G935", message)]
}

fn canonical_selection(content: Value) -> Result<(String, String)> {
    let canonical = wire::render(content, MAX_PROJECT_CANDIDATE_ASSURANCE_SELECTION_BYTES)?;
    let digest = wire::digest(SELECTION_DOMAIN, canonical.as_bytes());
    Ok((canonical, digest))
}

fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("candidate assurance summary has an invalid required text field"))
}

fn count(value: &Value, key: &str) -> Result<u64> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid("candidate assurance summary has an invalid required count field"))
}

fn values(value: &Value, key: &str) -> Result<Vec<Value>> {
    value
        .get(key)
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| invalid("candidate assurance summary has an invalid required array field"))
}

fn selected_inputs(inputs: &[CandidateAssuranceInput<'_>]) -> Vec<Value> {
    let mut selected = inputs
        .iter()
        .map(|input| {
            json!({
                "path": input.path,
                "envelope_sha256": wire::digest(
                    b"semaprax.project-candidate-assurance-envelope.v1\0",
                    input.envelope.as_bytes(),
                ),
            })
        })
        .collect::<Vec<_>>();
    selected.sort_by(|left, right| left["path"].as_str().cmp(&right["path"].as_str()));
    selected
}

fn content(candidate: &ProjectCandidate, inputs: &[CandidateAssuranceInput<'_>]) -> Result<Value> {
    let summary_text =
        candidate.candidate_assurance_summary(candidate.candidate_digest(), inputs)?;
    let summary: Value = serde_json::from_str(&summary_text)
        .map_err(|_| invalid("candidate assurance summary is not valid compiler JSON"))?;
    if text(&summary, "candidate_revision")? != candidate.candidate_digest()
        || text(&summary, "base_project_revision")? != candidate.base_revision().project_revision()
        || text(&summary, "project_revision")? != candidate.revision().project_revision()
    {
        return Err(stale(
            "candidate assurance summary does not bind the selected candidate revisions",
        ));
    }
    let selected = selected_inputs(inputs);
    let unique_paths = selected
        .iter()
        .filter_map(|row| row["path"].as_str())
        .collect::<BTreeSet<_>>();
    if unique_paths.len() != selected.len() {
        return Err(invalid(
            "candidate assurance selection inputs must name each source path at most once",
        ));
    }
    let sources_not_observed = values(&summary, "sources_not_observed")?;
    let unsupported_formal_claims = values(&summary, "unsupported_formal_claims")?;
    let kinds_not_yet_derived = values(&summary, "kinds_not_yet_derived")?;
    let sources_total = count(&summary, "sources_total")?;
    let sources_bound = count(&summary, "sources_bound")?;
    let obligations_total = count(&summary, "obligations_total")?;
    if sources_bound != selected.len() as u64
        || sources_bound.saturating_add(sources_not_observed.len() as u64) != sources_total
    {
        return Err(invalid(
            "candidate assurance summary coverage does not match selected source inputs",
        ));
    }
    Ok(json!({
        "schema": PROJECT_CANDIDATE_ASSURANCE_SELECTION_SCHEMA,
        "binding": {
            "candidate_revision": candidate.candidate_digest(),
            "base_project_revision": candidate.base_revision().project_revision(),
            "project_revision": candidate.revision().project_revision(),
        },
        "assurance_summary": {
            "schema": text(&summary, "schema")?,
            "sha256": wire::digest(
                b"semaprax.project-candidate-assurance-summary.v1\0",
                summary_text.as_bytes(),
            ),
            "availability": "replay_with_selected_candidate_assurance_inputs",
            "resolver": "candidate_assurance_summary",
        },
        "selected_inputs": selected,
        "coverage": {
            "sources_total": sources_total,
            "sources_bound": sources_bound,
            "sources_not_observed": sources_not_observed,
            "obligations_total": obligations_total,
            "by_class": summary["by_class"],
            "status": if sources_bound == sources_total { "complete_for_selected_assurance_producer" } else { "partial" },
        },
        "limitations": {
            "unsupported_formal_claims": unsupported_formal_claims,
            "kinds_not_yet_derived": kinds_not_yet_derived,
            "summary_nonclaims": values(&summary, "nonclaims")?,
        },
        "execution": false,
        "source_authority": false,
        "publication_authority": false,
        "nonclaims": [
            "no_caller_authored_assurance_verdicts",
            "no_obligation_derivation_or_assurance_lattice_reimplementation",
            "no_test_execution_or_effect_observation",
            "no_candidate_acceptance_or_publication_authority",
            "selected_envelopes_are_replayed_not_retained_by_this_compact_reference",
        ],
    }))
}

impl ProjectCandidate {
    /// Select independently replayed candidate assurance envelopes into a
    /// compact record. The content is deterministic and its digest binds the
    /// exact candidate/base/result revisions and envelope bytes selected.
    pub fn candidate_assurance_selection(
        &self,
        expected_candidate: &str,
        inputs: &[CandidateAssuranceInput<'_>],
    ) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        let (canonical, digest) = canonical_selection(content(self, inputs)?)?;
        wire::render(
            json!({
                "selection": serde_json::from_str::<Value>(&canonical).expect("canonical compiler JSON"),
                "selection_digest": digest,
            }),
            MAX_PROJECT_CANDIDATE_ASSURANCE_SELECTION_BYTES,
        )
    }

    /// Independently replay the selected envelope inputs and require exact
    /// canonical selection bytes. Rehashing modified selection JSON is not
    /// verification because all compiler-derived facts are recomputed first.
    pub fn verify_candidate_assurance_selection(
        &self,
        expected_candidate: &str,
        inputs: &[CandidateAssuranceInput<'_>],
        bytes: &[u8],
    ) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        if bytes.len() > MAX_PROJECT_CANDIDATE_ASSURANCE_SELECTION_BYTES {
            return Err(invalid(
                "candidate assurance selection exceeds its byte bound",
            ));
        }
        let expected = self.candidate_assurance_selection(expected_candidate, inputs)?;
        if expected.as_bytes() != bytes {
            return Err(stale(
                "candidate assurance selection failed exact candidate-assurance replay",
            ));
        }
        let value: Value = serde_json::from_slice(bytes)
            .map_err(|_| invalid("candidate assurance selection is not valid JSON"))?;
        let digest = value
            .get("selection_digest")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("candidate assurance selection digest is absent"))?;
        wire::render(
            json!({
                "schema": PROJECT_CANDIDATE_ASSURANCE_SELECTION_VERIFICATION_SCHEMA,
                "selection_digest": digest,
                "candidate_revision": self.candidate_digest(),
                "verified": true,
                "execution": false,
                "source_authority": false,
                "publication_authority": false,
                "nonclaims": [
                    "no_candidate_acceptance_or_publication_authority",
                    "no_test_execution_or_effect_observation",
                ],
            }),
            MAX_PROJECT_CANDIDATE_ASSURANCE_SELECTION_BYTES,
        )
    }
}
