//! Canonical rendering of the derived [`super::shape::TypeGraph`].
//!
//! Rendering only ever emits persistent stable identities and exact scalar
//! representations. Display names never reach this module, so a display
//! rename never changes rendered bytes or the derived revision, while any
//! structural change (a different field set, a different representation, a
//! different case, a different nested type) changes both.

use crate::diagnostic::quote_json;

use super::shape::{CaseRow, FieldRow, FieldType, Representation, TypeDecl, TypeGraph, TypeShape};
use super::{
    MAX_BYTES_FIELD_BYTES, MAX_DEPTH, MAX_DOCUMENT_BYTES, MAX_STRING_FIELD_BYTES, MAX_TYPES,
    SCHEMA_V1,
};

const NONCLAIMS: [&str; 8] = [
    "no_authorization_value_or_publication_token_from_a_decoded_value",
    "no_capability_grant_effect_or_host_authority",
    "no_trust_in_untrusted_bytes_without_decoder_validation",
    "no_arbitrary_recursion_resource_borrowed_or_callback_values",
    "no_floating_point_or_lossy_numeric_transport",
    "no_public_abi_promotion_in_this_slice",
    "provider_presentation_projections_never_broaden_runtime_admission",
    "decoding_never_reads_a_provider_presentation_projection",
];

/// Renders the closed `types` member shared by the canonical schema and (via
/// [`super::provider`]) every provider presentation projection.
pub(crate) fn render_types(types: &[TypeDecl]) -> String {
    let mut output = String::from("[");
    for (index, decl) in types.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&render_type(decl));
    }
    output.push(']');
    output
}

fn render_type(decl: &TypeDecl) -> String {
    match &decl.shape {
        TypeShape::Record { fields } => format!(
            "{{\"stable_id\":{},\"kind\":\"record\",\"fields\":{}}}",
            quote_json(&decl.stable_id),
            render_fields(fields)
        ),
        TypeShape::Variant { cases } => {
            let mut output = format!(
                "{{\"stable_id\":{},\"kind\":\"variant\",\"cases\":[",
                quote_json(&decl.stable_id)
            );
            for (index, case) in cases.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                output.push_str(&render_case(case));
            }
            output.push_str("]}");
            output
        }
    }
}

fn render_case(case: &CaseRow) -> String {
    format!(
        "{{\"stable_id\":{},\"fields\":{}}}",
        quote_json(&case.stable_id),
        render_fields(&case.fields)
    )
}

fn render_fields(fields: &[FieldRow]) -> String {
    let mut output = String::from("[");
    for (index, field) in fields.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&format!(
            "{{\"stable_id\":{},\"type\":{}}}",
            quote_json(&field.stable_id),
            render_type_ref(&field.ty)
        ));
    }
    output.push(']');
    output
}

pub(crate) fn render_type_ref(ty: &FieldType) -> String {
    match ty {
        FieldType::Scalar(representation) => render_scalar_ref(*representation),
        FieldType::Nested(stable_id) => {
            format!(
                "{{\"kind\":\"nested\",\"stable_id\":{}}}",
                quote_json(stable_id)
            )
        }
    }
}

fn render_scalar_ref(representation: Representation) -> String {
    let mut output = format!(
        "{{\"kind\":\"scalar\",\"representation\":{}",
        quote_json(representation.name())
    );
    if let Some((minimum, maximum)) = representation.bounds() {
        output.push_str(&format!(
            ",\"minimum\":{},\"maximum\":{}",
            quote_json(minimum),
            quote_json(maximum)
        ));
    }
    if let Some(max_bytes) = representation.max_bytes() {
        output.push_str(&format!(",\"max_bytes\":{max_bytes}"));
    }
    output.push('}');
    output
}

/// Renders the display-name-independent revision body: root identity plus
/// the exact rendered `types` array, nothing else.
pub(crate) fn render_revision_body(root_type_id: &str, rendered_types: &str) -> String {
    format!(
        "{{\"root_type_id\":{},\"types\":{rendered_types}}}",
        quote_json(root_type_id)
    )
}

/// Renders the complete canonical `semaprax.agent-interaction-schema.v1`
/// document, including its terminal LF.
pub(crate) fn render_schema(
    graph: &TypeGraph,
    root_type_revision: &str,
    rendered_types: &str,
) -> String {
    let mut output = format!(
        "{{\"schema\":{},\"root_type_id\":{},\"root_type_revision\":{},\"types\":{rendered_types},\"wire\":{{\"closed_objects\":true,\"key_order\":\"declaration_order\",\"exact_integer_encoding\":\"decimal_string\",\"bytes_encoding\":\"byte_array_u8\",\"max_document_bytes\":{MAX_DOCUMENT_BYTES},\"max_string_field_bytes\":{MAX_STRING_FIELD_BYTES},\"max_bytes_field_bytes\":{MAX_BYTES_FIELD_BYTES},\"max_types\":{MAX_TYPES},\"max_depth\":{MAX_DEPTH}}},\"nonclaims\":[",
        quote_json(SCHEMA_V1),
        quote_json(&graph.root_type_id),
        quote_json(root_type_revision),
    );
    for (index, nonclaim) in NONCLAIMS.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&quote_json(nonclaim));
    }
    output.push_str("]}\n");
    output
}
