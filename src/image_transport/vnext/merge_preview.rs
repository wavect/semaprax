//! Bidirectional checked merge preview over two immutable retained candidates.
use super::*;
use crate::project::ProjectCandidate;

const METHOD: Method = Method {
    name: "candidate/merge-preview",
    operation: Operation::VNext(Action::CandidateMergePreview),
    parameters: &[
        REVISION,
        Parameter {
            name: "candidate_revision",
            kind: ParameterKind::Digest,
            required: true,
        },
        Parameter {
            name: "other_candidate_revision",
            kind: ParameterKind::Digest,
            required: true,
        },
    ],
    query: true,
    payload_schema: "semaprax.project-candidate-merge-preview.v1",
};

pub(super) fn method() -> &'static Method {
    &METHOD
}

pub(super) fn prepare(
    params: &Map<String, Value>,
    image: &ProjectSemanticImage,
    registry: &candidates::Registry,
) -> Result<Value, Vec<Diagnostic>> {
    if text(params, "image_revision") != image.image_digest() {
        return Err(failure("SPX-G282", "v5 expected image revision is stale"));
    }
    let candidate = registry.candidate(text(params, "candidate_revision"))?;
    let other = registry.candidate(text(params, "other_candidate_revision"))?;
    for_candidates(params, candidate, other)
}

pub(super) fn for_candidates(
    params: &Map<String, Value>,
    candidate: &ProjectCandidate,
    other: &ProjectCandidate,
) -> Result<Value, Vec<Diagnostic>> {
    let report = candidate.merge_preview(
        text(params, "candidate_revision"),
        other,
        text(params, "other_candidate_revision"),
    )?;
    serde_json::from_str(&report)
        .map_err(|_| failure("SPX-G222", "retained merge preview is not valid JSON"))
}
