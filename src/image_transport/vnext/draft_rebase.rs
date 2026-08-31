//! Explicit typed-draft rebase; only the resulting draft enters the registry.
use super::*;

const METHOD: Method = Method {
    name: "hole/rebase",
    operation: Operation::VNext(Action::DraftRebase),
    parameters: &[
        REVISION,
        Parameter {
            name: "draft_revision",
            kind: ParameterKind::Digest,
            required: true,
        },
        Parameter {
            name: "new_base_candidate_revision",
            kind: ParameterKind::Digest,
            required: true,
        },
    ],
    query: false,
    payload_schema: "semaprax.image-draft-rebase.v1",
};

pub(super) fn method() -> &'static Method {
    &METHOD
}

pub(super) fn prepare(
    params: &Map<String, Value>,
    image: &ProjectSemanticImage,
    registry: &candidates::Registry,
) -> Result<(Value, candidates::Mutation), Vec<Diagnostic>> {
    if text(params, "image_revision") != image.image_digest() {
        return Err(failure("SPX-G232", "draft rebase image revision is stale"));
    }
    let draft = registry.draft_value(text(params, "draft_revision"))?;
    let selected = registry.candidate(text(params, "new_base_candidate_revision"))?;
    let prepared = draft.rebase(
        draft.draft_digest(),
        Arc::clone(selected.revision()),
        selected.revision().project_revision(),
    )?;
    // Match the existing candidate reconciliation transport bound. A large
    // library report never causes an unreported draft registry insertion.
    if prepared.to_json().len() > 65_536 {
        return Err(failure(
            "SPX-G234",
            "draft reconciliation report exceeds transport report bound",
        ));
    }
    let report: Value = serde_json::from_str(prepared.to_json())
        .map_err(|_| failure("SPX-G230", "draft rebase report is not compiler JSON"))?;
    let draft = prepared.into_draft();
    let summary: Value = serde_json::from_str(draft.summary(draft.draft_digest())?)
        .map_err(|_| failure("SPX-G230", "rebased draft summary is not compiler JSON"))?;
    let source_candidate = summary["last_valid_candidate_digest"]
        .as_str()
        .ok_or_else(|| failure("SPX-G230", "rebased draft candidate binding is absent"))?;
    let (handle, mutation) = registry.retain_recovered_draft(draft, source_candidate)?;
    Ok((
        json!({
            "schema":"semaprax.image-draft-rebase.v1",
            "selected_candidate_revision":selected.candidate_digest(),
            "draft":handle,"report":report,
        }),
        mutation,
    ))
}
