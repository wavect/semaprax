//! V5 contract-region selection. Existing body-expression routes stay closed.
use super::*;

const CANDIDATE: Parameter = Parameter {
    name: "candidate_revision",
    kind: ParameterKind::Digest,
    required: true,
};
const METHODS: &[Method] = &[
    Method {
        name: "candidate/contract-expression-catalog",
        operation: Operation::VNext(Action::ContractExpressionCatalog),
        parameters: &[REVISION, CANDIDATE, TARGET],
        query: true,
        payload_schema: "semaprax.project-contract-expression-catalog.v1",
    },
    Method {
        name: "hole/open-contract-expression",
        operation: Operation::VNext(Action::ContractHoleOpen),
        parameters: &[
            REVISION,
            CANDIDATE,
            TARGET,
            Parameter {
                name: "expression_id",
                kind: ParameterKind::Text(4096),
                required: true,
            },
            Parameter {
                name: "hole_id",
                kind: ParameterKind::Text(128),
                required: true,
            },
            Parameter {
                name: "draft_revision",
                kind: ParameterKind::Digest,
                required: false,
            },
        ],
        query: false,
        payload_schema: "semaprax.image-draft-handle.v1",
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
) -> Result<(Value, candidates::Mutation), Vec<Diagnostic>> {
    if text(params, "image_revision") != image.image_digest() {
        return Err(failure(
            "SPX-G221",
            "contract expression image revision is stale",
        ));
    }
    match action {
        Action::ContractExpressionCatalog => {
            let candidate = registry.candidate(text(params, "candidate_revision"))?;
            let report = candidate.contract_expression_catalog(text(params, "target"))?;
            let payload = serde_json::from_str(&report).map_err(|_| {
                failure(
                    "SPX-G230",
                    "compiler contract expression catalogue is invalid JSON",
                )
            })?;
            Ok((payload, candidates::Mutation::None))
        }
        Action::ContractHoleOpen => registry.open_contract_hole(
            text(params, "candidate_revision"),
            params.get("draft_revision").and_then(Value::as_str),
            text(params, "target"),
            text(params, "expression_id"),
            text(params, "hole_id"),
        ),
        _ => Err(failure("SPX-G230", "unknown contract expression operation")),
    }
}
