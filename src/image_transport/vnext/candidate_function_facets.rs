//! Candidate-bound compact function summaries and retained-HIR facet pages.
use super::*;
use crate::project::{
    ImageFacet, ImageFacetOptions, ProjectCandidate, MAX_PROJECT_CANDIDATE_FUNCTION_FACET_BYTES,
    PROJECT_CANDIDATE_FUNCTION_FACET_SCHEMA, PROJECT_CANDIDATE_FUNCTION_SUMMARY_SCHEMA,
};

const CURSOR: Parameter = Parameter {
    name: "cursor",
    kind: ParameterKind::Text(128),
    required: false,
};
const PAGE_SIZE: Parameter = Parameter {
    name: "page_size",
    kind: ParameterKind::Integer(1, 128),
    required: false,
};
const MAX_BYTES: Parameter = Parameter {
    name: "max_bytes",
    kind: ParameterKind::Integer(1024, MAX_PROJECT_CANDIDATE_FUNCTION_FACET_BYTES),
    required: false,
};

const METHODS: &[Method] = &[
    Method {
        name: "candidate/function-summary",
        operation: Operation::VNext(Action::CandidateFunctionSummary),
        parameters: &[
            REVISION,
            Parameter {
                name: "candidate_revision",
                kind: ParameterKind::Digest,
                required: true,
            },
            TARGET,
        ],
        query: true,
        payload_schema: PROJECT_CANDIDATE_FUNCTION_SUMMARY_SCHEMA,
    },
    Method {
        name: "candidate/function-facet",
        operation: Operation::VNext(Action::CandidateFunctionFacet),
        parameters: &[
            REVISION,
            Parameter {
                name: "candidate_revision",
                kind: ParameterKind::Digest,
                required: true,
            },
            TARGET,
            Parameter {
                name: "facet",
                kind: ParameterKind::Choice(&[
                    "signature",
                    "contracts",
                    "callers",
                    "ownership",
                    "loans",
                    "cleanup",
                    "relationships",
                    "data-access",
                    "unsafe-boundaries",
                ]),
                required: true,
            },
            Parameter {
                name: "handle",
                kind: ParameterKind::Digest,
                required: true,
            },
            CURSOR,
            PAGE_SIZE,
            MAX_BYTES,
        ],
        query: true,
        payload_schema: PROJECT_CANDIDATE_FUNCTION_FACET_SCHEMA,
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
    for_candidate(
        action,
        params,
        registry.candidate(text(params, "candidate_revision"))?,
    )
}

pub(super) fn for_candidate(
    action: Action,
    params: &Map<String, Value>,
    candidate: &ProjectCandidate,
) -> Result<Value, Vec<Diagnostic>> {
    let expected = text(params, "candidate_revision");
    let target = text(params, "target");
    let report = match action {
        Action::CandidateFunctionSummary => candidate.function_summary(expected, target)?,
        Action::CandidateFunctionFacet => candidate.expand_function_facet(
            expected,
            target,
            ImageFacet::parse(text(params, "facet"))?,
            text(params, "handle"),
            params.get("cursor").and_then(Value::as_str),
            ImageFacetOptions::new(
                number(params, "page_size", 32),
                number(params, "max_bytes", 65536),
            )?,
        )?,
        _ => {
            return Err(failure(
                "SPX-G358",
                "unsupported candidate function facet action",
            ))
        }
    };
    serde_json::from_str(&report).map_err(|_| {
        failure(
            "SPX-G358",
            "candidate function facet report is not compiler JSON",
        )
    })
}
