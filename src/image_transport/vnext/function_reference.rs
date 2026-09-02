//! Exact-revision function references are immutable navigation values, not handles.
use super::*;
use crate::project::{
    ImageFacet, IMAGE_FUNCTION_REFERENCE_RESOLUTION_SCHEMA, IMAGE_FUNCTION_REFERENCE_SCHEMA,
    MAX_IMAGE_FUNCTION_REFERENCE_BYTES, MAX_IMAGE_FUNCTION_REFERENCE_RESOLUTION_BYTES,
};

const FACET: Parameter = Parameter {
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
    required: false,
};

const METHODS: &[Method] = &[
    Method {
        name: "image/function-reference-export",
        operation: Operation::VNext(Action::FunctionReferenceExport),
        parameters: &[REVISION, TARGET, FACET],
        query: true,
        payload_schema: IMAGE_FUNCTION_REFERENCE_SCHEMA,
    },
    Method {
        name: "image/function-reference-resolve",
        operation: Operation::VNext(Action::FunctionReferenceResolve),
        parameters: &[
            REVISION,
            Parameter {
                name: "reference",
                kind: ParameterKind::Text(MAX_IMAGE_FUNCTION_REFERENCE_BYTES),
                required: true,
            },
        ],
        query: true,
        payload_schema: IMAGE_FUNCTION_REFERENCE_RESOLUTION_SCHEMA,
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
    let report = match action {
        Action::FunctionReferenceExport => image.export_function_reference(
            expected,
            text(params, "target"),
            params
                .get("facet")
                .and_then(Value::as_str)
                .map(ImageFacet::parse)
                .transpose()?,
        ),
        Action::FunctionReferenceResolve => {
            image.resolve_function_reference(expected, text(params, "reference").as_bytes())
        }
        _ => Err(failure(
            "SPX-G363",
            "unsupported function reference transport action",
        )),
    }?;
    let bound = match action {
        Action::FunctionReferenceExport => MAX_IMAGE_FUNCTION_REFERENCE_BYTES,
        Action::FunctionReferenceResolve => MAX_IMAGE_FUNCTION_REFERENCE_RESOLUTION_BYTES,
        _ => unreachable!("action checked above"),
    };
    if report.len() > bound {
        return Err(failure(
            "SPX-G364",
            "function reference report exceeds its transport byte bound",
        ));
    }
    serde_json::from_str(&report)
        .map_err(|_| failure("SPX-G363", "function reference report is not compiler JSON"))
}
