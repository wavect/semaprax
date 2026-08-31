//! Compact semantic-impact navigation over one immutable retained candidate.
use super::*;
use crate::project::{
    CandidateImpactPageOptions, CandidateImpactView, ProjectCandidate,
    MAX_PROJECT_CANDIDATE_IMPACT_PAGE_BYTES, MAX_PROJECT_CANDIDATE_IMPACT_SUMMARY_BYTES,
    PROJECT_CANDIDATE_IMPACT_PAGE_SCHEMA, PROJECT_CANDIDATE_IMPACT_SUMMARY_SCHEMA,
};
use crate::workspace_analysis::WorkspaceImpactOptions;

const DEPTH: Parameter = Parameter {
    name: "depth",
    kind: ParameterKind::Integer(0, 1024),
    required: false,
};
const IMPACT_MAX_BYTES: Parameter = Parameter {
    name: "impact_max_bytes",
    kind: ParameterKind::Integer(4096, 16 * 1024 * 1024),
    required: false,
};
const MAX_NODES: Parameter = Parameter {
    name: "max_nodes",
    kind: ParameterKind::Integer(1, 8208),
    required: false,
};

const METHODS: &[Method] = &[
    Method {
        name: "candidate/impact-summary",
        operation: Operation::VNext(Action::CandidateImpactSummary),
        parameters: &[
            REVISION,
            Parameter {
                name: "candidate_revision",
                kind: ParameterKind::Digest,
                required: true,
            },
            TARGET,
            DEPTH,
            IMPACT_MAX_BYTES,
            MAX_NODES,
        ],
        query: true,
        payload_schema: PROJECT_CANDIDATE_IMPACT_SUMMARY_SCHEMA,
    },
    Method {
        name: "candidate/impact-page",
        operation: Operation::VNext(Action::CandidateImpactPage),
        parameters: &[
            REVISION,
            Parameter {
                name: "candidate_revision",
                kind: ParameterKind::Digest,
                required: true,
            },
            TARGET,
            DEPTH,
            IMPACT_MAX_BYTES,
            MAX_NODES,
            Parameter {
                name: "view",
                kind: ParameterKind::Choice(&["affected", "dependency_edges", "frontier"]),
                required: true,
            },
            Parameter {
                name: "handle",
                kind: ParameterKind::Digest,
                required: true,
            },
            Parameter {
                name: "cursor",
                kind: ParameterKind::Text(128),
                required: false,
            },
            Parameter {
                name: "page_size",
                kind: ParameterKind::Integer(1, 128),
                required: false,
            },
            Parameter {
                name: "max_bytes",
                kind: ParameterKind::Integer(1024, MAX_PROJECT_CANDIDATE_IMPACT_PAGE_BYTES),
                required: false,
            },
        ],
        query: true,
        payload_schema: PROJECT_CANDIDATE_IMPACT_PAGE_SCHEMA,
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
    let impact_options = WorkspaceImpactOptions::new(
        number(params, "depth", 16),
        number(params, "impact_max_bytes", 1024 * 1024),
        number(params, "max_nodes", 1024),
    )
    .map_err(|error| vec![error])?;
    let report = match action {
        Action::CandidateImpactSummary => {
            candidate.impact_summary(expected, target, impact_options)?
        }
        Action::CandidateImpactPage => candidate.impact_page(
            expected,
            target,
            impact_options,
            CandidateImpactView::parse(text(params, "view"))?,
            text(params, "handle"),
            params.get("cursor").and_then(Value::as_str),
            CandidateImpactPageOptions::new(
                number(params, "page_size", 32),
                number(params, "max_bytes", 65_536),
            )?,
        )?,
        _ => {
            return Err(failure(
                "SPX-G333",
                "unsupported candidate impact navigation action",
            ))
        }
    };
    let maximum = match action {
        Action::CandidateImpactSummary => MAX_PROJECT_CANDIDATE_IMPACT_SUMMARY_BYTES,
        Action::CandidateImpactPage => MAX_PROJECT_CANDIDATE_IMPACT_PAGE_BYTES,
        _ => unreachable!("candidate impact action was selected above"),
    };
    if report.len() > maximum {
        return Err(failure(
            "SPX-G334",
            "candidate impact navigation report exceeds its transport byte bound",
        ));
    }
    serde_json::from_str(&report).map_err(|_| {
        failure(
            "SPX-G333",
            "candidate impact navigation report is not compiler JSON",
        )
    })
}
