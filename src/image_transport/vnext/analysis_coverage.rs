//! Retained analysis boundaries: a pure image report, never external discovery.
use super::*;
use crate::project::{
    ProjectCandidate, IMAGE_ANALYSIS_COVERAGE_SCHEMA, MAX_IMAGE_ANALYSIS_COVERAGE_BYTES,
    MAX_PROJECT_CANDIDATE_ANALYSIS_COVERAGE_BYTES, PROJECT_CANDIDATE_ANALYSIS_COVERAGE_SCHEMA,
};

const METHOD: Method = Method {
    name: "image/analysis-coverage",
    operation: Operation::VNext(Action::AnalysisCoverage),
    parameters: &[REVISION],
    query: true,
    payload_schema: IMAGE_ANALYSIS_COVERAGE_SCHEMA,
};

const CANDIDATE_METHOD: Method = Method {
    name: "candidate/analysis-coverage",
    operation: Operation::VNext(Action::CandidateAnalysisCoverage),
    parameters: &[
        REVISION,
        Parameter {
            name: "candidate_revision",
            kind: ParameterKind::Digest,
            required: true,
        },
    ],
    query: true,
    payload_schema: PROJECT_CANDIDATE_ANALYSIS_COVERAGE_SCHEMA,
};

pub(super) fn method() -> &'static Method {
    &METHOD
}

pub(super) fn candidate_method() -> &'static Method {
    &CANDIDATE_METHOD
}

pub(super) fn prepare(
    params: &Map<String, Value>,
    image: &ProjectSemanticImage,
) -> Result<Value, Vec<Diagnostic>> {
    let report = image.analysis_coverage(text(params, "image_revision"))?;
    if report.len() > MAX_IMAGE_ANALYSIS_COVERAGE_BYTES {
        return Err(failure(
            "SPX-G220",
            "analysis coverage report exceeds its transport byte bound",
        ));
    }
    serde_json::from_str(&report)
        .map_err(|_| failure("SPX-G219", "analysis coverage report is not compiler JSON"))
}

pub(super) fn prepare_candidate(
    params: &Map<String, Value>,
    image: &ProjectSemanticImage,
    registry: &candidates::Registry,
) -> Result<Value, Vec<Diagnostic>> {
    if text(params, "image_revision") != image.image_digest() {
        return Err(failure("SPX-G282", "v5 expected image revision is stale"));
    }
    for_candidate(
        params,
        registry.candidate(text(params, "candidate_revision"))?,
    )
}

pub(super) fn for_candidate(
    params: &Map<String, Value>,
    candidate: &ProjectCandidate,
) -> Result<Value, Vec<Diagnostic>> {
    let report = candidate.analysis_coverage(text(params, "candidate_revision"))?;
    if report.len() > MAX_PROJECT_CANDIDATE_ANALYSIS_COVERAGE_BYTES {
        return Err(failure(
            "SPX-G220",
            "candidate analysis coverage report exceeds its transport byte bound",
        ));
    }
    serde_json::from_str(&report).map_err(|_| {
        failure(
            "SPX-G219",
            "candidate analysis coverage report is not compiler JSON",
        )
    })
}
