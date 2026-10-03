//! Explicit candidate strict-law join. This does not install a global policy.
use super::{publication, wire, ProjectCandidate};
use crate::assurance_manifest::{
    law_set::{
        protected::{ProtectedLawBaseline, SpecificationChangeApproval},
        strict::{self, StrictLawPolicy},
        LawSet,
    },
    VerifiedProjectProof,
};
use crate::diagnostic::Diagnostic;
use std::path::Path;

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;
pub const STRICT_CANDIDATE_LAW_SCHEMA: &str = "semaprax.project-candidate-strict-law-assurance.v1";

/// Independently supplied host policy/evidence selection. No field is a proof
/// string or an execution capability; opaque approvals/proofs retain their owners.
pub struct StrictCandidateLawInputs<'a> {
    pub protection: &'a ProtectedLawBaseline,
    pub policy: &'a StrictLawPolicy,
    pub laws: &'a LawSet,
    pub proofs: &'a [VerifiedProjectProof],
    pub specification_approval: Option<&'a SpecificationChangeApproval>,
}
impl StrictCandidateLawInputs<'_> {
    fn require(&self, candidate: &ProjectCandidate) -> Result<()> {
        candidate
            .protected_law_review(self.protection, self.laws)?
            .require(self.specification_approval)?;
        let report = candidate.strict_law_assurance(
            candidate.candidate_digest(),
            self.laws,
            self.policy,
            self.proofs,
        )?;
        candidate.require_strict_law_assurance(&report, self.laws, self.policy, self.proofs)
    }
}
impl ProjectCandidate {
    pub fn strict_law_assurance(
        &self,
        expected_candidate: &str,
        laws: &LawSet,
        policy: &StrictLawPolicy,
        proofs: &[VerifiedProjectProof],
    ) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        if self.base_revision().project_revision() != policy.base_revision() {
            return Err(vec![Diagnostic::io(
                "SPX-LW104",
                "strict law policy belongs to a different candidate base revision",
            )]);
        }
        let report = strict::derive(self.revision(), laws, policy, proofs)?;
        let report: serde_json::Value =
            serde_json::from_str(&report).expect("derived strict report is JSON");
        wire::render(
            serde_json::json!({"schema":STRICT_CANDIDATE_LAW_SCHEMA,"candidate_digest":self.candidate_digest(),"base_project_revision":self.base_revision().project_revision(),"law_report":report,"publication_authority":false}),
            crate::assurance_manifest::law_set::MAX_BYTES,
        )
    }
    pub fn require_strict_law_assurance(
        &self,
        document: &str,
        laws: &LawSet,
        policy: &StrictLawPolicy,
        proofs: &[VerifiedProjectProof],
    ) -> Result<()> {
        if document.len() > crate::assurance_manifest::law_set::MAX_BYTES {
            return Err(vec![Diagnostic::io(
                "SPX-LW102",
                "strict candidate law report exceeds its byte bound",
            )]);
        }
        let expected = self.strict_law_assurance(self.candidate_digest(), laws, policy, proofs)?;
        if expected != document {
            return Err(vec![Diagnostic::io(
                "SPX-LW104",
                "candidate law report failed exact independent replay",
            )]);
        }
        let report: serde_json::Value =
            serde_json::from_str(&expected).expect("derived strict report is JSON");
        if report["law_report"]["accepted"] != true {
            return Err(vec![Diagnostic::io(
                "SPX-LW130",
                "candidate strict law requirements are not satisfied",
            )]);
        }
        Ok(())
    }
}

pub const STRICT_LAW_PUBLICATION_SCHEMA: &str = "semaprax.strict-law-publication.v1";

/// Authority-free exact publication proposal, including the strict policy and
/// checked law report. It cannot select its own baseline, proofs or host rights.
pub struct StrictLawPublication {
    document: String,
}
impl StrictLawPublication {
    pub fn to_json(&self) -> &str {
        &self.document
    }
}

fn publication_document(
    candidate: &ProjectCandidate,
    inputs: &StrictCandidateLawInputs<'_>,
    publication: &str,
) -> Result<String> {
    let law_report = candidate.strict_law_assurance(
        candidate.candidate_digest(),
        inputs.laws,
        inputs.policy,
        inputs.proofs,
    )?;
    let intent = candidate.protected_law_review(inputs.protection, inputs.laws)?;
    wire::render(
        serde_json::json!({
            "schema":STRICT_LAW_PUBLICATION_SCHEMA,"candidate_digest":candidate.candidate_digest(),
            "protected_intent_review_digest":intent.digest(),"law_assurance":law_report,
            "publication":publication,"publication_authority":false
        }),
        publication::MAX_PROJECT_CANDIDATE_PUBLICATION_BYTES,
    )
}

/// Strict evidence is replayed after the ordinary host lock is acquired and
/// before any proposal is returned; the proposal still grants no authority.
pub fn prepare_strict_law_publication(
    candidate: &ProjectCandidate,
    inputs: &StrictCandidateLawInputs<'_>,
    approved_candidate_digest: &str,
    workspace_root: &Path,
    project_manifest: &Path,
    expected_workspace_revision: &str,
) -> Result<StrictLawPublication> {
    let publication = publication::prepare_with_law_gate(
        candidate,
        approved_candidate_digest,
        workspace_root,
        project_manifest,
        expected_workspace_revision,
        || inputs.require(candidate),
    )?;
    Ok(StrictLawPublication {
        document: publication_document(candidate, inputs, publication.to_json())?,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn apply_strict_law_publication(
    candidate: &ProjectCandidate,
    inputs: &StrictCandidateLawInputs<'_>,
    approved_candidate_digest: &str,
    workspace_root: &Path,
    project_manifest: &Path,
    expected_workspace_revision: &str,
    submitted_publication: &[u8],
) -> Result<String> {
    if submitted_publication.len() > publication::MAX_PROJECT_CANDIDATE_PUBLICATION_BYTES {
        return Err(vec![Diagnostic::io(
            "SPX-LW102",
            "strict publication proposal exceeds its byte bound",
        )]);
    }
    // Extract untrusted bytes only. The ordinary publication route replays its
    // own exact artifact, and the strict gate rederives the whole outer envelope
    // after authority acquisition and before staging.
    let submitted: serde_json::Value =
        serde_json::from_slice(submitted_publication).map_err(|_| {
            vec![Diagnostic::io(
                "SPX-LW104",
                "invalid strict publication proposal",
            )]
        })?;
    let publication = submitted["publication"].as_str().ok_or_else(|| {
        vec![Diagnostic::io(
            "SPX-LW104",
            "strict publication artifact missing",
        )]
    })?;
    publication::apply_with_law_gate(
        candidate,
        approved_candidate_digest,
        workspace_root,
        project_manifest,
        expected_workspace_revision,
        publication.as_bytes(),
        || {
            inputs.require(candidate)?;
            if publication_document(candidate, inputs, publication)?.as_bytes()
                != submitted_publication
            {
                return Err(vec![Diagnostic::io(
                    "SPX-LW104",
                    "strict publication policy, proof or intent association changed",
                )]);
            }
            Ok(())
        },
    )
}
