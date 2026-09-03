//! Chunked environment and attached package-consumer review for one exact
//! candidate. Caller bundle bytes and the host-attached graph keep this method
//! outside the immutable parallel-read subset and grant no authority.

use super::*;
use crate::package_lock_v2::Coordinate;
use crate::package_semantic_graph::PackageSemanticGraph;
use crate::project::{
    MAX_PROJECT_CANDIDATE_ANALYSIS_BOUNDARY_BUNDLE_BYTES,
    MAX_PROJECT_CANDIDATE_ENVIRONMENT_CONSUMER_REVIEW_BYTES,
    PROJECT_CANDIDATE_ENVIRONMENT_CONSUMER_REVIEW_SCHEMA,
};

pub(super) const CHUNK_SCHEMA: &str =
    "semaprax.image-candidate-environment-consumer-review-chunk.v1";

const METHOD: Method = Method {
    name: "candidate/environment-consumer-review",
    operation: Operation::VNext(Action::CandidateEnvironmentConsumerReview),
    parameters: &[
        REVISION,
        Parameter {
            name: "candidate_revision",
            kind: ParameterKind::Digest,
            required: true,
        },
        Parameter {
            name: "package_revision",
            kind: ParameterKind::Digest,
            required: true,
        },
        Parameter {
            name: "bundle",
            kind: ParameterKind::CanonicalJsonText(
                MAX_PROJECT_CANDIDATE_ANALYSIS_BOUNDARY_BUNDLE_BYTES,
            ),
            required: true,
        },
        Parameter {
            name: "bundle_digest",
            kind: ParameterKind::Digest,
            required: true,
        },
        Parameter {
            name: "provider_package",
            kind: ParameterKind::Text(255),
            required: true,
        },
        Parameter {
            name: "provider_version",
            kind: ParameterKind::Text(128),
            required: true,
        },
        Parameter {
            name: "provider_source_path",
            kind: ParameterKind::Text(crate::project::MAX_PATH_BYTES),
            required: true,
        },
        TARGET,
        Parameter {
            name: "offset",
            kind: ParameterKind::Integer(
                0,
                MAX_PROJECT_CANDIDATE_ENVIRONMENT_CONSUMER_REVIEW_BYTES,
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
    graph: Option<&PackageSemanticGraph>,
) -> Result<Value, Vec<Diagnostic>> {
    if text(params, "image_revision") != image.image_digest() {
        return Err(failure(
            "SPX-G282",
            "candidate environment consumer review image revision is stale",
        ));
    }
    let graph = graph.ok_or_else(|| {
        failure(
            "SPX-G280",
            "candidate environment consumer review requires a host-attached package graph",
        )
    })?;
    if text(params, "package_revision") != graph.graph_digest() {
        return Err(failure(
            "SPX-G475",
            "candidate environment consumer review package revision is stale",
        ));
    }
    let candidate = registry.candidate(text(params, "candidate_revision"))?;
    let provider = Coordinate {
        package: text(params, "provider_package").to_owned(),
        version: text(params, "provider_version").to_owned(),
    };
    let report = candidate.environment_consumer_review(
        candidate.candidate_digest(),
        text(params, "bundle").as_bytes(),
        text(params, "bundle_digest"),
        graph,
        &provider,
        text(params, "provider_source_path"),
        text(params, "target"),
    )?;
    if report.len() > MAX_PROJECT_CANDIDATE_ENVIRONMENT_CONSUMER_REVIEW_BYTES {
        return Err(failure(
            "SPX-G476",
            "candidate environment consumer review exceeds its transport byte bound",
        ));
    }
    let offset = number(params, "offset", 0);
    let chunk_bytes = number(params, "chunk_bytes", 16384);
    if !(1024..=65536).contains(&chunk_bytes)
        || offset > report.len()
        || !report.is_char_boundary(offset)
    {
        return Err(failure(
            "SPX-G476",
            "candidate environment consumer review chunk is outside its bounded UTF-8 report",
        ));
    }
    let mut end = offset.saturating_add(chunk_bytes).min(report.len());
    while !report.is_char_boundary(end) {
        end -= 1;
    }
    Ok(json!({
        "schema":CHUNK_SCHEMA,
        "report_schema":PROJECT_CANDIDATE_ENVIRONMENT_CONSUMER_REVIEW_SCHEMA,
        "image_revision":image.image_digest(),
        "candidate_revision":candidate.candidate_digest(),
        "package_revision":graph.graph_digest(),
        "bundle_digest":text(params,"bundle_digest"),
        "provider":{"package":provider.package,"version":provider.version},
        "provider_source_path":text(params,"provider_source_path"),
        "target":text(params,"target"),
        "offset":offset,
        "total_bytes":report.len(),
        "chunk":&report[offset..end],
        "next_offset":(end<report.len()).then_some(end),
        "report_sha256":report_sha256(&report),
        "compatibility":"not_assessed",
        "source_authority":false,
        "approval_authority":false,
        "publication_authority":false,
        "external_io":false,
        "filesystem_observation":false,
        "filesystem_authority":false,
        "environment_observation":false,
        "network_observation":false,
        "registry_observation":false,
        "installed_consumer_observation":false,
        "consumer_discovery_complete":false,
        "package_acquisition_authority":false,
        "execution":false,
        "runtime_observation":false,
        "runtime_authority":false,
        "provider_observation":false,
        "provider_authority":false,
        "generator_execution":false,
        "generator_authority":false,
        "conformance_evidence":false,
        "conformance_authority":false,
        "deployment_authority":false,
        "candidate_retained":false,
        "graph_retained":false,
    }))
}

fn report_sha256(report: &str) -> String {
    use sha2::{Digest, Sha256};
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(report.as_bytes()))
    )
}
