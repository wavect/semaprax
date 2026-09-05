//! Compiler-owned string operation intrinsics.
//!
//! Three prelude-style intrinsic functions from the first wave, four from
//! the second bounded wave, and two numeric-text conversions are admitted wherever owned `string` values are
//! already admitted. They carry reserved stable identities in the
//! compiler-owned `core.string.*` family so a call site resolves to an
//! ordinary monomorphic [`crate::hir::ResolvedExprKind::Call`] node; no
//! source declaration, prelude contract byte, or graph schema version
//! participates, so programs that never name the operations keep
//! byte-identical projections.
//!
//! First wave (gated as one helper/import group):
//!
//! - `string_len(s: string) -> i64` borrows its operand for a read.
//! - `string_concat(a: string, b: string) -> string` consumes both operands
//!   by move and returns one new owned string.
//! - `string_is_empty(s: string) -> bool` borrows its operand for a read.
//!
//! Second wave (breadth v2, gated as its own helper/import group so first
//! wave programs keep their exact committed bytes):
//!
//! - `string_starts_with(s: string, prefix: string) -> bool` borrows both.
//! - `string_contains(s: string, needle: string) -> bool` borrows both.
//! - `string_len_chars(s: string) -> i64` counts Unicode scalar values,
//!   borrowing its operand for a read.
//! - `string_from_char(c: char) -> string` consumes nothing and returns one
//!   new owned string holding the scalar value's UTF-8 encoding.
//!
//! Numeric text wave (gated separately to preserve all earlier target bytes):
//!
//! - `string_from_i64(value: i64) -> string` renders canonical decimal text.
//! - `string_from_usize(value: usize) -> string` renders canonical decimal text.

use crate::ast::{Param, ParamMode, Span, Type};
use crate::hir::{OwnershipMode, ResolvedParam, ResolvedType, ValueId};

pub(crate) const LEN_NAME: &str = "string_len";
pub(crate) const CONCAT_NAME: &str = "string_concat";
pub(crate) const IS_EMPTY_NAME: &str = "string_is_empty";
pub(crate) const STARTS_WITH_NAME: &str = "string_starts_with";
pub(crate) const CONTAINS_NAME: &str = "string_contains";
pub(crate) const LEN_CHARS_NAME: &str = "string_len_chars";
pub(crate) const FROM_CHAR_NAME: &str = "string_from_char";
pub(crate) const FROM_I64_NAME: &str = "string_from_i64";
pub(crate) const FROM_USIZE_NAME: &str = "string_from_usize";

pub(crate) const LEN_ID: &str = "core.string.len";
pub(crate) const CONCAT_ID: &str = "core.string.concat";
pub(crate) const IS_EMPTY_ID: &str = "core.string.is_empty";
pub(crate) const STARTS_WITH_ID: &str = "core.string.starts_with";
pub(crate) const CONTAINS_ID: &str = "core.string.contains";
pub(crate) const LEN_CHARS_ID: &str = "core.string.len_chars";
pub(crate) const FROM_CHAR_ID: &str = "core.string.from_char";
pub(crate) const FROM_I64_ID: &str = "core.string.from_i64";
pub(crate) const FROM_USIZE_ID: &str = "core.string.from_usize";

/// One admitted string operation intrinsic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StringOp {
    /// Borrowed byte-length read.
    Len,
    /// Consuming concatenation of two owned strings.
    Concat,
    /// Borrowed emptiness read.
    IsEmpty,
    /// Borrowed prefix test over both operands.
    StartsWith,
    /// Borrowed substring test over both operands.
    Contains,
    /// Borrowed Unicode scalar-value count.
    LenChars,
    /// Allocation of one owned string from a copied scalar value.
    FromChar,
    /// Canonical decimal text for one copied signed integer.
    FromI64,
    /// Canonical decimal text for one copied portable size value.
    FromUsize,
}

impl StringOp {
    pub(crate) const ALL: [Self; 9] = [
        Self::Len,
        Self::Concat,
        Self::IsEmpty,
        Self::StartsWith,
        Self::Contains,
        Self::LenChars,
        Self::FromChar,
        Self::FromI64,
        Self::FromUsize,
    ];

    pub(crate) fn name(self) -> &'static str {
        match self {
            StringOp::Len => LEN_NAME,
            StringOp::Concat => CONCAT_NAME,
            StringOp::IsEmpty => IS_EMPTY_NAME,
            StringOp::StartsWith => STARTS_WITH_NAME,
            StringOp::Contains => CONTAINS_NAME,
            StringOp::LenChars => LEN_CHARS_NAME,
            StringOp::FromChar => FROM_CHAR_NAME,
            StringOp::FromI64 => FROM_I64_NAME,
            StringOp::FromUsize => FROM_USIZE_NAME,
        }
    }

    pub(crate) fn id(self) -> &'static str {
        match self {
            StringOp::Len => LEN_ID,
            StringOp::Concat => CONCAT_ID,
            StringOp::IsEmpty => IS_EMPTY_ID,
            StringOp::StartsWith => STARTS_WITH_ID,
            StringOp::Contains => CONTAINS_ID,
            StringOp::LenChars => LEN_CHARS_ID,
            StringOp::FromChar => FROM_CHAR_ID,
            StringOp::FromI64 => FROM_I64_ID,
            StringOp::FromUsize => FROM_USIZE_ID,
        }
    }

    pub(crate) fn arity(self) -> usize {
        match self {
            StringOp::Len | StringOp::IsEmpty | StringOp::LenChars => 1,
            StringOp::FromChar | StringOp::FromI64 | StringOp::FromUsize => 1,
            StringOp::Concat | StringOp::StartsWith | StringOp::Contains => 2,
        }
    }

    /// Source parameter names in left-to-right order; they only label
    /// diagnostics because the operations have no authored declaration.
    pub(crate) fn param_names(self) -> &'static [&'static str] {
        match self {
            StringOp::Len | StringOp::IsEmpty | StringOp::LenChars => &["s"],
            StringOp::FromChar => &["c"],
            StringOp::FromI64 | StringOp::FromUsize => &["value"],
            StringOp::Concat => &["a", "b"],
            StringOp::StartsWith => &["s", "prefix"],
            StringOp::Contains => &["s", "needle"],
        }
    }

    /// Resolved parameter types in left-to-right order. Every operation takes
    /// `string` operands except the copied-scalar constructors.
    pub(crate) fn param_types(self) -> &'static [ResolvedType] {
        match self {
            StringOp::Len | StringOp::IsEmpty | StringOp::LenChars => &[ResolvedType::String],
            StringOp::FromChar => &[ResolvedType::Char],
            StringOp::FromI64 => &[ResolvedType::I64],
            StringOp::FromUsize => &[ResolvedType::Usize],
            StringOp::Concat | StringOp::StartsWith | StringOp::Contains => {
                &[ResolvedType::String, ResolvedType::String]
            }
        }
    }

    pub(crate) fn consumes_arguments(self) -> bool {
        matches!(self, StringOp::Concat)
    }

    /// Whether the operation belongs to the breadth-v2 wave. Its native
    /// helpers and Wasm host imports gate as one separate group so programs
    /// that reach only first-wave operations keep their exact bytes.
    pub(crate) fn is_breadth_v2(self) -> bool {
        matches!(
            self,
            StringOp::StartsWith | StringOp::Contains | StringOp::LenChars | StringOp::FromChar
        )
    }

    /// Numeric-to-text operations form a third optional backend group so
    /// programs using either earlier wave retain byte-identical artifacts.
    pub(crate) fn is_numeric_text(self) -> bool {
        matches!(self, StringOp::FromI64 | StringOp::FromUsize)
    }

    pub(crate) fn return_type(self) -> ResolvedType {
        match self {
            StringOp::Len | StringOp::LenChars => ResolvedType::I64,
            StringOp::Concat | StringOp::FromChar | StringOp::FromI64 | StringOp::FromUsize => {
                ResolvedType::String
            }
            StringOp::IsEmpty | StringOp::StartsWith | StringOp::Contains => ResolvedType::Bool,
        }
    }

    pub(crate) fn ast_return_type(self) -> Type {
        match self {
            StringOp::Len | StringOp::LenChars => Type::I64,
            StringOp::Concat | StringOp::FromChar | StringOp::FromI64 | StringOp::FromUsize => {
                Type::String
            }
            StringOp::IsEmpty | StringOp::StartsWith | StringOp::Contains => Type::Bool,
        }
    }
}

/// Resolve a source-level call name to its intrinsic operation.
pub(crate) fn by_name(name: &str) -> Option<StringOp> {
    match name {
        LEN_NAME => Some(StringOp::Len),
        CONCAT_NAME => Some(StringOp::Concat),
        IS_EMPTY_NAME => Some(StringOp::IsEmpty),
        STARTS_WITH_NAME => Some(StringOp::StartsWith),
        CONTAINS_NAME => Some(StringOp::Contains),
        LEN_CHARS_NAME => Some(StringOp::LenChars),
        FROM_CHAR_NAME => Some(StringOp::FromChar),
        FROM_I64_NAME => Some(StringOp::FromI64),
        FROM_USIZE_NAME => Some(StringOp::FromUsize),
        _ => None,
    }
}

/// Resolve a resolved-callee identity to its intrinsic operation.
pub(crate) fn by_id(id: &str) -> Option<StringOp> {
    match id {
        LEN_ID => Some(StringOp::Len),
        CONCAT_ID => Some(StringOp::Concat),
        IS_EMPTY_ID => Some(StringOp::IsEmpty),
        STARTS_WITH_ID => Some(StringOp::StartsWith),
        CONTAINS_ID => Some(StringOp::Contains),
        LEN_CHARS_ID => Some(StringOp::LenChars),
        FROM_CHAR_ID => Some(StringOp::FromChar),
        FROM_I64_ID => Some(StringOp::FromI64),
        FROM_USIZE_ID => Some(StringOp::FromUsize),
        _ => None,
    }
}

/// Synthetic HIR parameters for one operation: consuming arguments carry
/// `Own` ownership exactly like an ordinary declared `string` parameter,
/// borrowed arguments accept every argument ownership without a transfer,
/// and copied scalar arguments use the ordinary `Value` mode of their kind.
pub(crate) fn resolved_params(op: StringOp) -> Vec<ResolvedParam> {
    let consumption = if op.consumes_arguments() {
        OwnershipMode::Own
    } else {
        OwnershipMode::Borrow
    };
    op.param_names()
        .iter()
        .zip(op.param_types())
        .enumerate()
        .map(|(index, (name, ty))| ResolvedParam {
            id: ValueId::intrinsic_parameter(op.id(), index),
            name: (*name).to_owned(),
            ownership: if matches!(
                ty,
                ResolvedType::Char | ResolvedType::I64 | ResolvedType::Usize
            ) {
                OwnershipMode::Value
            } else {
                consumption
            },
            ty: ty.clone(),
            span: Span::default(),
        })
        .collect()
}

/// Synthetic AST parameters for source verification. Consuming arguments use
/// the established `own` transfer mode; borrowed arguments and copied scalars
/// use the plain value mode that never marks its sources moved.
pub(crate) fn ast_params(op: StringOp) -> Vec<Param> {
    op.param_names()
        .iter()
        .zip(op.param_types())
        .map(|(name, ty)| Param {
            name: (*name).to_owned(),
            mode: if matches!(
                ty,
                ResolvedType::Char | ResolvedType::I64 | ResolvedType::Usize
            ) || !op.consumes_arguments()
            {
                ParamMode::Value
            } else {
                ParamMode::Own
            },
            ty: match ty {
                ResolvedType::Char => Type::Char,
                ResolvedType::I64 => Type::I64,
                ResolvedType::Usize => Type::Usize,
                _ => Type::String,
            },
            span: Span::default(),
        })
        .collect()
}
