//! Bounded, nonrecursive interaction type graph derived from checked HIR.
//!
//! A single root stable-ID record or variant declaration is resolved from
//! one checked module. Its fields may be direct scalars, bounded UTF-8 text,
//! bounded Bytes, or a reference to exactly one further monomorphic
//! record/variant declaration in the same module. Every rejection here is an
//! explicit diagnostic: an unresolved identity, a non-persistent identity, a
//! generic declaration, an empty variant, a cyclic type reference, a schema
//! that exceeds the bounded type/depth budget, and any nested, borrowed,
//! floating-point, generic-argument-bearing, or otherwise unadmitted field
//! type all fail closed here rather than being silently approximated.

use crate::diagnostic::Diagnostic;
use crate::hir::{
    DeclarationId, ResolvedFieldDeclaration, ResolvedProgram, ResolvedType,
    ResolvedTypeDeclarationKind,
};

use super::invariant;

/// The maximum number of distinct types (root plus every transitively
/// referenced record/variant) one interaction schema may contain.
pub(crate) const MAX_TYPES: usize = 64;
/// The maximum nesting depth from the root type to a transitively referenced
/// type. Depth 0 is the root itself.
pub(crate) const MAX_DEPTH: u32 = 16;
/// The maximum admitted UTF-8 byte length of one Text field value.
pub(crate) const MAX_STRING_FIELD_BYTES: usize = 4_096;
/// The maximum admitted element count of one Bytes field value.
pub(crate) const MAX_BYTES_FIELD_BYTES: usize = 4_096;

/// One admitted exact scalar wire representation, including the two bounded
/// non-nominal leaf kinds (`Text`, `Bytes`) this profile adds to the closed
/// scalar vocabulary already admitted elsewhere in the compiler.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Representation {
    Bool,
    I32,
    I64,
    U8,
    U64,
    Text,
    Bytes,
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
            Self::Bytes => "bytes",
        }
    }

    /// The inclusive decimal bounds of an exact integer representation.
    pub(crate) fn bounds(self) -> Option<(&'static str, &'static str)> {
        match self {
            Self::Bool | Self::Text | Self::Bytes => None,
            Self::I32 => Some(("-2147483648", "2147483647")),
            Self::I64 => Some(("-9223372036854775808", "9223372036854775807")),
            Self::U8 => Some(("0", "255")),
            Self::U64 => Some(("0", "18446744073709551615")),
        }
    }

    /// The declared bounded element/byte limit of a `Text` or `Bytes` field,
    /// or `None` for every other representation.
    pub(crate) fn max_bytes(self) -> Option<usize> {
        match self {
            Self::Text => Some(MAX_STRING_FIELD_BYTES),
            Self::Bytes => Some(MAX_BYTES_FIELD_BYTES),
            _ => None,
        }
    }

    fn of_scalar(ty: &ResolvedType) -> Option<Self> {
        match ty {
            ResolvedType::Bool => Some(Self::Bool),
            ResolvedType::I32 => Some(Self::I32),
            ResolvedType::I64 => Some(Self::I64),
            ResolvedType::U8 => Some(Self::U8),
            ResolvedType::Usize => Some(Self::U64),
            ResolvedType::String => Some(Self::Text),
            ResolvedType::Bytes => Some(Self::Bytes),
            ResolvedType::Function { .. }
            | ResolvedType::Unit
            | ResolvedType::Char
            | ResolvedType::ArrayU8(_)
            | ResolvedType::F32
            | ResolvedType::F64
            | ResolvedType::Str
            | ResolvedType::SliceU8
            | ResolvedType::TypeParameter { .. }
            | ResolvedType::Nominal { .. } => None,
        }
    }
}

/// One field's admitted type: a leaf scalar, or a reference to exactly one
/// other type appearing (once) in the same [`TypeGraph`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum FieldType {
    Scalar(Representation),
    Nested(String),
}

/// One closed interaction field: a persistent identity and an admitted type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FieldRow {
    pub(crate) stable_id: String,
    pub(crate) ty: FieldType,
}

/// One closed interaction variant case.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CaseRow {
    pub(crate) stable_id: String,
    pub(crate) fields: Vec<FieldRow>,
}

/// The closed shape of one type in the graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TypeShape {
    Record { fields: Vec<FieldRow> },
    Variant { cases: Vec<CaseRow> },
}

/// One derived type declaration, keyed by its persistent stable identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TypeDecl {
    pub(crate) stable_id: String,
    pub(crate) shape: TypeShape,
}

/// The complete closed, nonrecursive interaction type graph: one root type
/// plus every type it transitively references, each appearing exactly once.
///
/// Types are listed in dependency-first order (every type appears only after
/// every type it references), which is one valid deterministic topological
/// order of the underlying DAG. `root_type_id` — not position — names the
/// entry point.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TypeGraph {
    pub(crate) root_type_id: String,
    pub(crate) types: Vec<TypeDecl>,
}

impl TypeGraph {
    pub(crate) fn get(&self, stable_id: &str) -> Option<&TypeDecl> {
        self.types.iter().find(|decl| decl.stable_id == stable_id)
    }
}

/// Derives the closed, bounded interaction type graph rooted at
/// `root_type_id` from one resolved, verified module.
pub(crate) fn derive(resolved: &ResolvedProgram, root_type_id: &str) -> Result<TypeGraph, Diagnostic> {
    let mut types: Vec<TypeDecl> = Vec::new();
    let mut visiting: Vec<String> = Vec::new();
    derive_one(resolved, root_type_id, &mut types, &mut visiting, 0)?;
    Ok(TypeGraph {
        root_type_id: root_type_id.to_owned(),
        types,
    })
}

fn derive_one(
    resolved: &ResolvedProgram,
    type_id: &str,
    types: &mut Vec<TypeDecl>,
    visiting: &mut Vec<String>,
    depth: u32,
) -> Result<(), Diagnostic> {
    if types.iter().any(|decl| decl.stable_id == type_id) {
        return Ok(());
    }
    if visiting.iter().any(|id| id == type_id) {
        return Err(invariant("type.recursive"));
    }
    if depth > MAX_DEPTH {
        return Err(invariant("type.max_depth"));
    }
    if types.len() >= MAX_TYPES {
        return Err(invariant("type.max_types"));
    }
    let declaration = resolved
        .types
        .iter()
        .find(|declaration| declaration.id.as_str() == type_id)
        .ok_or_else(|| invariant("type.unresolved"))?;
    if !declaration.type_parameters.is_empty() {
        return Err(invariant("type.generic"));
    }
    persistent(resolved, type_id, "type.identity_origin")?;
    visiting.push(type_id.to_owned());
    let shape = match &declaration.kind {
        ResolvedTypeDeclarationKind::Record { fields } => TypeShape::Record {
            fields: field_rows(resolved, fields, types, visiting, depth)?,
        },
        ResolvedTypeDeclarationKind::Variant { cases } => {
            if cases.is_empty() {
                return Err(invariant("type.cases"));
            }
            let mut rows = Vec::with_capacity(cases.len());
            for case in cases {
                persistent(resolved, case.id.as_str(), "type.case.identity_origin")?;
                rows.push(CaseRow {
                    stable_id: case.id.as_str().to_owned(),
                    fields: field_rows(resolved, &case.fields, types, visiting, depth)?,
                });
            }
            TypeShape::Variant { cases: rows }
        }
        ResolvedTypeDeclarationKind::Resource { .. } | ResolvedTypeDeclarationKind::Class { .. } => {
            return Err(invariant("type.kind"));
        }
    };
    visiting.pop();
    types.push(TypeDecl {
        stable_id: type_id.to_owned(),
        shape,
    });
    Ok(())
}

fn field_rows(
    resolved: &ResolvedProgram,
    fields: &[ResolvedFieldDeclaration],
    types: &mut Vec<TypeDecl>,
    visiting: &mut Vec<String>,
    depth: u32,
) -> Result<Vec<FieldRow>, Diagnostic> {
    let mut rows = Vec::with_capacity(fields.len());
    for field in fields {
        persistent(resolved, field.id.as_str(), "type.field.identity_origin")?;
        let ty = classify(resolved, &field.ty, types, visiting, depth)?;
        rows.push(FieldRow {
            stable_id: field.id.as_str().to_owned(),
            ty,
        });
    }
    Ok(rows)
}

fn classify(
    resolved: &ResolvedProgram,
    ty: &ResolvedType,
    types: &mut Vec<TypeDecl>,
    visiting: &mut Vec<String>,
    depth: u32,
) -> Result<FieldType, Diagnostic> {
    if let Some(representation) = Representation::of_scalar(ty) {
        return Ok(FieldType::Scalar(representation));
    }
    if let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    {
        if !arguments.is_empty() {
            return Err(invariant("type.field.generic_argument"));
        }
        derive_one(resolved, declaration.as_str(), types, visiting, depth + 1)?;
        return Ok(FieldType::Nested(declaration.as_str().to_owned()));
    }
    Err(invariant("type.field.unsupported"))
}

fn persistent(resolved: &ResolvedProgram, id: &str, field: &str) -> Result<(), Diagnostic> {
    let declaration = resolved
        .declarations
        .declaration(&DeclarationId::new(id))
        .ok_or_else(|| invariant("type.unresolved"))?;
    if !declaration.identity_origin.is_persistent() {
        return Err(invariant(field));
    }
    Ok(())
}
