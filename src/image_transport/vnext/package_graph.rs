//! Independently verified package subjects attached only by the embedding host.
use super::*;
use crate::package_lock_v2::Coordinate;
use crate::package_semantic_graph::{
    PackageSemanticGraph, MAX_PACKAGE_SEMANTIC_REPORT_BYTES, PACKAGE_SEMANTIC_CONSUMERS_SCHEMA,
    PACKAGE_SEMANTIC_SUMMARY_SCHEMA,
};

const PACKAGE_REVISION: Parameter = Parameter {
    name: "package_revision",
    kind: ParameterKind::Digest,
    required: true,
};
const METHODS: &[Method] = &[
    Method {
        name: "package/summary",
        operation: Operation::VNext(Action::PackageSummary),
        parameters: &[REVISION],
        query: true,
        payload_schema: PACKAGE_SEMANTIC_SUMMARY_SCHEMA,
    },
    Method {
        name: "package/consumers",
        operation: Operation::VNext(Action::PackageConsumers),
        parameters: &[
            REVISION,
            PACKAGE_REVISION,
            Parameter {
                name: "provider_package",
                kind: ParameterKind::Text(255),
                required: true,
            },
            Parameter {
                name: "provider_version",
                kind: ParameterKind::Text(128),
                required: true,
            },
            TARGET,
        ],
        query: true,
        payload_schema: PACKAGE_SEMANTIC_CONSUMERS_SCHEMA,
    },
];

pub(super) fn methods() -> &'static [Method] {
    METHODS
}

impl VNextSession {
    /// Attach one opaque, independently verified package graph before any frame
    /// or batch call. This does not associate packages with the Project, install
    /// candidates, or change the host's execution/publication grants.
    pub fn attach_package_graph(
        &mut self,
        graph: Arc<PackageSemanticGraph>,
        expected_graph: &str,
    ) -> Result<(), Vec<Diagnostic>> {
        if self.started
            || self.package_attachment_closed
            || self.terminal
            || self.package_graph.is_some()
        {
            return Err(failure(
                "SPX-G280",
                "package graph attachment requires an unused session and is permitted only once",
            ));
        }
        self.snapshot.with_authenticated_request(|_| {
            if graph.graph_digest() != expected_graph {
                return Err(failure(
                    "SPX-PS602",
                    "package graph attachment digest is stale",
                ));
            }
            Ok(())
        })?;
        self.package_graph = Some(graph);
        Ok(())
    }
}

pub(super) fn prepare(
    action: Action,
    params: &Map<String, Value>,
    image: &ProjectSemanticImage,
    graph: Option<&PackageSemanticGraph>,
) -> Result<Value, Vec<Diagnostic>> {
    if text(params, "image_revision") != image.image_digest() {
        return Err(failure("SPX-G282", "v5 expected image revision is stale"));
    }
    let graph =
        graph.ok_or_else(|| failure("SPX-G280", "no package graph was attached by the host"))?;
    let report = match action {
        Action::PackageSummary => graph.summary(graph.graph_digest())?,
        Action::PackageConsumers => graph.consumers(
            text(params, "package_revision"),
            &Coordinate {
                package: text(params, "provider_package").to_owned(),
                version: text(params, "provider_version").to_owned(),
            },
            text(params, "target"),
        )?,
        _ => {
            return Err(failure(
                "SPX-G280",
                "operation is not a package graph query",
            ))
        }
    };
    if report.len() > MAX_PACKAGE_SEMANTIC_REPORT_BYTES {
        return Err(failure(
            "SPX-PS603",
            "package graph report exceeds its transport byte bound",
        ));
    }
    serde_json::from_str(&report)
        .map_err(|_| failure("SPX-PS601", "package graph report is not compiler JSON"))
}
