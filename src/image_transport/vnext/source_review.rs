//! Closed candidate source review through the existing immutable read boundary.
use super::*;
use crate::project::{
    ProjectCandidate, MAX_PROJECT_CANDIDATE_SOURCE_REVIEW_BYTES,
    PROJECT_CANDIDATE_SOURCE_REVIEW_SCHEMA,
};

const METHOD: Method = Method {
    name: "candidate/source-review",
    operation: Operation::VNext(Action::SourceReview),
    parameters: &[
        REVISION,
        Parameter {
            name: "candidate_revision",
            kind: ParameterKind::Digest,
            required: true,
        },
        Parameter {
            name: "offset",
            kind: ParameterKind::Integer(0, MAX_PROJECT_CANDIDATE_SOURCE_REVIEW_BYTES),
            required: false,
        },
        Parameter {
            name: "chunk_bytes",
            kind: ParameterKind::Integer(1024, 65536),
            required: false,
        },
    ],
    query: true,
    payload_schema: "semaprax.image-source-review-chunk.v1",
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
        return Err(failure("SPX-G221", "source review image revision is stale"));
    }
    let candidate = registry.candidate(text(params, "candidate_revision"))?;
    for_candidate(params, image, candidate)
}

pub(super) fn for_candidate(
    params: &Map<String, Value>,
    image: &ProjectSemanticImage,
    candidate: &ProjectCandidate,
) -> Result<Value, Vec<Diagnostic>> {
    let report = candidate.source_review_shared(text(params, "candidate_revision"))?;
    let offset = number(params, "offset", 0);
    if offset > report.len() || !report.is_char_boundary(offset) {
        return Err(failure(
            "SPX-G222",
            "source review offset is outside its canonical UTF-8 report",
        ));
    }
    let mut end = offset
        .saturating_add(number(params, "chunk_bytes", 16384))
        .min(report.len());
    while !report.is_char_boundary(end) {
        end -= 1;
    }
    Ok(json!({
        "schema":"semaprax.image-source-review-chunk.v1",
        "report_schema":PROJECT_CANDIDATE_SOURCE_REVIEW_SCHEMA,
        "image_revision":image.image_digest(),
        "candidate_revision":candidate.candidate_digest(),
        "offset":offset,"total_bytes":report.len(),"chunk":&report[offset..end],
        "next_offset":(end<report.len()).then_some(end),
        "source_authority":false,
    }))
}
