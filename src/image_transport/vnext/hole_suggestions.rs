//! Source-admitted fill suggestions over one immutable retained draft.
use super::*;
use crate::project::{ProjectCandidateDraft, PROJECT_HOLE_FILL_SUGGESTIONS_SCHEMA};

const METHOD: Method = Method {
    name: "hole/fill-suggestions",
    operation: Operation::VNext(Action::HoleFillSuggestions),
    parameters: &[
        REVISION,
        Parameter {
            name: "draft_revision",
            kind: ParameterKind::Digest,
            required: true,
        },
        Parameter {
            name: "hole_id",
            kind: ParameterKind::Text(128),
            required: true,
        },
    ],
    query: true,
    payload_schema: PROJECT_HOLE_FILL_SUGGESTIONS_SCHEMA,
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
    for_draft(
        params,
        registry.draft_value(text(params, "draft_revision"))?,
    )
}

pub(super) fn for_draft(
    params: &Map<String, Value>,
    draft: &ProjectCandidateDraft,
) -> Result<Value, Vec<Diagnostic>> {
    let report =
        draft.hole_fill_suggestions(text(params, "draft_revision"), text(params, "hole_id"))?;
    serde_json::from_str(&report)
        .map_err(|_| failure("SPX-G230", "hole fill suggestions are not compiler JSON"))
}
