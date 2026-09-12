//! Requirement traceability bound to exact assurance subjects.
//!
//! See [`docs/REQUIREMENT-TRACEABILITY-V1.md`](../docs/REQUIREMENT-TRACEABILITY-V1.md)
//! for the full specification this module implements. It composes
//! [`crate::assurance_manifest`] (#183): it never re-implements obligation
//! derivation, the assurance lattice, or envelope replay, and it never
//! executes a target, discovers or runs project tests, writes source, or
//! grants publication/execution authority. Like
//! [`crate::project::candidate::candidate_assurance`] (#129), it is a
//! read-only join over an *existing* evidence artifact.
//!
//! A [`Requirement`] is a caller-assigned persistent identity plus a bounded
//! set of [`RequirementCriterion`] entries. Each criterion names one *exact
//! assurance subject*: an [`crate::assurance_manifest::obligation_id`]
//! within one exact source path. It never accepts a free-text description,
//! a declaration name alone, or a file path alone as a criterion's subject,
//! because none of those fail closed when the thing they name changes.
//! [`evaluate_requirement`] independently re-verifies and rebinds every
//! supplied `semaprax.assurance-manifest.v1` envelope to the exact current
//! bytes of its named source path (via
//! [`crate::assurance_manifest::verify_envelope_against_source`], never
//! trusting the envelope's own self-reported path or revision) before
//! deriving one conservative satisfaction verdict per requirement.
//!
//! Diagnostics use the previously unused `SPX-Z4xx` family:
//! - `SPX-Z401`: invalid requirement, criterion, or call input (empty or
//!   over-bound id/title/path, or no criteria to evaluate).
//! - `SPX-Z402`: criteria or evidence count exceeds its bound, or the
//!   rendered report exceeds its byte budget; fail closed, never truncated.
//! - `SPX-Z403`: an ambiguous or malformed input this module refuses to
//!   silently resolve (a duplicate criterion naming the same exact subject
//!   twice, or evidence naming one source path more than once).
//!
//! A structurally malformed or drift-rejected *assurance envelope itself*
//! keeps its own `crate::assurance_manifest` diagnostic code
//! (`SPX-Z101`..`SPX-Z104`): only the source-drift case (`SPX-Z104`) is
//! caught here and turned into a per-criterion `stale` verdict, because
//! drift is the expected "the subject changed" case this module exists to
//! report conservatively rather than abort on. Any other envelope
//! malformation is a caller/input error and is propagated unchanged.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::{json, Value};

use crate::assurance_manifest::{self, AssuranceClass};
use crate::diagnostic::Diagnostic;

pub const SCHEMA: &str = "semaprax.requirement-traceability.v1";
pub const MAX_REQUIREMENT_ID_BYTES: usize = 256;
pub const MAX_REQUIREMENT_TITLE_BYTES: usize = 4096;
pub const MAX_REQUIREMENT_CRITERIA: usize = 256;
pub const MAX_EVIDENCE_INPUTS: usize = 256;
pub const MAX_REQUIREMENT_REPORT_BYTES: usize = 1_048_576;

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-Z401", message.into())
}

fn capacity(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-Z402", message.into())
}

fn consistency(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-Z403", message.into())
}

/// One requirement criterion's outcome against one requirement's currently
/// supplied evidence. Never derived from anything but the freshly
/// re-verified, source-rebound envelope for its exact `source_path`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CriterionStatus {
    /// The obligation's current classification meets or exceeds the
    /// required minimum through machine-checked evidence.
    Satisfied,
    /// The obligation's current classification meets or exceeds the
    /// required minimum, but only through `assumed`/`attempt_inconclusive`
    /// evidence: never rendered as a plain `satisfied` claim so that an
    /// explicit human assumption is never mistaken for technical proof.
    Assumed,
    /// The obligation exists and was classified, but the classification
    /// does not meet or exceed the required minimum.
    Unmet,
    /// The named obligation `id` (the exact assurance subject) is absent
    /// from the current, source-bound evidence for this source path.
    Dangling,
    /// Evidence was supplied for this source path, but it no longer matches
    /// the current bytes on disk: the exact subject drifted since the
    /// evidence was generated.
    Stale,
    /// No evidence was supplied at all for this criterion's source path.
    Unevaluable,
}

impl CriterionStatus {
    pub const ALL: [Self; 6] = [
        Self::Satisfied,
        Self::Assumed,
        Self::Unmet,
        Self::Dangling,
        Self::Stale,
        Self::Unevaluable,
    ];

    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Satisfied => "satisfied",
            Self::Assumed => "assumed",
            Self::Unmet => "unmet",
            Self::Dangling => "dangling",
            Self::Stale => "stale",
            Self::Unevaluable => "unevaluable",
        }
    }

    #[must_use]
    pub fn from_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|status| status.token() == token)
    }
}

/// One requirement's conservative, whole-requirement satisfaction verdict,
/// derived from the worst-first aggregate of its criteria: `stale` outranks
/// `dangling` (drift is fail-closed before a broken link is even
/// meaningful), which outranks `unevaluable` (nothing to conclude), which
/// outranks the positive/negative mix (`partial`/`assumed`/`failed`), which
/// outranks a clean `satisfied`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequirementSatisfaction {
    Satisfied,
    Assumed,
    Partial,
    Failed,
    Stale,
    Unevaluable,
}

impl RequirementSatisfaction {
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Satisfied => "satisfied",
            Self::Assumed => "assumed",
            Self::Partial => "partial",
            Self::Failed => "failed",
            Self::Stale => "stale",
            Self::Unevaluable => "unevaluable",
        }
    }
}

/// One exact assurance subject a requirement criterion is bound to: an
/// [`crate::assurance_manifest::obligation_id`] within one exact source
/// path, and the minimum [`AssuranceClass`] required of its current
/// classification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequirementCriterion {
    source_path: String,
    obligation_id: String,
    minimum_class: AssuranceClass,
}

impl RequirementCriterion {
    pub fn new(
        source_path: impl Into<String>,
        obligation_id: impl Into<String>,
        minimum_class: AssuranceClass,
    ) -> Result<Self, Diagnostic> {
        let source_path = source_path.into();
        let obligation_id = obligation_id.into();
        if source_path.is_empty() {
            return Err(invalid(
                "requirement criterion source_path must not be empty",
            ));
        }
        if obligation_id.is_empty() {
            return Err(invalid(
                "requirement criterion obligation_id must not be empty",
            ));
        }
        Ok(Self {
            source_path,
            obligation_id,
            minimum_class,
        })
    }

    #[must_use]
    pub fn source_path(&self) -> &str {
        &self.source_path
    }

    #[must_use]
    pub fn obligation_id(&self) -> &str {
        &self.obligation_id
    }

    #[must_use]
    pub fn minimum_class(&self) -> AssuranceClass {
        self.minimum_class
    }
}

/// A stable, caller-assigned requirement identity plus its bounded set of
/// [`RequirementCriterion`] entries. `title` is explanatory natural-language
/// text; it is never consulted by [`evaluate_requirement`] and never
/// substitutes for a criterion's machine-checkable subject.
#[derive(Clone, Debug)]
pub struct Requirement {
    id: String,
    title: String,
    criteria: Vec<RequirementCriterion>,
}

impl Requirement {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Result<Self, Diagnostic> {
        let id = id.into();
        let title = title.into();
        if id.is_empty() || id.len() > MAX_REQUIREMENT_ID_BYTES {
            return Err(invalid(
                "requirement id must be non-empty and within its byte bound",
            ));
        }
        if title.len() > MAX_REQUIREMENT_TITLE_BYTES {
            return Err(invalid("requirement title exceeds its byte bound"));
        }
        Ok(Self {
            id,
            title,
            criteria: Vec::new(),
        })
    }

    /// Attach one criterion. Refuses (`SPX-Z402`) once
    /// [`MAX_REQUIREMENT_CRITERIA`] is reached, and refuses (`SPX-Z403`) an
    /// exact duplicate: a second criterion naming the identical
    /// `(source_path, obligation_id)` pair as one already attached is an
    /// ambiguous link (which minimum class governs?), never silently
    /// merged or overwritten.
    pub fn with_criterion(mut self, criterion: RequirementCriterion) -> Result<Self, Diagnostic> {
        if self.criteria.len() >= MAX_REQUIREMENT_CRITERIA {
            return Err(capacity("requirement criteria count exceeds its bound"));
        }
        let id = self.id.clone();
        if self.criteria.iter().any(|existing| {
            existing.source_path == criterion.source_path
                && existing.obligation_id == criterion.obligation_id
        }) {
            return Err(consistency(format!(
                "requirement `{id}` already has a criterion naming obligation \
                 `{}` at `{}`; an ambiguous duplicate link is rejected rather \
                 than silently merged or overwritten",
                criterion.obligation_id, criterion.source_path
            )));
        }
        self.criteria.push(criterion);
        Ok(self)
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn criteria(&self) -> &[RequirementCriterion] {
        &self.criteria
    }
}

/// One caller-supplied `semaprax.assurance-manifest.v1` envelope, claimed to
/// describe the exact current bytes at `source_path`. Never trusted as-is:
/// [`evaluate_requirement`] independently re-verifies and rebinds it via
/// [`crate::assurance_manifest::verify_envelope_against_source`].
#[derive(Clone, Copy, Debug)]
pub struct EvidenceInput<'a> {
    pub source_path: &'a str,
    pub envelope: &'a str,
}

enum EvidenceOutcome {
    Verified(Value),
    Stale,
    Unevaluable,
}

fn verify_one(
    source_path: &str,
    evidence: &[EvidenceInput<'_>],
) -> Result<EvidenceOutcome, Diagnostic> {
    let Some(item) = evidence
        .iter()
        .find(|candidate| candidate.source_path == source_path)
    else {
        return Ok(EvidenceOutcome::Unevaluable);
    };
    match assurance_manifest::verify_envelope_against_source(
        item.envelope,
        Path::new(item.source_path),
    ) {
        Ok(()) => {
            let value: Value = serde_json::from_str(item.envelope).map_err(|_| {
                consistency("assurance-manifest envelope is not valid JSON despite passing replay")
            })?;
            Ok(EvidenceOutcome::Verified(value))
        }
        Err(diagnostic) if diagnostic.code == "SPX-Z104" => Ok(EvidenceOutcome::Stale),
        Err(diagnostic) => Err(diagnostic),
    }
}

fn evaluate_criterion(criterion: &RequirementCriterion, outcome: &EvidenceOutcome) -> Value {
    let source_path = criterion.source_path();
    let obligation_id = criterion.obligation_id();
    let minimum_class = criterion.minimum_class();
    let (status, achieved_class, reason): (CriterionStatus, Option<AssuranceClass>, String) =
        match outcome {
            EvidenceOutcome::Unevaluable => (
                CriterionStatus::Unevaluable,
                None,
                format!("no assurance-manifest evidence supplied for source path `{source_path}`"),
            ),
            EvidenceOutcome::Stale => (
                CriterionStatus::Stale,
                None,
                format!(
                    "assurance-manifest evidence for `{source_path}` no longer matches the \
                     current source bytes at that path; regenerate it against current bytes \
                     before this criterion can be evaluated"
                ),
            ),
            EvidenceOutcome::Verified(envelope) => {
                let obligations = envelope["payload"]["obligations"].as_array().expect(
                    "verify_envelope_against_source already validated payload.obligations is an array",
                );
                let found = obligations
                    .iter()
                    .find(|obligation| obligation["id"].as_str() == Some(obligation_id));
                match found {
                    None => (
                        CriterionStatus::Dangling,
                        None,
                        format!(
                            "obligation `{obligation_id}` is not present in the current, \
                             source-bound assurance-manifest evidence for `{source_path}`; the \
                             exact assurance subject this criterion named no longer exists there"
                        ),
                    ),
                    Some(obligation) => {
                        let token = obligation["classification"].as_str().expect(
                            "verify_envelope_against_source already validated classification is \
                             a closed token",
                        );
                        let achieved = AssuranceClass::from_token(token).expect(
                            "verify_envelope_against_source already validated the classification \
                             vocabulary",
                        );
                        let meets = achieved == minimum_class
                            || assurance_manifest::dominates(achieved, minimum_class);
                        if !meets {
                            (
                                CriterionStatus::Unmet,
                                Some(achieved),
                                format!(
                                    "obligation `{obligation_id}` is currently classified \
                                     `{token}`, which does not meet or exceed the required \
                                     minimum `{}`",
                                    minimum_class.token()
                                ),
                            )
                        } else if matches!(
                            achieved,
                            AssuranceClass::Assumed | AssuranceClass::AttemptInconclusive
                        ) {
                            (
                                CriterionStatus::Assumed,
                                Some(achieved),
                                format!(
                                    "obligation `{obligation_id}` meets the required minimum \
                                     `{}` only through `{token}` evidence, never machine-checked \
                                     proof",
                                    minimum_class.token()
                                ),
                            )
                        } else {
                            (
                                CriterionStatus::Satisfied,
                                Some(achieved),
                                format!(
                                    "obligation `{obligation_id}` is currently classified \
                                     `{token}`, meeting the required minimum `{}`",
                                    minimum_class.token()
                                ),
                            )
                        }
                    }
                }
            }
        };
    json!({
        "source_path": source_path,
        "obligation_id": obligation_id,
        "minimum_class": minimum_class.token(),
        "status": status.token(),
        "achieved_class": achieved_class.map(AssuranceClass::token),
        "reason": reason,
    })
}

fn aggregate(statuses: &[CriterionStatus]) -> RequirementSatisfaction {
    if statuses.contains(&CriterionStatus::Stale) {
        return RequirementSatisfaction::Stale;
    }
    if statuses.contains(&CriterionStatus::Dangling) {
        return RequirementSatisfaction::Failed;
    }
    if statuses.contains(&CriterionStatus::Unevaluable) {
        return RequirementSatisfaction::Unevaluable;
    }
    let has_assumed = statuses.contains(&CriterionStatus::Assumed);
    let has_unmet = statuses.contains(&CriterionStatus::Unmet);
    let has_satisfied = statuses.contains(&CriterionStatus::Satisfied);
    if !has_unmet && !has_assumed {
        return RequirementSatisfaction::Satisfied;
    }
    if !has_unmet {
        return RequirementSatisfaction::Assumed;
    }
    if has_satisfied || has_assumed {
        return RequirementSatisfaction::Partial;
    }
    RequirementSatisfaction::Failed
}

/// Evaluate `requirement` against `evidence`, returning the canonical
/// `semaprax.requirement-traceability.v1` report as a JSON string.
///
/// Refuses closed (`SPX-Z401`) when `requirement` has no criteria at all:
/// there is nothing machine-checkable to evaluate, and a requirement with no
/// criteria is never silently reported "satisfied". Refuses (`SPX-Z402`)
/// when `evidence` exceeds [`MAX_EVIDENCE_INPUTS`] or when the rendered
/// report would exceed [`MAX_REQUIREMENT_REPORT_BYTES`]. Refuses
/// (`SPX-Z403`) when `evidence` names one source path more than once: which
/// envelope would govern is ambiguous, and this module never silently picks
/// one.
///
/// Every supplied envelope is independently re-verified and rebound to the
/// exact current bytes of its named source path via
/// [`crate::assurance_manifest::verify_envelope_against_source`] — never
/// trusted as handed in, and never matched against `payload.source.path`
/// (caller-supplied display text). A structural/malformation failure in an
/// envelope (any `crate::assurance_manifest` code other than the drift code
/// `SPX-Z104`) is propagated unchanged: only drift is downgraded to a
/// per-criterion `stale` verdict, because drift is the case this module
/// exists to report rather than treat as an aborting error.
pub fn evaluate_requirement(
    requirement: &Requirement,
    evidence: &[EvidenceInput<'_>],
) -> Result<String, Diagnostic> {
    if requirement.criteria.is_empty() {
        return Err(invalid(format!(
            "requirement `{}` has no criteria to evaluate",
            requirement.id
        )));
    }
    if evidence.len() > MAX_EVIDENCE_INPUTS {
        return Err(capacity(
            "requirement evidence input count exceeds its bound",
        ));
    }
    let mut seen_paths: BTreeSet<&str> = BTreeSet::new();
    for item in evidence {
        if !seen_paths.insert(item.source_path) {
            return Err(consistency(format!(
                "requirement evidence names source path `{}` more than once; ambiguous \
                 evidence is rejected rather than silently picking one",
                item.source_path
            )));
        }
    }

    let mut outcomes: BTreeMap<&str, EvidenceOutcome> = BTreeMap::new();
    for criterion in &requirement.criteria {
        if !outcomes.contains_key(criterion.source_path()) {
            let outcome = verify_one(criterion.source_path(), evidence)?;
            outcomes.insert(criterion.source_path(), outcome);
        }
    }

    let mut statuses: Vec<CriterionStatus> = Vec::with_capacity(requirement.criteria.len());
    let mut criteria_json: Vec<Value> = Vec::with_capacity(requirement.criteria.len());
    for criterion in &requirement.criteria {
        let outcome = outcomes
            .get(criterion.source_path())
            .expect("every criterion's source path was populated into outcomes above");
        let rendered = evaluate_criterion(criterion, outcome);
        let status_token = rendered["status"]
            .as_str()
            .expect("evaluate_criterion always renders a closed status token");
        let status = CriterionStatus::from_token(status_token)
            .expect("evaluate_criterion only ever renders its own closed vocabulary");
        statuses.push(status);
        criteria_json.push(rendered);
    }
    criteria_json.sort_by(|left, right| {
        let left_key = (
            left["source_path"].as_str().unwrap_or_default(),
            left["obligation_id"].as_str().unwrap_or_default(),
        );
        let right_key = (
            right["source_path"].as_str().unwrap_or_default(),
            right["obligation_id"].as_str().unwrap_or_default(),
        );
        left_key.cmp(&right_key)
    });

    let satisfaction = aggregate(&statuses);
    let value = json!({
        "schema": SCHEMA,
        "requirement_id": requirement.id,
        "title": requirement.title,
        "satisfaction": satisfaction.token(),
        "criteria_total": criteria_json.len(),
        "criteria": criteria_json,
        "nonclaims": [
            "not_a_language_syntax_feature",
            "not_publication_or_execution_authority",
            "not_human_approval_or_policy",
            "no_target_execution",
            "no_project_test_discovery_or_execution",
            "read_only_no_source_changes",
            "natural_language_title_is_not_evidence",
        ],
    });
    let rendered = serde_json::to_string(&value).expect("json value always serializes");
    if rendered.len() > MAX_REQUIREMENT_REPORT_BYTES {
        return Err(capacity(
            "requirement traceability report exceeds its byte budget; refusing to truncate",
        ));
    }
    Ok(rendered)
}

#[cfg(test)]
mod tests;
