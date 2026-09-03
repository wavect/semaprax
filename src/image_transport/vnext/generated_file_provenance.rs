//! Exact caller-declared generated-file provenance over one retained candidate.
//! Declaration bytes carry no path access, generator execution, materialization,
//! runtime observation, or deployment authority.

use super::*;
use crate::project::{
    MAX_PROJECT_CANDIDATE_GENERATED_FILE_PROVENANCE_DECLARATION_BYTES,
    MAX_PROJECT_CANDIDATE_GENERATED_FILE_PROVENANCE_EVIDENCE_BYTES,
    PROJECT_CANDIDATE_GENERATED_FILE_PROVENANCE_EVIDENCE_SCHEMA,
};

pub(super) const CHUNK_SCHEMA: &str =
    "semaprax.image-candidate-generated-file-provenance-evidence-chunk.v1";

const METHOD: Method = Method {
    name: "candidate/analysis-generated-file-provenance-evidence",
    operation: Operation::VNext(Action::CandidateGeneratedFileProvenanceEvidence),
    parameters: &[
        REVISION,
        Parameter {
            name: "candidate_revision",
            kind: ParameterKind::Digest,
            required: true,
        },
        Parameter {
            name: "declaration",
            kind: ParameterKind::CanonicalJsonText(
                MAX_PROJECT_CANDIDATE_GENERATED_FILE_PROVENANCE_DECLARATION_BYTES,
            ),
            required: true,
        },
        Parameter {
            name: "declaration_digest",
            kind: ParameterKind::Digest,
            required: true,
        },
        Parameter {
            name: "offset",
            kind: ParameterKind::Integer(
                0,
                MAX_PROJECT_CANDIDATE_GENERATED_FILE_PROVENANCE_EVIDENCE_BYTES,
            ),
            required: false,
        },
        Parameter {
            name: "chunk_bytes",
            kind: ParameterKind::Integer(1024, 65536),
            required: false,
        },
    ],
    query: true,
    payload_schema: CHUNK_SCHEMA,
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
        return Err(failure(
            "SPX-G282",
            "candidate generated-file provenance image revision is stale",
        ));
    }
    let candidate = registry.candidate(text(params, "candidate_revision"))?;
    let report = candidate.analysis_generated_file_provenance_evidence(
        candidate.candidate_digest(),
        text(params, "declaration").as_bytes(),
        text(params, "declaration_digest"),
    )?;
    if report.len() > MAX_PROJECT_CANDIDATE_GENERATED_FILE_PROVENANCE_EVIDENCE_BYTES {
        return Err(failure(
            "SPX-G437",
            "candidate generated-file provenance evidence exceeds its transport byte bound",
        ));
    }
    let offset = number(params, "offset", 0);
    let chunk_bytes = number(params, "chunk_bytes", 16384);
    if !(1024..=65536).contains(&chunk_bytes)
        || offset > report.len()
        || !report.is_char_boundary(offset)
    {
        return Err(failure(
            "SPX-G436",
            "candidate generated-file provenance chunk is outside its bounded UTF-8 report",
        ));
    }
    let mut end = offset.saturating_add(chunk_bytes).min(report.len());
    while !report.is_char_boundary(end) {
        end -= 1;
    }
    if end == offset && offset < report.len() {
        return Err(failure(
            "SPX-G437",
            "candidate generated-file provenance chunk cannot make progress",
        ));
    }
    Ok(json!({
        "schema":CHUNK_SCHEMA,
        "report_schema":PROJECT_CANDIDATE_GENERATED_FILE_PROVENANCE_EVIDENCE_SCHEMA,
        "image_revision":image.image_digest(),
        "candidate_revision":candidate.candidate_digest(),
        "declaration_digest":text(params,"declaration_digest"),
        "offset":offset,
        "total_bytes":report.len(),
        "chunk":&report[offset..end],
        "next_offset":(end<report.len()).then_some(end),
        "report_sha256":report_sha256(&report),
        "source_authority":false,
        "filesystem_scan":false,
        "generator_execution":false,
        "artifact_materialization":false,
        "runtime_observation":false,
        "deployment_authority":false,
    }))
}

fn report_sha256(report: &str) -> String {
    use sha2::{Digest, Sha256};
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(report.as_bytes()))
    )
}
