//! Proposal shape derived from verified stable-ID record and variant types.
//!
//! The shape carries semantic identities and exact scalar representations
//! only. Display names never reach it, so a display rename preserves the
//! derived revision while an actual type change invalidates it.

use crate::diagnostic::{quote_json, Diagnostic};
use crate::hir::{
    DeclarationId, ResolvedFieldDeclaration, ResolvedProgram, ResolvedType,
    ResolvedTypeDeclarationKind,
};

use super::{invariant, MAX_STRING_FIELD_BYTES};

/// One admitted exact scalar wire representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Representation {
    Bool,
    I32,
    I64,
    U8,
    U64,
    Text,
}

impl Representation {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::U8 => "u8",
            Self::U64 => "u64",
            Self::Text => "string",
        }
    }

    /// The inclusive decimal bounds of an exact integer representation.
    pub(crate) fn bounds(self) -> Option<(&'static str, &'static str)> {
        match self {
            Self::Bool | Self::Text => None,
            Self::I32 => Some(("-2147483648", "2147483647")),
            Self::I64 => Some(("-9223372036854775808", "9223372036854775807")),
            Self::U8 => Some(("0", "255")),
            Self::U64 => Some(("0", "18446744073709551615")),
        }
    }

    fn of(ty: &ResolvedType) -> Option<Self> {
        match ty {
            ResolvedType::Bool => Some(Self::Bool),
            ResolvedType::I32 => Some(Self::I32),
            ResolvedType::I64 => Some(Self::I64),
            ResolvedType::U8 => Some(Self::U8),
            ResolvedType::Usize => Some(Self::U64),
            ResolvedType::String => Some(Self::Text),
            ResolvedType::Unit
            | ResolvedType::Char
            | ResolvedType::ArrayU8(_)
            | ResolvedType::F32
            | ResolvedType::F64
            | ResolvedType::Bytes
            | ResolvedType::Str
            | ResolvedType::SliceU8
            | ResolvedType::TypeParameter { .. }
            | ResolvedType::Nominal { .. } => None,
        }
    }
}

/// One closed proposal field: a persistent identity and an exact scalar.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FieldRow {
    pub(crate) stable_id: String,
    pub(crate) representation: Representation,
}

/// One closed proposal variant case.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CaseRow {
    pub(crate) stable_id: String,
    pub(crate) fields: Vec<FieldRow>,
}

/// The complete closed proposal shape.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Shape {
    Record { fields: Vec<FieldRow> },
    Variant { cases: Vec<CaseRow> },
}

impl Shape {
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Self::Record { .. } => "record",
            Self::Variant { .. } => "variant",
        }
    }
}

/// Resolves the Proposal role to one verified record or variant declaration
/// and derives its closed schema shape.
///
/// Every rejection is an explicit diagnostic: an unresolved identity, a
/// non-persistent identity, a generic or non-record/variant declaration, an
/// empty variant, and any nested, borrowed, floating-point, byte-owning, or
/// otherwise unadmitted field type all fail closed here.
pub(crate) fn derive(
    resolved: &ResolvedProgram,
    proposal_type_id: &str,
) -> Result<Shape, Diagnostic> {
    let declaration = resolved
        .types
        .iter()
        .find(|declaration| declaration.id.as_str() == proposal_type_id)
        .ok_or_else(|| invariant("proposal_type.unresolved"))?;
    if !declaration.type_parameters.is_empty() {
        return Err(invariant("proposal_type.generic"));
    }
    persistent(resolved, proposal_type_id, "proposal_type.identity_origin")?;
    match &declaration.kind {
        ResolvedTypeDeclarationKind::Record { fields } => Ok(Shape::Record {
            fields: field_rows(resolved, fields)?,
        }),
        ResolvedTypeDeclarationKind::Variant { cases } => {
            if cases.is_empty() {
                return Err(invariant("proposal_type.cases"));
            }
            let mut rows = Vec::with_capacity(cases.len());
            for case in cases {
                persistent(
                    resolved,
                    case.id.as_str(),
                    "proposal_type.case.identity_origin",
                )?;
                rows.push(CaseRow {
                    stable_id: case.id.as_str().to_owned(),
                    fields: field_rows(resolved, &case.fields)?,
                });
            }
            Ok(Shape::Variant { cases: rows })
        }
        ResolvedTypeDeclarationKind::Resource { .. }
        | ResolvedTypeDeclarationKind::Class { .. } => Err(invariant("proposal_type.kind")),
    }
}

fn field_rows(
    resolved: &ResolvedProgram,
    fields: &[ResolvedFieldDeclaration],
) -> Result<Vec<FieldRow>, Diagnostic> {
    let mut rows = Vec::with_capacity(fields.len());
    for field in fields {
        persistent(
            resolved,
            field.id.as_str(),
            "proposal_type.field.identity_origin",
        )?;
        let representation =
            Representation::of(&field.ty).ok_or_else(|| invariant("proposal_type.field.type"))?;
        rows.push(FieldRow {
            stable_id: field.id.as_str().to_owned(),
            representation,
        });
    }
    Ok(rows)
}

fn persistent(resolved: &ResolvedProgram, id: &str, field: &str) -> Result<(), Diagnostic> {
    let declaration = resolved
        .declarations
        .declaration(&DeclarationId::new(id))
        .ok_or_else(|| invariant("proposal_type.unresolved"))?;
    if !declaration.identity_origin.is_persistent() {
        return Err(invariant(field));
    }
    Ok(())
}

/// Renders the canonical `shape` member. Rows carry identities and exact
/// representations only, never display names.
pub(crate) fn render(shape: &Shape) -> String {
    let mut output = format!("{{\"kind\":{}", quote_json(shape.kind()));
    match shape {
        Shape::Record { fields } => {
            output.push_str(",\"fields\":");
            output.push_str(&render_fields(fields));
        }
        Shape::Variant { cases } => {
            output.push_str(",\"cases\":[");
            for (index, case) in cases.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                output.push_str(&format!(
                    "{{\"stable_id\":{},\"fields\":{}}}",
                    quote_json(&case.stable_id),
                    render_fields(&case.fields)
                ));
            }
            output.push(']');
        }
    }
    output.push('}');
    output
}

fn render_fields(fields: &[FieldRow]) -> String {
    let mut output = String::from("[");
    for (index, field) in fields.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&format!(
            "{{\"stable_id\":{},\"representation\":{}",
            quote_json(&field.stable_id),
            quote_json(field.representation.name())
        ));
        match field.representation.bounds() {
            Some((minimum, maximum)) => output.push_str(&format!(
                ",\"minimum\":{},\"maximum\":{}",
                quote_json(minimum),
                quote_json(maximum)
            )),
            None => {
                if field.representation == Representation::Text {
                    output.push_str(&format!(",\"max_bytes\":{MAX_STRING_FIELD_BYTES}"));
                }
            }
        }
        output.push('}');
    }
    output.push(']');
    output
}
