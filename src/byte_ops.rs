//! Compiler-owned operations over non-escaping borrowed byte slices.

use crate::ast::{Expr, ExprKind, MatchPattern, Span, Type};
use crate::hir::{DeclarationId, OwnershipMode, ResolvedParam, ResolvedType, ValueId};

pub(crate) const LEN_NAME: &str = "byte_len";
pub(crate) const GET_NAME: &str = "byte_get";
pub(crate) const LEN_ID: &str = "core.bytes.len";
pub(crate) const GET_ID: &str = "core.bytes.get";
pub(crate) const RANGE_NAME: &str = "byte_range";
pub(crate) const RANGE_ID: &str = "core.bytes.range";
pub(crate) const RANGE_STATUS_DOMAIN: &str = "semaprax.byte-range.v1";
pub(crate) const RANGE_START_AFTER_END_CODE: u32 = 1;
pub(crate) const RANGE_END_OUT_OF_BOUNDS_CODE: u32 = 2;
pub(crate) const COPY_NAME: &str = "bytes_copy";
pub(crate) const COPY_ID: &str = "core.bytes.copy";
pub(crate) const BYTES_AS_SLICE_NAME: &str = "bytes_as_slice";
pub(crate) const BYTES_AS_SLICE_ID: &str = "core.bytes.as-slice";
pub(crate) const ARRAY_AS_SLICE_NAME: &str = "array_as_slice";
pub(crate) const ARRAY_AS_SLICE_ID: &str = "core.array-u8.as-slice";
pub(crate) const STR_AS_BYTES_NAME: &str = "str_as_bytes";
pub(crate) const STR_AS_BYTES_ID: &str = "core.str.as-bytes";
pub(crate) const MAX_EXTERNAL_ROOT_BYTES: u64 = 65_536;
pub(crate) const MAX_RANGE_DEPTH: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ByteOp {
    Len,
    Get,
    Range,
    Copy,
    BytesAsSlice,
    ArrayAsSlice,
    StrAsBytes,
}

impl ByteOp {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Len => LEN_NAME,
            Self::Get => GET_NAME,
            Self::Range => RANGE_NAME,
            Self::Copy => COPY_NAME,
            Self::BytesAsSlice => BYTES_AS_SLICE_NAME,
            Self::ArrayAsSlice => ARRAY_AS_SLICE_NAME,
            Self::StrAsBytes => STR_AS_BYTES_NAME,
        }
    }
    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::Len => LEN_ID,
            Self::Get => GET_ID,
            Self::Range => RANGE_ID,
            Self::Copy => COPY_ID,
            Self::BytesAsSlice => BYTES_AS_SLICE_ID,
            Self::ArrayAsSlice => ARRAY_AS_SLICE_ID,
            Self::StrAsBytes => STR_AS_BYTES_ID,
        }
    }
    pub(crate) const fn arity(self) -> usize {
        match self {
            Self::Len => 1,
            Self::Get => 2,
            Self::Range => 3,
            Self::Copy | Self::BytesAsSlice | Self::ArrayAsSlice | Self::StrAsBytes => 1,
        }
    }
    pub(crate) fn param_types(self) -> &'static [ResolvedType] {
        match self {
            Self::Len => &[ResolvedType::SliceU8],
            Self::Get => &[ResolvedType::SliceU8, ResolvedType::Usize],
            Self::Range => &[
                ResolvedType::SliceU8,
                ResolvedType::Usize,
                ResolvedType::Usize,
            ],
            Self::Copy => &[ResolvedType::SliceU8],
            Self::BytesAsSlice => &[ResolvedType::Bytes],
            Self::ArrayAsSlice => &[ResolvedType::ArrayU8(0)],
            Self::StrAsBytes => &[ResolvedType::Str],
        }
    }
    pub(crate) fn return_type(self) -> ResolvedType {
        match self {
            Self::Len => ResolvedType::Usize,
            Self::Get => ResolvedType::Nominal {
                declaration: DeclarationId::new(crate::prelude::OPTION_ID),
                arguments: vec![ResolvedType::U8],
            },
            Self::Range => ResolvedType::SliceU8,
            Self::Copy => ResolvedType::Bytes,
            Self::BytesAsSlice | Self::ArrayAsSlice | Self::StrAsBytes => ResolvedType::SliceU8,
        }
    }
    pub(crate) fn ast_return_type(self) -> Type {
        match self {
            Self::Len => Type::Usize,
            Self::Get => Type::Named {
                name: "Option".to_owned(),
                arguments: vec![Type::U8],
            },
            Self::Range => Type::SliceU8,
            Self::Copy => Type::Bytes,
            Self::BytesAsSlice | Self::ArrayAsSlice | Self::StrAsBytes => Type::SliceU8,
        }
    }

    pub(crate) fn accepts_resolved(self, index: usize, ty: &ResolvedType) -> bool {
        match (self, index) {
            (Self::Len | Self::Copy, 0) => *ty == ResolvedType::SliceU8,
            (Self::Get, 0) => *ty == ResolvedType::SliceU8,
            (Self::Get, 1) => *ty == ResolvedType::Usize,
            (Self::Range, 0) => *ty == ResolvedType::SliceU8,
            (Self::Range, 1 | 2) => *ty == ResolvedType::Usize,
            (Self::BytesAsSlice, 0) => *ty == ResolvedType::Bytes,
            (Self::ArrayAsSlice, 0) => matches!(ty, ResolvedType::ArrayU8(_)),
            (Self::StrAsBytes, 0) => *ty == ResolvedType::Str,
            _ => false,
        }
    }

    pub(crate) fn accepts_ast(self, index: usize, ty: &Type) -> bool {
        match (self, index) {
            (Self::Len | Self::Copy, 0) => *ty == Type::SliceU8,
            (Self::Get, 0) => *ty == Type::SliceU8,
            (Self::Get, 1) => *ty == Type::Usize,
            (Self::Range, 0) => *ty == Type::SliceU8,
            (Self::Range, 1 | 2) => *ty == Type::Usize,
            (Self::BytesAsSlice, 0) => *ty == Type::Bytes,
            (Self::ArrayAsSlice, 0) => matches!(ty, Type::ArrayU8(_)),
            (Self::StrAsBytes, 0) => *ty == Type::Str,
            _ => false,
        }
    }

    pub(crate) const fn is_view(self) -> bool {
        matches!(
            self,
            Self::BytesAsSlice | Self::ArrayAsSlice | Self::StrAsBytes
        )
    }
}

pub(crate) fn by_name(name: &str) -> Option<ByteOp> {
    match name {
        LEN_NAME => Some(ByteOp::Len),
        GET_NAME => Some(ByteOp::Get),
        RANGE_NAME => Some(ByteOp::Range),
        COPY_NAME => Some(ByteOp::Copy),
        BYTES_AS_SLICE_NAME => Some(ByteOp::BytesAsSlice),
        ARRAY_AS_SLICE_NAME => Some(ByteOp::ArrayAsSlice),
        STR_AS_BYTES_NAME => Some(ByteOp::StrAsBytes),
        _ => None,
    }
}
pub(crate) fn by_id(id: &str) -> Option<ByteOp> {
    match id {
        LEN_ID => Some(ByteOp::Len),
        GET_ID => Some(ByteOp::Get),
        RANGE_ID => Some(ByteOp::Range),
        COPY_ID => Some(ByteOp::Copy),
        BYTES_AS_SLICE_ID => Some(ByteOp::BytesAsSlice),
        ARRAY_AS_SLICE_ID => Some(ByteOp::ArrayAsSlice),
        STR_AS_BYTES_ID => Some(ByteOp::StrAsBytes),
        _ => None,
    }
}

/// Indexed Byte Loop v2 source-shape gate. This deliberately recognizes only
/// the reserved `byte_get` spelling and the complete, guard-free `Option<u8>`
/// case inventory. Resolution and hostile-HIR validation then authenticate the
/// corresponding compiler-owned identities and concrete types.
pub(crate) fn is_indexed_byte_option_match_source(expression: &Expr) -> bool {
    let ExprKind::Match { scrutinee, arms } = &expression.kind else {
        return false;
    };
    let ExprKind::Call {
        name,
        type_arguments,
        args,
    } = &scrutinee.kind
    else {
        return false;
    };
    if by_name(name) != Some(ByteOp::Get)
        || !type_arguments.is_empty()
        || args.len() != ByteOp::Get.arity()
        || arms.len() != 2
    {
        return false;
    }

    let mut some_seen = false;
    let mut none_seen = false;
    for arm in arms {
        if arm.guard.is_some() {
            return false;
        }
        let MatchPattern::Variant {
            type_name,
            case_name,
            fields,
            ..
        } = &arm.pattern
        else {
            return false;
        };
        if type_name != "Option" {
            return false;
        }
        match case_name.as_str() {
            "Some" if !some_seen && fields.len() == 1 && fields[0].name == "value" => {
                some_seen = true;
            }
            "None" if !none_seen && fields.is_empty() => {
                none_seen = true;
            }
            _ => return false,
        }
    }
    some_seen && none_seen
}

pub(crate) fn resolved_params(op: ByteOp) -> Vec<ResolvedParam> {
    op.param_types()
        .iter()
        .enumerate()
        .map(|(index, ty)| ResolvedParam {
            id: ValueId::intrinsic_parameter(op.id(), index),
            name: match (op, index) {
                (_, 0) => "value",
                (ByteOp::Range, 1) => "start",
                (ByteOp::Range, 2) => "end",
                _ => "index",
            }
            .to_owned(),
            ownership: if index == 0 {
                OwnershipMode::Borrow
            } else {
                OwnershipMode::Value
            },
            ty: ty.clone(),
            span: Span::default(),
        })
        .collect()
}
