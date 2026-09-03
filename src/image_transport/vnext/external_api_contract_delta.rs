//! Chunked descriptive delta over exact base and candidate external API
//! declarations. Declaration bytes grant no observation or authority.

use super::*;
use crate::project::{
    MAX_PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_DECLARATION_BYTES,
    MAX_PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_DELTA_BYTES,
    PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_DELTA_SCHEMA,
};

pub(super) const CHUNK_SCHEMA: &str =
    "semaprax.image-candidate-external-api-contract-delta-chunk.v1";

const METHOD: Method = Method {
    name: "candidate/external-api-contract-delta",
    operation: Operation::VNext(Action::CandidateExternalApiContractDelta),
    parameters: &[
        REVISION,
        Parameter {
            name: "candidate_revision",
            kind: ParameterKind::Digest,
            required: true,
        },
        Parameter {
            name: "base_declaration",
            kind: ParameterKind::CanonicalJsonText(
                MAX_PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_DECLARATION_BYTES,
            ),
            required: true,
        },
        Parameter {
            name: "base_declaration_digest",
            kind: ParameterKind::Digest,
            required: true,
        },
        Parameter {
            name: "candidate_declaration",
            kind: ParameterKind::CanonicalJsonText(
                MAX_PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_DECLARATION_BYTES,
            ),
            required: true,
        },
        Parameter {
            name: "candidate_declaration_digest",
            kind: ParameterKind::Digest,
            required: true,
        },
        Parameter {
            name: "offset",
            kind: ParameterKind::Integer(
                0,
                MAX_PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_DELTA_BYTES,
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
            "candidate external API contract delta image revision is stale",
        ));
    }
    let candidate = registry.candidate(text(params, "candidate_revision"))?;
    let report = candidate.external_api_contract_delta(
        candidate.candidate_digest(),
        text(params, "base_declaration").as_bytes(),
        text(params, "base_declaration_digest"),
        text(params, "candidate_declaration").as_bytes(),
        text(params, "candidate_declaration_digest"),
    )?;
    if report.len() > MAX_PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_DELTA_BYTES {
        return Err(failure(
            "SPX-G458",
            "candidate external API contract delta exceeds its transport byte bound",
        ));
    }
    let offset = number(params, "offset", 0);
    let chunk_bytes = number(params, "chunk_bytes", 16384);
    if !(1024..=65536).contains(&chunk_bytes)
        || offset > report.len()
        || !report.is_char_boundary(offset)
    {
        return Err(failure(
            "SPX-G457",
            "candidate external API contract delta chunk is outside its bounded UTF-8 report",
        ));
    }
    let mut end = offset.saturating_add(chunk_bytes).min(report.len());
    while !report.is_char_boundary(end) {
        end -= 1;
    }
    Ok(json!({
        "schema":CHUNK_SCHEMA,
        "report_schema":PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_DELTA_SCHEMA,
        "image_revision":image.image_digest(),
        "candidate_revision":candidate.candidate_digest(),
        "base_declaration_digest":text(params,"base_declaration_digest"),
        "candidate_declaration_digest":text(params,"candidate_declaration_digest"),
        "offset":offset,
        "total_bytes":report.len(),
        "chunk":&report[offset..end],
        "next_offset":(end<report.len()).then_some(end),
        "report_sha256":report_sha256(&report),
        "compatibility":"not_assessed",
        "comparison_scope":"caller_declared_export_identity_operation_digest_and_schema_digest_inventory_only",
        "source_authority":false,
        "external_io":false,
        "filesystem_authority":false,
        "process_authority":false,
        "network_observation":false,
        "network_authority":false,
        "provider_observation":false,
        "runtime_observation":false,
        "version_evidence":false,
        "conformance_evidence":false,
        "consumer_evidence":false,
        "ambient_authority":false,
        "publication_authority":false,
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
