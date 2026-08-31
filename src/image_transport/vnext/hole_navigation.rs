//! Compact typed navigation over one retained immutable draft context.
use super::*;
use crate::project::{
    ProjectCandidateDraft, MAX_PROJECT_HOLE_NAVIGATION_ITEMS,
    PROJECT_DRAFT_EXPRESSION_CATALOG_SCHEMA, PROJECT_HOLE_PAGE_SCHEMA, PROJECT_HOLE_SUMMARY_SCHEMA,
};

const DRAFT: Parameter = Parameter {
    name: "draft_revision",
    kind: ParameterKind::Digest,
    required: true,
};
const HOLE: Parameter = Parameter {
    name: "hole_id",
    kind: ParameterKind::Text(128),
    required: true,
};
const METHODS: &[Method] = &[
    Method {
        name: "hole/expression-catalog",
        operation: Operation::VNext(Action::DraftExpressionCatalog),
        parameters: &[
            REVISION,
            DRAFT,
            Parameter {
                name: "target",
                kind: ParameterKind::Text(4096),
                required: true,
            },
            Parameter {
                name: "region",
                kind: ParameterKind::Choice(&["body", "contract"]),
                required: true,
            },
        ],
        query: true,
        payload_schema: PROJECT_DRAFT_EXPRESSION_CATALOG_SCHEMA,
    },
    Method {
        name: "hole/summary",
        operation: Operation::VNext(Action::HoleSummary),
        parameters: &[REVISION, DRAFT, HOLE],
        query: true,
        payload_schema: PROJECT_HOLE_SUMMARY_SCHEMA,
    },
    Method {
        name: "hole/page",
        operation: Operation::VNext(Action::HolePage),
        parameters: &[
            REVISION,
            DRAFT,
            HOLE,
            Parameter {
                name: "reference",
                kind: ParameterKind::Digest,
                required: true,
            },
            Parameter {
                name: "offset",
                kind: ParameterKind::Integer(0, MAX_PROJECT_HOLE_NAVIGATION_ITEMS),
                required: false,
            },
            Parameter {
                name: "limit",
                kind: ParameterKind::Integer(1, 64),
                required: false,
            },
        ],
        query: true,
        payload_schema: PROJECT_HOLE_PAGE_SCHEMA,
    },
];

pub(super) fn methods() -> &'static [Method] {
    METHODS
}

pub(super) fn prepare(
    action: Action,
    params: &Map<String, Value>,
    image: &ProjectSemanticImage,
    registry: &candidates::Registry,
) -> Result<Value, Vec<Diagnostic>> {
    if text(params, "image_revision") != image.image_digest() {
        return Err(failure("SPX-G282", "v5 expected image revision is stale"));
    }
    let draft = registry.draft_value(text(params, "draft_revision"))?;
    for_draft(action, params, draft)
}

pub(super) fn for_draft(
    action: Action,
    params: &Map<String, Value>,
    draft: &ProjectCandidateDraft,
) -> Result<Value, Vec<Diagnostic>> {
    let expected = text(params, "draft_revision");
    let report = match action {
        Action::DraftExpressionCatalog => match text(params, "region") {
            "body" => draft.expression_catalog(expected, text(params, "target"))?,
            "contract" => draft.contract_expression_catalog(expected, text(params, "target"))?,
            _ => {
                return Err(failure(
                    "SPX-G230",
                    "draft expression catalogue region is invalid",
                ))
            }
        },
        Action::HoleSummary => draft.hole_summary(expected, text(params, "hole_id"))?,
        Action::HolePage => draft.hole_page(
            expected,
            text(params, "hole_id"),
            text(params, "reference"),
            number(params, "offset", 0),
            number(params, "limit", 16),
        )?,
        _ => return Err(failure("SPX-G230", "operation is not hole navigation")),
    };
    serde_json::from_str(&report)
        .map_err(|_| failure("SPX-G230", "hole navigation report is not compiler JSON"))
}
