//! Compact dependency navigation over one immutable retained candidate.
use super::*;
use crate::project::{ImageDependencyPageOptions, ImageDependencyView, ProjectCandidate};

const SUMMARY_SCHEMA: &str = "semaprax.project-candidate-dependency-summary.v1";
const PAGE_SCHEMA: &str = "semaprax.project-candidate-dependency-page.v1";
const MAX_SUMMARY_BYTES: usize = 64 * 1024;
const MAX_PAGE_BYTES: usize = 1024 * 1024;

const METHODS: &[Method] = &[
    Method {
        name: "candidate/dependency-summary",
        operation: Operation::VNext(Action::CandidateDependencySummary),
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
        payload_schema: SUMMARY_SCHEMA,
    },
    Method {
        name: "candidate/dependency-page",
        operation: Operation::VNext(Action::CandidateDependencyPage),
        parameters: &[
            REVISION,
            Parameter {
                name: "candidate_revision",
                kind: ParameterKind::Digest,
                required: true,
            },
            TARGET,
            Parameter {
                name: "view",
                kind: ParameterKind::Choice(&["sites", "callers", "calls", "members"]),
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
                kind: ParameterKind::Integer(1024, MAX_PAGE_BYTES),
                required: false,
            },
        ],
        query: true,
        payload_schema: PAGE_SCHEMA,
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
    let candidate = registry.candidate(text(params, "candidate_revision"))?;
    for_candidate(action, params, candidate)
}

pub(super) fn for_candidate(
    action: Action,
    params: &Map<String, Value>,
    candidate: &ProjectCandidate,
) -> Result<Value, Vec<Diagnostic>> {
    let expected = text(params, "candidate_revision");
    let target = text(params, "target");
    let report = match action {
        Action::CandidateDependencySummary => candidate.dependency_summary(expected, target)?,
        Action::CandidateDependencyPage => candidate.dependency_page(
            expected,
            target,
            ImageDependencyView::parse(text(params, "view"))?,
            text(params, "handle"),
            params.get("cursor").and_then(Value::as_str),
            ImageDependencyPageOptions::new(
                number(params, "page_size", 32),
                number(params, "max_bytes", 65536),
            )?,
        )?,
        _ => {
            return Err(failure(
                "SPX-G322",
                "unsupported candidate dependency navigation action",
            ))
        }
    };
    let maximum = match action {
        Action::CandidateDependencySummary => MAX_SUMMARY_BYTES,
        Action::CandidateDependencyPage => MAX_PAGE_BYTES,
        _ => unreachable!("candidate dependency action was selected above"),
    };
    if report.len() > maximum {
        return Err(failure(
            "SPX-G323",
            "candidate dependency navigation report exceeds its transport byte bound",
        ));
    }
    serde_json::from_str(&report).map_err(|_| {
        failure(
            "SPX-G322",
            "candidate dependency navigation report is not compiler JSON",
        )
    })
}
