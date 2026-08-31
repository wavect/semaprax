//! Pure diagnostic and semantic report projections over detached subjects.
use super::*;
use crate::image_transport::candidates::reads::ReadSubjects;

pub(super) fn payload(
    action: Action,
    params: &Map<String, Value>,
    image: &ProjectSemanticImage,
    subjects: &ReadSubjects,
) -> Result<Value, Vec<Diagnostic>> {
    match action {
        Action::ProtocolConformance | Action::InterfaceCatalog => {
            let candidate = subjects.candidate.as_ref();
            let (schema, report_schema, report) = if matches!(action, Action::InterfaceCatalog) {
                let candidate = candidate
                    .ok_or_else(|| failure("SPX-G241", "interface catalog requires a candidate"))?;
                (
                    "semaprax.image-interface-catalog-chunk.v1",
                    "semaprax.project-interface-change-catalog.v1",
                    candidate
                        .interface_catalog(candidate.candidate_digest(), text(params, "target"))?,
                )
            } else {
                let revision = candidate
                    .map(|candidate| candidate.revision())
                    .unwrap_or(image.revision());
                let selected = ProjectSemanticImage::derive(
                    Arc::clone(revision),
                    revision.project_revision(),
                )?;
                (
                    "semaprax.image-protocol-conformance-chunk.v1",
                    crate::project::IMAGE_PROTOCOL_CONFORMANCE_SCHEMA,
                    selected.protocol_conformance(selected.image_digest())?,
                )
            };
            let (offset, end) = chunk(
                params,
                &report,
                "protocol report offset is outside canonical UTF-8 report",
            )?;
            Ok(json!({"schema":schema,"report_schema":report_schema,
                "image_revision":image.image_digest(),"candidate_revision":params.get("candidate_revision"),
                "target":params.get("target"),"offset":offset,"total_bytes":report.len(),
                "chunk":&report[offset..end],"next_offset":(end<report.len()).then_some(end),"source_authority":false}))
        }
        Action::Summary | Action::Query | Action::RepairCatalog => {
            let attempt = subjects.attempt.as_ref().ok_or_else(|| {
                failure("SPX-G243", "attempt handle is stale, discarded, or unknown")
            })?;
            match action {
                Action::Summary => parse_payload(attempt.summary(attempt.attempt_digest())?),
                Action::RepairCatalog => {
                    parse_payload(attempt.repair_catalog(attempt.attempt_digest())?)
                }
                _ => {
                    let report = attempt.to_json();
                    let (offset, end) = chunk(
                        params,
                        report,
                        "attempt query offset is outside canonical UTF-8 report",
                    )?;
                    Ok(
                        json!({"schema":"semaprax.image-attempt-report-chunk.v1","attempt_revision":attempt.attempt_digest(),
                        "report_schema":crate::project::PROJECT_CANDIDATE_ATTEMPT_SCHEMA,"offset":offset,"total_bytes":report.len(),
                        "chunk":&report[offset..end],"next_offset":(end<report.len()).then_some(end),"materializable":false,"source_authority":false}),
                    )
                }
            }
        }
        Action::Delta | Action::DeltaCatalog => {
            let candidate = subjects.candidate.as_ref().ok_or_else(|| {
                failure(
                    "SPX-G224",
                    "candidate handle is stale, discarded, or unknown",
                )
            })?;
            let (schema, report) = if matches!(action, Action::Delta) {
                (
                    "semaprax.project-candidate-semantic-delta.v1",
                    candidate
                        .semantic_delta(candidate.candidate_digest(), text(params, "target"))?,
                )
            } else {
                (
                    "semaprax.project-candidate-semantic-delta-catalog.v1",
                    candidate.semantic_delta_catalog(candidate.candidate_digest())?,
                )
            };
            let (offset, end) = chunk(
                params,
                &report,
                "semantic delta offset is outside canonical UTF-8 report",
            )?;
            Ok(
                json!({"schema":"semaprax.image-semantic-delta-chunk.v1","candidate_revision":candidate.candidate_digest(),
                "target":params.get("target"),"report_schema":schema,"offset":offset,"total_bytes":report.len(),
                "chunk":&report[offset..end],"next_offset":(end<report.len()).then_some(end),"source_authority":false}),
            )
        }
        _ => Err(failure(
            "SPX-G241",
            "diagnostic operation is not an immutable report",
        )),
    }
}

fn chunk(
    params: &Map<String, Value>,
    report: &str,
    message: &'static str,
) -> Result<(usize, usize), Vec<Diagnostic>> {
    let offset = number(params, "offset", 0);
    if offset > report.len() || !report.is_char_boundary(offset) {
        return Err(failure("SPX-G241", message));
    }
    let mut end = offset
        .saturating_add(number(params, "chunk_bytes", 16_384))
        .min(report.len());
    while !report.is_char_boundary(end) {
        end -= 1;
    }
    if end == offset && offset < report.len() {
        return Err(failure(
            "SPX-G241",
            "chunk_bytes cannot hold the next UTF-8 character",
        ));
    }
    Ok((offset, end))
}
