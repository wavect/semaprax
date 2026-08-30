//! Immutable declaration dependency facts, shared with the host read batch.
use super::*;
use crate::project::{
    ImageDependencyPageOptions, ImageDependencyView, IMAGE_DECLARATION_DEPENDENCIES_SCHEMA,
    MAX_IMAGE_DECLARATION_DEPENDENCIES_BYTES,
};

const METHOD: Method = Method {
    name: "image/dependencies",
    operation: Operation::VNext(Action::Dependencies),
    parameters: &[
        REVISION,
        TARGET,
        Parameter {
            name: "offset",
            kind: ParameterKind::Integer(0, MAX_IMAGE_DECLARATION_DEPENDENCIES_BYTES),
            required: false,
        },
        Parameter {
            name: "chunk_bytes",
            kind: ParameterKind::Integer(1024, 65536),
            required: false,
        },
    ],
    query: true,
    payload_schema: "semaprax.image-declaration-dependencies-chunk.v1",
};

pub(super) fn method() -> &'static Method {
    &METHOD
}

const NAVIGATION_METHODS: &[Method] = &[
    Method {
        name: "image/dependency-summary",
        operation: Operation::VNext(Action::DependencySummary),
        parameters: &[REVISION, TARGET],
        query: true,
        payload_schema: "semaprax.image-dependency-summary.v1",
    },
    Method {
        name: "image/dependency-page",
        operation: Operation::VNext(Action::DependencyPage),
        parameters: &[
            REVISION,
            TARGET,
            Parameter {
                name: "view",
                kind: ParameterKind::Choice(&["sites", "callers", "calls", "members"]),
                required: true,
            },
            Parameter {
                name: "handle",
                kind: ParameterKind::Text(71),
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
                kind: ParameterKind::Integer(1024, 1024 * 1024),
                required: false,
            },
        ],
        query: true,
        payload_schema: "semaprax.image-dependency-page.v1",
    },
];

pub(super) fn navigation_methods() -> &'static [Method] {
    NAVIGATION_METHODS
}

pub(super) fn prepare_navigation(
    action: Action,
    params: &Map<String, Value>,
    image: &ProjectSemanticImage,
) -> Result<Value, Vec<Diagnostic>> {
    let expected = text(params, "image_revision");
    let target = text(params, "target");
    let report = match action {
        Action::DependencySummary => image.dependency_summary(expected, target)?,
        Action::DependencyPage => image.dependency_page(
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
                "SPX-G320",
                "unsupported dependency navigation action",
            ))
        }
    };
    parse_payload(report)
}

pub(super) fn prepare(
    params: &Map<String, Value>,
    image: &ProjectSemanticImage,
) -> Result<Value, Vec<Diagnostic>> {
    if text(params, "image_revision") != image.image_digest() {
        return Err(failure(
            "SPX-G221",
            "dependency query image revision is stale",
        ));
    }
    let target = text(params, "target");
    let report = image.declaration_dependencies(image.image_digest(), target)?;
    let offset = number(params, "offset", 0);
    let chunk_bytes = number(params, "chunk_bytes", 16384);
    if report.len() > MAX_IMAGE_DECLARATION_DEPENDENCIES_BYTES {
        return Err(failure(
            "SPX-G220",
            "dependency report exceeds its transport byte bound",
        ));
    }
    if !(1024..=65536).contains(&chunk_bytes)
        || offset > report.len()
        || !report.is_char_boundary(offset)
    {
        return Err(failure(
            "SPX-G219",
            "dependency chunk is outside its bounded UTF8 report",
        ));
    }
    let mut end = offset.saturating_add(chunk_bytes).min(report.len());
    while !report.is_char_boundary(end) {
        end -= 1;
    }
    Ok(json!({
        "schema":"semaprax.image-declaration-dependencies-chunk.v1",
        "report_schema":IMAGE_DECLARATION_DEPENDENCIES_SCHEMA,"image_revision":image.image_digest(),"target":target,
        "offset":offset,"total_bytes":report.len(),"chunk":&report[offset..end],
        "next_offset":(end<report.len()).then_some(end),"source_authority":false,
    }))
}
