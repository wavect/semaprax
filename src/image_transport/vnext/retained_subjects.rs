//! Compact references to the live candidate-session registry.
use super::*;

const METHOD: Method = Method {
    name: "workspace/retained-subjects",
    operation: Operation::VNext(Action::RetainedSubjects),
    parameters: &[REVISION],
    query: true,
    payload_schema: "semaprax.image-retained-subjects.v1",
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
    let snapshot = registry.retained_subject_snapshot()?;
    let value = json!({
        "schema":"semaprax.image-retained-subjects.v1",
        "image_revision":image.image_digest(),
        "candidates":snapshot.candidates.into_iter().map(|row| json!({
            "candidate_revision":row.candidate_revision,
            "base_project_revision":row.base_project_revision,
            "project_revision":row.project_revision,
            "retained_report_bytes":row.retained_report_bytes,
            "has_retained_drafts":row.has_retained_drafts,
            "has_retained_attempts":row.has_retained_attempts,
            "detail_method":"candidate/query",
            "discard_method":"candidate/discard",
        })).collect::<Vec<_>>(),
        "drafts":snapshot.drafts.into_iter().map(|row| json!({
            "draft_revision":row.draft_revision,
            "source_candidate_revision":row.source_candidate_revision,
            "source_candidate_retained":row.source_candidate_retained,
            "state":row.state,
            "unresolved_hole_count":row.unresolved_hole_count,
            "retained_report_bytes":row.retained_report_bytes,
            "detail_method":"hole/recovery-export",
            "discard_method":"hole/discard",
        })).collect::<Vec<_>>(),
        "attempts":snapshot.attempts.into_iter().map(|row| json!({
            "attempt_revision":row.attempt_revision,
            "base_candidate_revision":row.base_candidate_revision,
            "base_project_revision":row.base_project_revision,
            "base_candidate_retained":row.base_candidate_retained,
            "state":"rejected",
            "diagnostic_count":row.diagnostic_count,
            "retained_report_bytes":row.retained_report_bytes,
            "detail_method":"attempt/query",
            "discard_method":"attempt/discard",
        })).collect::<Vec<_>>(),
        "retained_report_bytes":snapshot.retained_report_bytes,
        "limits":{
            "max_candidates":candidates::MAX_CANDIDATES,
            "max_drafts":candidates::MAX_DRAFTS,
            "max_attempts":candidates::MAX_ATTEMPTS,
            "max_retained_report_bytes":candidates::MAX_RETAINED_REPORT_BYTES,
            "max_inventory_bytes":64 * 1024,
        },
        "source_authority":false,
        "artifact_materialization":false,
        "execution":false,
        "publication_authority":false,
        "nonclaims":[
            "session_inventory_is_not_persistent_storage",
            "registry_association_is_not_ownership_or_current_candidate_validity",
            "drafts_and_rejected_attempts_are_not_checked_candidates",
            "references_grant_no_source_execution_materialization_or_publication_authority",
        ],
    });
    if value.to_string().len() > 64 * 1024 {
        return Err(failure(
            "SPX-G357",
            "retained subject inventory exceeds its transport byte bound",
        ));
    }
    Ok(value)
}
