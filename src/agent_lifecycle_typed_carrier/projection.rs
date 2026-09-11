//! Recursive projection from one admitted
//! [`DecodedInteractionValue`] into the real checked interpreter's
//! [`RetainedValue`] vocabulary.
//!
//! This walks the decoded value and its derivation
//! [`TypeGraph`](crate::agent_interaction_schema::shape::TypeGraph) in
//! lockstep (the decoded value alone does not retain a nested field's
//! nominal type identity — only the derivation graph does — so both are
//! required to build a `RetainedRecord`/`RetainedVariant` correctly keyed
//! by persistent stable identity). Every leaf and shape retained_call
//! admits (`bool`/`i32`/`i64`/`u8`/`usize`, owned `Bytes`, and bounded
//! acyclic nested records/variants over exactly those leaves) round-trips
//! exactly. The one leaf retained_call has no carrier for — `string` — is
//! refused explicitly (`SPX-Z210`, `projection.string_unsupported`): this
//! is the "refused, never silently degraded" edge the carrying contract
//! requires for a value that cannot be carried while preserving its
//! type/ownership identity.

use crate::agent_interaction_schema::decode::{DecodedField, FieldValue, ScalarValue, TypedValue};
use crate::agent_interaction_schema::shape::{self, FieldType, Representation, TypeGraph, TypeShape};
use crate::agent_interaction_schema::DecodedInteractionValue;
use crate::diagnostic::Diagnostic;
use crate::hir::{DeclarationId, ResolvedProgram};
use crate::interpreter::retained_call::{RetainedField, RetainedRecord, RetainedValue, RetainedVariant};

use super::refusal;

/// An opaque public handle on the `agent_interaction_schema` derivation
/// graph a decoded value was checked against.
///
/// `agent_interaction_schema::shape::TypeGraph` is `pub(crate)` (owned by
/// issue #109, outside this worker's file lease to widen). This newtype
/// gives the carrier's own public projection/evaluation entry points a
/// fully public parameter type without changing that module's visibility
/// at all — it wraps the exact same graph, never a copy or a
/// re-derivation.
pub struct InteractionTypeGraph(pub(crate) TypeGraph);

impl InteractionTypeGraph {
    /// Derives the graph for `root_type_id` from an already resolved,
    /// verified module — the same derivation
    /// `agent_interaction_schema::compile_agent_interaction_schema` performs
    /// internally, exposed here so a caller that already has a
    /// `ResolvedProgram` (for example to also call
    /// `interpreter::retained_call::prepare_retained_call` on it) does not
    /// need to re-resolve the module a second time just to get one.
    pub fn derive(program: &ResolvedProgram, root_type_id: &str) -> Result<Self, Diagnostic> {
        Ok(Self(shape::derive(program, root_type_id)?))
    }
}

/// Projects one admitted decoded interaction value into the interpreter's
/// real checked retained-call value vocabulary, recursively.
///
/// `graph` must be the exact derivation graph `value` was decoded against
/// (same root type, same source revision); it supplies the nested-type
/// identities the decoded value itself does not retain.
pub fn to_retained(
    graph: &InteractionTypeGraph,
    value: &DecodedInteractionValue,
) -> Result<RetainedValue, Diagnostic> {
    project_typed(&graph.0, value.root_type_id(), value.value())
}

fn project_typed(graph: &TypeGraph, type_id: &str, value: &TypedValue) -> Result<RetainedValue, Diagnostic> {
    let decl = graph
        .get(type_id)
        .ok_or_else(|| refusal("SPX-Z210", "projection.unknown_type"))?;
    match (&decl.shape, value) {
        (TypeShape::Record { fields: rows }, TypedValue::Record { fields }) => {
            Ok(RetainedValue::Record(RetainedRecord {
                record: DeclarationId::new(type_id),
                fields: project_fields(graph, rows, fields)?,
            }))
        }
        (TypeShape::Variant { cases }, TypedValue::Variant { case, fields }) => {
            let case_row = cases
                .iter()
                .find(|row| &row.stable_id == case)
                .ok_or_else(|| refusal("SPX-Z210", "projection.unknown_case"))?;
            Ok(RetainedValue::Variant(RetainedVariant {
                variant: DeclarationId::new(type_id),
                case: DeclarationId::new(case.clone()),
                fields: project_fields(graph, &case_row.fields, fields)?,
            }))
        }
        _ => Err(refusal("SPX-Z210", "projection.shape_mismatch")),
    }
}

fn project_fields(
    graph: &TypeGraph,
    rows: &[crate::agent_interaction_schema::shape::FieldRow],
    fields: &[DecodedField],
) -> Result<Vec<RetainedField>, Diagnostic> {
    fields
        .iter()
        .map(|field| {
            let row = rows
                .iter()
                .find(|row| row.stable_id == field.stable_id())
                .ok_or_else(|| refusal("SPX-Z210", "projection.unknown_field"))?;
            let value = match (&row.ty, field.value()) {
                (FieldType::Scalar(representation), FieldValue::Scalar(scalar)) => {
                    project_scalar(*representation, scalar)?
                }
                (FieldType::Nested(nested_type_id), FieldValue::Nested(nested)) => {
                    project_typed(graph, nested_type_id, nested)?
                }
                _ => return Err(refusal("SPX-Z210", "projection.field_shape_mismatch")),
            };
            Ok(RetainedField {
                field: DeclarationId::new(field.stable_id()),
                value,
            })
        })
        .collect()
}

fn project_scalar(representation: Representation, scalar: &ScalarValue) -> Result<RetainedValue, Diagnostic> {
    match (representation, scalar) {
        (Representation::Bool, ScalarValue::Bool(value)) => Ok(RetainedValue::Bool(*value)),
        (Representation::I32, ScalarValue::Signed(value)) => i32::try_from(*value)
            .map(RetainedValue::I32)
            .map_err(|_| refusal("SPX-Z210", "projection.integer_range")),
        (Representation::I64, ScalarValue::Signed(value)) => Ok(RetainedValue::I64(*value)),
        (Representation::U8, ScalarValue::Unsigned(value)) => u8::try_from(*value)
            .map(RetainedValue::U8)
            .map_err(|_| refusal("SPX-Z210", "projection.integer_range")),
        (Representation::U64, ScalarValue::Unsigned(value)) => Ok(RetainedValue::Usize(*value)),
        (Representation::Bytes, ScalarValue::Bytes(value)) => Ok(RetainedValue::Bytes(value.clone())),
        (Representation::Text, ScalarValue::Text(_)) => {
            Err(refusal("SPX-Z210", "projection.string_unsupported"))
        }
        _ => Err(refusal("SPX-Z210", "projection.representation_mismatch")),
    }
}
