//! Compiler-owned operations over non-escaping borrowed byte and UTF-8 views,
//! plus the Owned Bounded Byte Buffer v1 allocate-then-fill pair.
//!
//! `bytes_zeroed` and `bytes_set` build one write-once owned buffer. The
//! capacity is a literal at the single allocation site, every element index is
//! a literal strictly below that capacity, and the buffer operand of a
//! `bytes_set` is syntactically the enclosing chain's previous link. A filled
//! buffer therefore has exactly one owner, no observable intermediate state,
//! and one destruction path; binding the chain's result is the freeze, after
//! which the ordinary borrowed reads apply.

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
/// Owned Bounded Byte Buffer v1 runtime failure domain. `bytes_set` admits a
/// computed `usize` index, so an index at or above the buffer's length is a
/// selected operation failure rather than a backend accident. The domain is
/// separate from `semaprax.byte-range.v1` because it belongs to the owned
/// buffer family rather than to borrowed sub-view construction.
pub(crate) const SET_STATUS_DOMAIN: &str = "semaprax.byte-buffer.v1";
/// The single admitted `semaprax.byte-buffer.v1` code.
pub(crate) const SET_INDEX_OUT_OF_BOUNDS_CODE: u32 = 1;
pub(crate) const COPY_NAME: &str = "bytes_copy";
pub(crate) const COPY_ID: &str = "core.bytes.copy";
pub(crate) const BYTES_AS_SLICE_NAME: &str = "bytes_as_slice";
pub(crate) const BYTES_AS_SLICE_ID: &str = "core.bytes.as-slice";
pub(crate) const ARRAY_AS_SLICE_NAME: &str = "array_as_slice";
pub(crate) const ARRAY_AS_SLICE_ID: &str = "core.array-u8.as-slice";
pub(crate) const STR_AS_BYTES_NAME: &str = "str_as_bytes";
pub(crate) const STR_AS_BYTES_ID: &str = "core.str.as-bytes";
pub(crate) const STRING_AS_STR_NAME: &str = "string_as_str";
pub(crate) const STRING_AS_STR_ID: &str = "core.string.as-str";
pub(crate) const ZEROED_NAME: &str = "bytes_zeroed";
pub(crate) const ZEROED_ID: &str = "core.bytes.zeroed";
pub(crate) const SET_NAME: &str = "bytes_set";
pub(crate) const SET_ID: &str = "core.bytes.set";
pub(crate) const MAX_EXTERNAL_ROOT_BYTES: u64 = 65_536;
pub(crate) const MAX_RANGE_DEPTH: usize = 64;
/// Owned Bounded Byte Buffer v1 capacity ceiling. One buffer never exceeds the
/// established owned byte payload extent of a single allocation site.
pub(crate) const MAX_BUFFER_CAPACITY_BYTES: u64 = MAX_EXTERNAL_ROOT_BYTES;
/// Owned Bounded Byte Buffer v1 fill ceiling. The chain is unrolled source, so
/// its length is bounded to keep resolution, verification, cleanup planning and
/// both backends linear in an explicitly stated budget.
pub(crate) const MAX_BUFFER_FILL_SITES: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ByteOp {
    Len,
    Get,
    Range,
    Copy,
    BytesAsSlice,
    ArrayAsSlice,
    StrAsBytes,
    StringAsStr,
    /// Owned Bounded Byte Buffer v1: allocate one zeroed owned buffer whose
    /// capacity is a literal at this allocation site.
    Zeroed,
    /// Owned Bounded Byte Buffer v1: consume the buffer, store one byte at a
    /// literal index below the chain capacity, and return the same owner.
    Set,
}

impl ByteOp {
    pub(crate) const ALL: [Self; 10] = [
        Self::Len,
        Self::Get,
        Self::Range,
        Self::Copy,
        Self::BytesAsSlice,
        Self::ArrayAsSlice,
        Self::StrAsBytes,
        Self::StringAsStr,
        Self::Zeroed,
        Self::Set,
    ];

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Len => LEN_NAME,
            Self::Get => GET_NAME,
            Self::Range => RANGE_NAME,
            Self::Copy => COPY_NAME,
            Self::BytesAsSlice => BYTES_AS_SLICE_NAME,
            Self::ArrayAsSlice => ARRAY_AS_SLICE_NAME,
            Self::StrAsBytes => STR_AS_BYTES_NAME,
            Self::StringAsStr => STRING_AS_STR_NAME,
            Self::Zeroed => ZEROED_NAME,
            Self::Set => SET_NAME,
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
            Self::StringAsStr => STRING_AS_STR_ID,
            Self::Zeroed => ZEROED_ID,
            Self::Set => SET_ID,
        }
    }
    pub(crate) const fn arity(self) -> usize {
        match self {
            Self::Len => 1,
            Self::Get => 2,
            Self::Range | Self::Set => 3,
            Self::Copy
            | Self::BytesAsSlice
            | Self::ArrayAsSlice
            | Self::StrAsBytes
            | Self::StringAsStr
            | Self::Zeroed => 1,
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
            Self::StringAsStr => &[ResolvedType::String],
            Self::Zeroed => &[ResolvedType::Usize],
            Self::Set => &[ResolvedType::Bytes, ResolvedType::Usize, ResolvedType::U8],
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
            Self::Copy | Self::Zeroed | Self::Set => ResolvedType::Bytes,
            Self::BytesAsSlice | Self::ArrayAsSlice | Self::StrAsBytes => ResolvedType::SliceU8,
            Self::StringAsStr => ResolvedType::Str,
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
            Self::Copy | Self::Zeroed | Self::Set => Type::Bytes,
            Self::BytesAsSlice | Self::ArrayAsSlice | Self::StrAsBytes => Type::SliceU8,
            Self::StringAsStr => Type::Str,
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
            (Self::StringAsStr, 0) => *ty == ResolvedType::String,
            (Self::Zeroed, 0) => *ty == ResolvedType::Usize,
            (Self::Set, 0) => *ty == ResolvedType::Bytes,
            (Self::Set, 1) => *ty == ResolvedType::Usize,
            (Self::Set, 2) => *ty == ResolvedType::U8,
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
            (Self::StringAsStr, 0) => *ty == Type::String,
            (Self::Zeroed, 0) => *ty == Type::Usize,
            (Self::Set, 0) => *ty == Type::Bytes,
            (Self::Set, 1) => *ty == Type::Usize,
            (Self::Set, 2) => *ty == Type::U8,
            _ => false,
        }
    }

    pub(crate) const fn is_view(self) -> bool {
        matches!(
            self,
            Self::BytesAsSlice | Self::ArrayAsSlice | Self::StrAsBytes | Self::StringAsStr
        )
    }

    /// `true` for the Owned Bounded Byte Buffer v1 write-once chain links.
    pub(crate) const fn is_owned_buffer_chain(self) -> bool {
        matches!(self, Self::Zeroed | Self::Set)
    }

    /// `true` for the one byte operation that can select a runtime failure.
    ///
    /// `bytes_set` admits a computed element index, so the store is checked
    /// against the transferred buffer's length before the owner is committed.
    /// Every other operation in this family is total after HIR admission, and
    /// physical allocation failure stays invariant fail-stop.
    pub(crate) const fn is_fallible(self) -> bool {
        matches!(self, Self::Set)
    }

    /// Source parameter names in left-to-right order. They label diagnostics
    /// and synthetic parameters; the operations have no authored declaration.
    pub(crate) const fn param_names(self) -> &'static [&'static str] {
        match self {
            Self::Get => &["value", "index"],
            Self::Range => &["value", "start", "end"],
            Self::Len
            | Self::Copy
            | Self::BytesAsSlice
            | Self::ArrayAsSlice
            | Self::StrAsBytes
            | Self::StringAsStr => &["value"],
            Self::Zeroed => &["count"],
            Self::Set => &["buffer", "index", "value"],
        }
    }

    /// Canonical argument ownership. The first operand of a borrowed view or
    /// read is borrowed, `bytes_set` transfers its buffer, and every scalar
    /// operand is an ordinary copied value.
    pub(crate) const fn param_ownership(self, index: usize) -> OwnershipMode {
        match (self, index) {
            (Self::Set, 0) => OwnershipMode::Own,
            (Self::Zeroed, _) | (Self::Set, _) => OwnershipMode::Value,
            (_, 0) => OwnershipMode::Borrow,
            _ => OwnershipMode::Value,
        }
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
        STRING_AS_STR_NAME => Some(ByteOp::StringAsStr),
        ZEROED_NAME => Some(ByteOp::Zeroed),
        SET_NAME => Some(ByteOp::Set),
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
        STRING_AS_STR_ID => Some(ByteOp::StringAsStr),
        ZEROED_ID => Some(ByteOp::Zeroed),
        SET_ID => Some(ByteOp::Set),
        _ => None,
    }
}

/// Indexed Byte Loop v2 source-shape gate. This deliberately recognizes only
/// the reserved `byte_get` spelling and the complete, guard-free `Option<u8>`
/// case inventory. Resolution and hostile-HIR validation then authenticate the
/// corresponding compiler-owned identities and concrete types.
pub(crate) fn is_indexed_byte_option_match_source(expression: &Expr) -> bool {
    let ExprKind::Match {
        scrutinee, arms, ..
    } = &expression.kind
    else {
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
        .zip(op.param_names())
        .enumerate()
        .map(|(index, (ty, name))| ResolvedParam {
            id: ValueId::intrinsic_parameter(op.id(), index),
            name: (*name).to_owned(),
            ownership: op.param_ownership(index),
            ty: ty.clone(),
            span: Span::default(),
        })
        .collect()
}

/// Owned Bounded Byte Buffer v1 capacity of one source-level write-once chain.
///
/// A chain is exactly `bytes_zeroed(<usize literal>)` optionally wrapped in
/// `bytes_set(<chain>, <usize literal>, <u8 literal or expression>)` links. The
/// capacity is the literal at the single allocation site; `None` means the
/// expression is not an admitted chain and no `bytes_set` may consume it.
pub(crate) fn owned_buffer_chain_capacity(expression: &Expr) -> Option<u64> {
    let mut links = 0usize;
    let mut current = expression;
    loop {
        let ExprKind::Call {
            name,
            type_arguments,
            args,
        } = &current.kind
        else {
            return None;
        };
        let op = by_name(name)?;
        if !type_arguments.is_empty() || args.len() != op.arity() {
            return None;
        }
        match op {
            ByteOp::Zeroed => {
                let ExprKind::Usize(capacity) = &args[0].kind else {
                    return None;
                };
                return (*capacity <= MAX_BUFFER_CAPACITY_BYTES).then_some(*capacity);
            }
            ByteOp::Set => {
                links += 1;
                if links > MAX_BUFFER_FILL_SITES {
                    return None;
                }
                current = &args[0];
            }
            _ => return None,
        }
    }
}

/// The literal element index of one `bytes_set` call, or `None` when the index
/// operand is not a `usize` literal.
pub(crate) fn owned_buffer_set_index(args: &[Expr]) -> Option<u64> {
    match args.get(1).map(|argument| &argument.kind) {
        Some(ExprKind::Usize(index)) => Some(*index),
        _ => None,
    }
}
