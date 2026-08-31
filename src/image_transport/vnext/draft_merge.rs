//! Explicit merging of checked histories and pending holes; no candidate install.
use super::*;

const METHOD: Method = Method {
    name: "hole/merge",
    operation: Operation::VNext(Action::DraftMerge),
    parameters: &[
        REVISION,
        Parameter {
            name: "draft_revision",
            kind: ParameterKind::Digest,
            required: true,
        },
        Parameter {
            name: "other_draft_revision",
            kind: ParameterKind::Digest,
            required: true,
        },
    ],
    query: false,
    payload_schema: "semaprax.image-draft-merge.v1",
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
        return Err(failure("SPX-G232", "draft merge image revision is stale"));
    }
    let left = registry.draft_value(text(params, "draft_revision"))?;
    let right = registry.draft_value(text(params, "other_draft_revision"))?;
    let prepared = left.merge(left.draft_digest(), right, right.draft_digest())?;
    if prepared.to_json().len() > 65_536 {
        return Err(failure(
            "SPX-G234",
            "draft reconciliation report exceeds transport report bound",
        ));
    }
    let report: Value = serde_json::from_str(prepared.to_json())
        .map_err(|_| failure("SPX-G230", "draft merge report is not compiler JSON"))?;
    let draft = prepared.into_draft();
    let summary: Value = serde_json::from_str(draft.summary(draft.draft_digest())?)
        .map_err(|_| failure("SPX-G230", "merged draft summary is not compiler JSON"))?;
    let source_candidate = summary["last_valid_candidate_digest"]
        .as_str()
        .ok_or_else(|| failure("SPX-G230", "merged draft candidate binding is absent"))?;
    let (handle, mutation) = registry.retain_recovered_draft(draft, source_candidate)?;
    Ok((
        json!({
            "schema":"semaprax.image-draft-merge.v1",
            "left_draft_revision":left.draft_digest(),
            "right_draft_revision":right.draft_digest(),
            "draft":handle,"report":report,
        }),
        mutation,
    ))
}
