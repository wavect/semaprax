//! Read-only stable-ID cleanup dependencies over one immutable source image.
use super::*;
use crate::project::{
    IMAGE_CLEANUP_DEPENDENCIES_SCHEMA, MAX_IMAGE_CLEANUP_DEPENDENCIES_BYTES,
    MAX_PROJECT_CANDIDATE_CLEANUP_DEPENDENCIES_BYTES,
    PROJECT_CANDIDATE_CLEANUP_DEPENDENCIES_SCHEMA,
};

const METHOD: Method = Method {
    name: "image/cleanup-dependencies",
    operation: Operation::VNext(Action::CleanupDependencies),
    parameters: &[
        REVISION,
        TARGET,
        Parameter {
            name: "offset",
            kind: ParameterKind::Integer(0, MAX_IMAGE_CLEANUP_DEPENDENCIES_BYTES),
            required: false,
        },
        Parameter {
            name: "chunk_bytes",
            kind: ParameterKind::Integer(1024, 65536),
            required: false,
        },
    ],
    query: true,
    payload_schema: "semaprax.image-cleanup-dependencies-chunk.v1",
};

pub(super) fn method() -> &'static Method {
    &METHOD
}

const CANDIDATE_METHOD: Method = Method {
    name: "candidate/cleanup-dependencies",
    operation: Operation::VNext(Action::CandidateCleanupDependencies),
    parameters: &[
        REVISION,
        Parameter {
            name: "candidate_revision",
            kind: ParameterKind::Digest,
            required: true,
        },
        TARGET,
        Parameter {
            name: "offset",
            kind: ParameterKind::Integer(0, MAX_PROJECT_CANDIDATE_CLEANUP_DEPENDENCIES_BYTES),
            required: false,
        },
        Parameter {
            name: "chunk_bytes",
            kind: ParameterKind::Integer(1024, 65536),
            required: false,
        },
    ],
    query: true,
    payload_schema: "semaprax.image-candidate-cleanup-dependencies-chunk.v1",
};

pub(super) fn candidate_method() -> &'static Method {
    &CANDIDATE_METHOD
}

pub(super) fn prepare_candidate(
    params: &Map<String, Value>,
    image: &ProjectSemanticImage,
    registry: &candidates::Registry,
) -> Result<Value, Vec<Diagnostic>> {
    if text(params, "image_revision") != image.image_digest() {
        return Err(failure(
            "SPX-G221",
            "candidate cleanup dependency image revision is stale",
        ));
    }
    let candidate = registry.candidate(text(params, "candidate_revision"))?;
    for_candidate(params, image, candidate)
}

pub(super) fn for_candidate(
    params: &Map<String, Value>,
    image: &ProjectSemanticImage,
    candidate: &crate::project::ProjectCandidate,
) -> Result<Value, Vec<Diagnostic>> {
    let target = text(params, "target");
    let report = candidate.cleanup_dependencies(candidate.candidate_digest(), target)?;
    if report.len() > MAX_PROJECT_CANDIDATE_CLEANUP_DEPENDENCIES_BYTES {
        return Err(failure(
            "SPX-G220",
            "candidate cleanup dependency report exceeds its transport byte bound",
        ));
    }
    let offset = number(params, "offset", 0);
    let chunk_bytes = number(params, "chunk_bytes", 16384);
    if !(1024..=65536).contains(&chunk_bytes)
        || offset > report.len()
        || !report.is_char_boundary(offset)
    {
        return Err(failure(
            "SPX-G219",
            "candidate cleanup dependency chunk is outside its bounded UTF8 report",
        ));
    }
    let mut end = offset.saturating_add(chunk_bytes).min(report.len());
    while !report.is_char_boundary(end) {
        end -= 1;
    }
    Ok(json!({
        "schema":"semaprax.image-candidate-cleanup-dependencies-chunk.v1",
        "report_schema":PROJECT_CANDIDATE_CLEANUP_DEPENDENCIES_SCHEMA,
        "image_revision":image.image_digest(),"candidate_revision":candidate.candidate_digest(),
        "target":target,"offset":offset,"total_bytes":report.len(),"chunk":&report[offset..end],
        "next_offset":(end<report.len()).then_some(end),"source_authority":false,
    }))
}

pub(super) fn prepare(
    params: &Map<String, Value>,
    image: &ProjectSemanticImage,
) -> Result<Value, Vec<Diagnostic>> {
    if text(params, "image_revision") != image.image_digest() {
        return Err(failure(
            "SPX-G221",
            "cleanup dependency image revision is stale",
        ));
    }
    let target = text(params, "target");
    let report = image.cleanup_dependencies(image.image_digest(), target)?;
    if report.len() > MAX_IMAGE_CLEANUP_DEPENDENCIES_BYTES {
        return Err(failure(
            "SPX-G220",
            "cleanup dependency report exceeds its transport byte bound",
        ));
    }
    let offset = number(params, "offset", 0);
    let chunk_bytes = number(params, "chunk_bytes", 16384);
    if !(1024..=65536).contains(&chunk_bytes)
        || offset > report.len()
        || !report.is_char_boundary(offset)
    {
        return Err(failure(
            "SPX-G219",
            "cleanup dependency chunk is outside its bounded UTF8 report",
        ));
    }
    let mut end = offset.saturating_add(chunk_bytes).min(report.len());
    while !report.is_char_boundary(end) {
        end -= 1;
    }
    Ok(json!({
        "schema":"semaprax.image-cleanup-dependencies-chunk.v1",
        "report_schema":IMAGE_CLEANUP_DEPENDENCIES_SCHEMA,
        "image_revision":image.image_digest(),"target":target,
        "offset":offset,"total_bytes":report.len(),"chunk":&report[offset..end],
        "next_offset":(end<report.len()).then_some(end),"source_authority":false,
    }))
}
