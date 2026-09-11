//! Provider-compatible presentation projections derived from the canonical
//! interaction schema.
//!
//! [`render_json_schema`] renders one self-contained JSON Schema draft
//! 2020-12 document (recursive references use local `$defs`; a validator
//! needs no network lookup), following the same convention as
//! [Candidate Constructor Schemas v1](../../docs/CANDIDATE-CONSTRUCTOR-SCHEMAS-V1.md).
//! Exact integers are presented as decimal-string patterns, never as JSON
//! `type: integer`, so that no generated client derived from this
//! projection can round an out-of-`f64`-range value through a native
//! number type. `Bytes` is presented as the same bounded array-of-byte-
//! integers wire form the canonical decoder accepts.
//!
//! This projection is informational. [`super::decode::decode`] never reads
//! it, and construction here cannot alter what the canonical decoder
//! admits: a provider's inability to express a constraint this document
//! carries (for example the `x-minimum`/`x-maximum` vendor extension on an
//! exact integer) can only ever cause the *provider* to accept something
//! the canonical decoder would still refuse post-response — it can never
//! cause the decoder to accept more than the canonical schema admits.

use crate::diagnostic::quote_json;

use super::shape::{FieldRow, FieldType, Representation, TypeDecl, TypeGraph, TypeShape};
use super::{MAX_BYTES_FIELD_BYTES, MAX_STRING_FIELD_BYTES};

/// Renders one self-contained JSON Schema draft 2020-12 document describing
/// exactly the value shape [`super::decode::decode`] admits for this graph.
pub(crate) fn render_json_schema(graph: &TypeGraph) -> String {
    let mut defs = String::from("{");
    for (index, decl) in graph.types.iter().enumerate() {
        if index > 0 {
            defs.push(',');
        }
        defs.push_str(&format!(
            "{}:{}",
            quote_json(&decl.stable_id),
            render_type_schema(decl)
        ));
    }
    defs.push('}');
    format!(
        "{{\"$schema\":\"https://json-schema.org/draft/2020-12/schema\",\"$id\":{},\"$defs\":{defs},\"$ref\":{}}}\n",
        quote_json(&format!(
            "urn:semaprax.agent-interaction-schema.v1:{}",
            graph.root_type_id
        )),
        quote_json(&format!("#/$defs/{}", graph.root_type_id)),
    )
}

fn render_type_schema(decl: &TypeDecl) -> String {
    match &decl.shape {
        TypeShape::Record { fields } => format!(
            "{{\"type\":\"object\",\"additionalProperties\":false,\"required\":[\"fields\"],\"properties\":{{\"fields\":{}}}}}",
            render_fields_schema(fields)
        ),
        TypeShape::Variant { cases } => {
            let mut alternatives = String::new();
            for (index, case) in cases.iter().enumerate() {
                if index > 0 {
                    alternatives.push(',');
                }
                alternatives.push_str(&format!(
                    "{{\"type\":\"object\",\"additionalProperties\":false,\"required\":[\"case\",\"fields\"],\"properties\":{{\"case\":{{\"const\":{}}},\"fields\":{}}}}}",
                    quote_json(&case.stable_id),
                    render_fields_schema(&case.fields)
                ));
            }
            format!("{{\"oneOf\":[{alternatives}]}}")
        }
    }
}

fn render_fields_schema(fields: &[FieldRow]) -> String {
    let mut required = String::new();
    let mut properties = String::new();
    for (index, field) in fields.iter().enumerate() {
        if index > 0 {
            required.push(',');
            properties.push(',');
        }
        required.push_str(&quote_json(&field.stable_id));
        properties.push_str(&format!(
            "{}:{}",
            quote_json(&field.stable_id),
            render_type_ref_schema(&field.ty)
        ));
    }
    format!(
        "{{\"type\":\"object\",\"additionalProperties\":false,\"required\":[{required}],\"properties\":{{{properties}}}}}"
    )
}

fn render_type_ref_schema(ty: &FieldType) -> String {
    match ty {
        FieldType::Scalar(representation) => render_scalar_schema(*representation),
        FieldType::Nested(stable_id) => {
            format!(
                "{{\"$ref\":{}}}",
                quote_json(&format!("#/$defs/{stable_id}"))
            )
        }
    }
}

fn render_scalar_schema(representation: Representation) -> String {
    match representation {
        Representation::Bool => "{\"type\":\"boolean\"}".to_owned(),
        Representation::Text => {
            format!("{{\"type\":\"string\",\"maxLength\":{MAX_STRING_FIELD_BYTES}}}")
        }
        Representation::Bytes => format!(
            "{{\"type\":\"array\",\"items\":{{\"type\":\"integer\",\"minimum\":0,\"maximum\":255}},\"maxItems\":{MAX_BYTES_FIELD_BYTES}}}"
        ),
        Representation::I32 | Representation::I64 | Representation::U8 | Representation::U64 => {
            let (minimum, maximum) = representation
                .bounds()
                .expect("an integer representation always declares bounds");
            format!(
                "{{\"type\":\"string\",\"pattern\":\"^-?[0-9]+$\",\"x-representation\":{},\"x-minimum\":{},\"x-maximum\":{}}}",
                quote_json(representation.name()),
                quote_json(minimum),
                quote_json(maximum)
            )
        }
    }
}
