//! Read-only navigation of retained generic instances, never instantiation.
use super::*;
use crate::project::{ImageFacet, ImageFacetOptions};

const CURSOR: Parameter = Parameter {
    name: "cursor",
    kind: ParameterKind::Text(100),
    required: false,
};
const PAGE_SIZE: Parameter = Parameter {
    name: "page_size",
    kind: ParameterKind::Integer(1, 128),
    required: false,
};
const MAX_BYTES: Parameter = Parameter {
    name: "max_bytes",
    kind: ParameterKind::Integer(1024, 1024 * 1024),
    required: false,
};
const METHODS: &[Method] = &[
    Method {
        name: "image/function-instances",
        operation: Operation::VNext(Action::FunctionInstances),
        parameters: &[REVISION, TARGET, CURSOR, PAGE_SIZE, MAX_BYTES],
        query: true,
        payload_schema: "semaprax.image-function-instances.v1",
    },
    Method {
        name: "image/function-instance-facet",
        operation: Operation::VNext(Action::FunctionInstanceFacet),
        parameters: &[
            REVISION,
            TARGET,
            Parameter {
                name: "instance_id",
                kind: ParameterKind::Text(65536),
                required: true,
            },
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
        payload_schema: "semaprax.image-instance-facet.v1",
    },
];

pub(super) fn methods() -> &'static [Method] {
    METHODS
}

pub(super) fn prepare(
    action: Action,
    params: &Map<String, Value>,
    image: &ProjectSemanticImage,
) -> Result<Value, Vec<Diagnostic>> {
    let expected = text(params, "image_revision");
    let target = text(params, "target");
    let cursor = params.get("cursor").and_then(Value::as_str);
    let options = ImageFacetOptions::new(
        number(params, "page_size", 32),
        number(params, "max_bytes", 65536),
    )?;
    let report = match action {
        Action::FunctionInstances => image.function_instances(expected, target, cursor, options)?,
        Action::FunctionInstanceFacet => image.expand_instance_facet(
            expected,
            target,
            text(params, "instance_id"),
            ImageFacet::parse(text(params, "facet"))?,
            text(params, "handle"),
            cursor,
            options,
        )?,
        _ => {
            return Err(failure(
                "SPX-G227",
                "unsupported instance navigation action",
            ))
        }
    };
    serde_json::from_str(&report).map_err(|_| {
        failure(
            "SPX-G227",
            "instance navigation report is not compiler JSON",
        )
    })
}
