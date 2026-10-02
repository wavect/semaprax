//! Closed v5 transport adapters for compiler-owned explorer projections.
use super::*;
use crate::project::{
    ExplorerDirection, ExplorerMode, ExplorerPageOptions, ExplorerQuery, ExplorerSide,
    ExplorerView, ProjectCandidate, MAX_EXPLORER_PAGE_BYTES, MAX_EXPLORER_SUMMARY_BYTES,
};

const IMAGE_METHODS: &[Method] = &[
    Method {
        name: "image/explorer-summary",
        operation: Operation::VNext(Action::ExplorerSummary),
        parameters: &[
            REVISION,
            MODE,
            TARGET_OPTIONAL,
            DIRECTION,
            DEPTH,
            MAX_NODES,
            EXPLORER_MAX_BYTES,
        ],
        query: true,
        payload_schema: "semaprax.explorer-view.v1",
    },
    Method {
        name: "image/explorer-page",
        operation: Operation::VNext(Action::ExplorerPage),
        parameters: &[
            REVISION,
            MODE,
            TARGET_OPTIONAL,
            DIRECTION,
            DEPTH,
            MAX_NODES,
            EXPLORER_MAX_BYTES,
            VIEW,
            HANDLE,
            CURSOR,
            PAGE_SIZE,
            PAGE_BYTES,
        ],
        query: true,
        payload_schema: "semaprax.explorer-view.v1",
    },
];
const CANDIDATE_METHODS: &[Method] = &[
    Method {
        name: "candidate/explorer-summary",
        operation: Operation::VNext(Action::CandidateExplorerSummary),
        parameters: &[
            REVISION,
            CANDIDATE,
            SIDE,
            MODE,
            TARGET_OPTIONAL,
            DIRECTION,
            DEPTH,
            MAX_NODES,
            EXPLORER_MAX_BYTES,
        ],
        query: true,
        payload_schema: "semaprax.explorer-view.v1",
    },
    Method {
        name: "candidate/explorer-page",
        operation: Operation::VNext(Action::CandidateExplorerPage),
        parameters: &[
            REVISION,
            CANDIDATE,
            SIDE,
            MODE,
            TARGET_OPTIONAL,
            DIRECTION,
            DEPTH,
            MAX_NODES,
            EXPLORER_MAX_BYTES,
            VIEW,
            HANDLE,
            CURSOR,
            PAGE_SIZE,
            PAGE_BYTES,
        ],
        query: true,
        payload_schema: "semaprax.explorer-view.v1",
    },
];
const CANDIDATE: Parameter = Parameter {
    name: "candidate_revision",
    kind: ParameterKind::Digest,
    required: true,
};
const SIDE: Parameter = Parameter {
    name: "side",
    kind: ParameterKind::Choice(&["base", "candidate"]),
    required: true,
};
const MODE: Parameter = Parameter {
    name: "mode",
    kind: ParameterKind::Choice(&["overview", "context", "impact"]),
    required: true,
};
const TARGET_OPTIONAL: Parameter = Parameter {
    name: "target",
    kind: ParameterKind::Text(4096),
    required: false,
};
const DIRECTION: Parameter = Parameter {
    name: "direction",
    kind: ParameterKind::Choice(&["forward", "reverse", "both"]),
    required: false,
};
const DEPTH: Parameter = Parameter {
    name: "depth",
    kind: ParameterKind::Integer(0, 16),
    required: false,
};
const MAX_NODES: Parameter = Parameter {
    name: "max_nodes",
    kind: ParameterKind::Integer(1, 256),
    required: false,
};
const EXPLORER_MAX_BYTES: Parameter = Parameter {
    name: "analysis_max_bytes",
    kind: ParameterKind::Integer(1024, 256 * 1024),
    required: false,
};
const VIEW: Parameter = Parameter {
    name: "view",
    kind: ParameterKind::Choice(&["modules", "declarations", "relations", "frontier"]),
    required: true,
};
const HANDLE: Parameter = Parameter {
    name: "handle",
    kind: ParameterKind::Digest,
    required: true,
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
const PAGE_BYTES: Parameter = Parameter {
    name: "max_bytes",
    kind: ParameterKind::Integer(1024, MAX_EXPLORER_PAGE_BYTES),
    required: false,
};

pub(super) fn image_methods() -> &'static [Method] {
    IMAGE_METHODS
}
pub(super) fn candidate_methods() -> &'static [Method] {
    CANDIDATE_METHODS
}
fn target(params: &Map<String, Value>) -> Option<&str> {
    params.get("target").and_then(Value::as_str)
}
fn query(params: &Map<String, Value>) -> Result<ExplorerQuery, Vec<Diagnostic>> {
    ExplorerQuery::new(
        ExplorerDirection::parse(
            params
                .get("direction")
                .and_then(Value::as_str)
                .unwrap_or("both"),
        )?,
        number(params, "depth", 1),
        number(params, "max_nodes", 256),
        number(params, "analysis_max_bytes", 256 * 1024),
    )
}
fn options(params: &Map<String, Value>) -> Result<ExplorerPageOptions, Vec<Diagnostic>> {
    ExplorerPageOptions::new(
        number(params, "page_size", 32),
        number(params, "max_bytes", 64 * 1024),
    )
}
fn parse(report: String) -> Result<Value, Vec<Diagnostic>> {
    if report.len() > MAX_EXPLORER_PAGE_BYTES {
        return Err(failure(
            "SPX-G328",
            "explorer report exceeds its transport byte bound",
        ));
    }
    serde_json::from_str(&report)
        .map_err(|_| failure("SPX-G326", "explorer report is not compiler JSON"))
}

pub(super) fn image(
    action: Action,
    params: &Map<String, Value>,
    image: &ProjectSemanticImage,
) -> Result<Value, Vec<Diagnostic>> {
    let expected = text(params, "image_revision");
    let mode = ExplorerMode::parse(text(params, "mode"))?;
    let query = query(params)?;
    let report = match action {
        Action::ExplorerSummary => image.explorer_summary(expected, mode, target(params), query)?,
        Action::ExplorerPage => image.explorer_page(
            expected,
            mode,
            target(params),
            query,
            ExplorerView::parse(text(params, "view"))?,
            text(params, "handle"),
            params.get("cursor").and_then(Value::as_str),
            options(params)?,
        )?,
        _ => return Err(failure("SPX-G326", "unsupported image explorer action")),
    };
    if report.len() > MAX_EXPLORER_SUMMARY_BYTES && matches!(action, Action::ExplorerSummary) {
        return Err(failure(
            "SPX-G328",
            "explorer summary exceeds its transport byte bound",
        ));
    }
    parse(report)
}
pub(super) fn candidate(
    action: Action,
    params: &Map<String, Value>,
    image: &ProjectSemanticImage,
    candidate: &ProjectCandidate,
) -> Result<Value, Vec<Diagnostic>> {
    if text(params, "image_revision") != image.image_digest() {
        return Err(failure("SPX-G282", "v5 expected image revision is stale"));
    }
    let expected = text(params, "candidate_revision");
    let side = ExplorerSide::parse(text(params, "side"))?;
    let mode = ExplorerMode::parse(text(params, "mode"))?;
    let query = query(params)?;
    let report = match action {
        Action::CandidateExplorerSummary => {
            candidate.explorer_summary(expected, side, mode, target(params), query)?
        }
        Action::CandidateExplorerPage => candidate.explorer_page(
            expected,
            side,
            mode,
            target(params),
            query,
            ExplorerView::parse(text(params, "view"))?,
            text(params, "handle"),
            params.get("cursor").and_then(Value::as_str),
            options(params)?,
        )?,
        _ => return Err(failure("SPX-G326", "unsupported candidate explorer action")),
    };
    if report.len() > MAX_EXPLORER_SUMMARY_BYTES
        && matches!(action, Action::CandidateExplorerSummary)
    {
        return Err(failure(
            "SPX-G328",
            "candidate explorer summary exceeds its transport byte bound",
        ));
    }
    parse(report)
}
