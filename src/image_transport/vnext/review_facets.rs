//! Read-only candidate review facets selected by the startup host policy.
use super::*;

const CANDIDATE_REVISION: Parameter = Parameter {
    name: "candidate_revision",
    kind: ParameterKind::Digest,
    required: true,
};
const OFFSET: Parameter = Parameter {
    name: "offset",
    kind: ParameterKind::Integer(0, 8 * 1024 * 1024),
    required: false,
};
const CHUNK: Parameter = Parameter {
    name: "chunk_bytes",
    kind: ParameterKind::Integer(1024, 65536),
    required: false,
};
const INTERFACE_DELTA: Method = Method {
    name: "candidate/interface-delta",
    operation: Operation::VNext(Action::InterfaceDelta),
    parameters: &[REVISION, CANDIDATE_REVISION, OFFSET, CHUNK],
    query: true,
    payload_schema: "semaprax.image-interface-delta-chunk.v1",
};
const CONTRACT_DELTA: Method = Method {
    name: "candidate/contract-delta",
    operation: Operation::VNext(Action::ContractDelta),
    parameters: &[REVISION, CANDIDATE_REVISION, OFFSET, CHUNK],
    query: true,
    payload_schema: "semaprax.image-contract-delta-chunk.v1",
};
const OWNERSHIP_DELTA: Method = Method {
    name: "candidate/ownership-delta",
    operation: Operation::VNext(Action::OwnershipDelta),
    parameters: &[REVISION, CANDIDATE_REVISION, OFFSET, CHUNK],
    query: true,
    payload_schema: "semaprax.image-ownership-delta-chunk.v1",
};
const SYMBOL_DIAGNOSTICS: Method = Method {
    name: "candidate/symbol-diagnostics",
    operation: Operation::VNext(Action::SymbolDiagnostics),
    parameters: &[
        REVISION,
        CANDIDATE_REVISION,
        TARGET,
        OFFSET,
        CHUNK,
        Parameter {
            name: "expected_report_revision",
            kind: ParameterKind::Digest,
            required: false,
        },
    ],
    query: true,
    payload_schema: "semaprax.image-symbol-diagnostics-chunk.v1",
};

pub(super) fn methods(policy: &VNextPolicy) -> Vec<&'static Method> {
    let mut result = Vec::new();
    if policy.candidate_prepare {
        result.push(&INTERFACE_DELTA);
        result.push(&CONTRACT_DELTA);
        result.push(&OWNERSHIP_DELTA);
    }
    if policy.diagnostics {
        result.push(&SYMBOL_DIAGNOSTICS);
    }
    result
}

pub(super) fn ownership_delta(
    params: &Map<String, Value>,
    image: &ProjectSemanticImage,
    registry: &candidates::Registry,
) -> Result<Value, Vec<Diagnostic>> {
    if text(params, "image_revision") != image.image_digest() {
        return Err(failure(
            "SPX-G221",
            "ownership delta image revision is stale",
        ));
    }
    let candidate = registry.candidate(text(params, "candidate_revision"))?;
    ownership_delta_for_candidate(params, image, candidate)
}

pub(super) fn ownership_delta_for_candidate(
    params: &Map<String, Value>,
    image: &ProjectSemanticImage,
    candidate: &crate::project::ProjectCandidate,
) -> Result<Value, Vec<Diagnostic>> {
    let report = candidate.ownership_delta(candidate.candidate_digest())?;
    let offset = number(params, "offset", 0);
    if offset > report.len() || !report.is_char_boundary(offset) {
        return Err(failure(
            "SPX-G328",
            "ownership delta offset is outside its UTF-8 report",
        ));
    }
    let mut end = offset
        .saturating_add(number(params, "chunk_bytes", 16384))
        .min(report.len());
    while !report.is_char_boundary(end) {
        end -= 1;
    }
    Ok(json!({
        "schema":"semaprax.image-ownership-delta-chunk.v1",
        "report_schema":"semaprax.project-candidate-ownership-delta.v1",
        "image_revision":image.image_digest(),
        "candidate_revision":candidate.candidate_digest(),
        "offset":offset,"total_bytes":report.len(),"chunk":&report[offset..end],
        "next_offset":(end<report.len()).then_some(end),
        "source_authority":false
    }))
}

pub(super) fn contract_delta(
    params: &Map<String, Value>,
    image: &ProjectSemanticImage,
    registry: &candidates::Registry,
) -> Result<Value, Vec<Diagnostic>> {
    if text(params, "image_revision") != image.image_digest() {
        return Err(failure(
            "SPX-G221",
            "contract delta image revision is stale",
        ));
    }
    let candidate = registry.candidate(text(params, "candidate_revision"))?;
    contract_delta_for_candidate(params, image, candidate)
}

pub(super) fn contract_delta_for_candidate(
    params: &Map<String, Value>,
    image: &ProjectSemanticImage,
    candidate: &crate::project::ProjectCandidate,
) -> Result<Value, Vec<Diagnostic>> {
    let report = candidate.contract_delta(candidate.candidate_digest())?;
    let offset = number(params, "offset", 0);
    if offset > report.len() || !report.is_char_boundary(offset) {
        return Err(failure(
            "SPX-G325",
            "contract delta offset is outside its UTF-8 report",
        ));
    }
    let mut end = offset
        .saturating_add(number(params, "chunk_bytes", 16384))
        .min(report.len());
    while !report.is_char_boundary(end) {
        end -= 1;
    }
    Ok(json!({
        "schema":"semaprax.image-contract-delta-chunk.v1",
        "report_schema":"semaprax.project-candidate-contract-delta.v1",
        "image_revision":image.image_digest(),
        "candidate_revision":candidate.candidate_digest(),
        "offset":offset,"total_bytes":report.len(),"chunk":&report[offset..end],
        "next_offset":(end<report.len()).then_some(end),
        "source_authority":false
    }))
}

pub(super) fn interface_delta(
    params: &Map<String, Value>,
    image: &ProjectSemanticImage,
    registry: &candidates::Registry,
) -> Result<Value, Vec<Diagnostic>> {
    if text(params, "image_revision") != image.image_digest() {
        return Err(failure(
            "SPX-G221",
            "interface delta image revision is stale",
        ));
    }
    let candidate = registry.candidate(text(params, "candidate_revision"))?;
    interface_delta_for_candidate(params, image, candidate)
}

pub(super) fn interface_delta_for_candidate(
    params: &Map<String, Value>,
    image: &ProjectSemanticImage,
    candidate: &crate::project::ProjectCandidate,
) -> Result<Value, Vec<Diagnostic>> {
    let report = candidate.interface_delta(candidate.candidate_digest())?;
    let offset = number(params, "offset", 0);
    if offset > report.len() || !report.is_char_boundary(offset) {
        return Err(failure(
            "SPX-G310",
            "interface delta offset is outside its UTF-8 report",
        ));
    }
    let mut end = offset
        .saturating_add(number(params, "chunk_bytes", 16384))
        .min(report.len());
    while !report.is_char_boundary(end) {
        end -= 1;
    }
    Ok(json!({
        "schema":"semaprax.image-interface-delta-chunk.v1",
        "report_schema":"semaprax.project-candidate-interface-delta.v1",
        "image_revision":image.image_digest(),
        "candidate_revision":candidate.candidate_digest(),
        "offset":offset,"total_bytes":report.len(),"chunk":&report[offset..end],
        "next_offset":(end<report.len()).then_some(end),
        "source_authority":false
    }))
}
