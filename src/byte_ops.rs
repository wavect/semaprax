//! Compiler-owned operations over non-escaping borrowed byte and UTF-8 views,
//! plus the Owned Bounded Byte Buffer v1 allocate-then-fill pair.
//!
//! `bytes_zeroed`, `bytes_set`, and `bytes_set5` build one write-once owned buffer. The
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
pub(crate) const SET5_NAME: &str = "bytes_set5";
pub(crate) const SET5_ID: &str = "core.bytes.set5";
pub(crate) const SET5_WIDTH: u64 = 5;
pub(crate) const SET1_OR5_NAME: &str = "bytes_set1_or5_from_slice";
pub(crate) const SET1_OR5_ID: &str = "core.bytes.set1_or5_from_slice";
pub(crate) const SET1_OR6_OR48_NAME: &str = "bytes_set1_or6_or48_from_slice";
pub(crate) const SET1_OR6_OR48_ID: &str = "core.bytes.set1_or6_or48_from_slice";
/// The high bit selects the five-byte source-read path. The lower 63 bits are
/// its source offset. A clear tag selects the one-byte supplied-value path.
pub(crate) const SET1_OR5_WIDE_TAG: u64 = 1_u64 << 63;
/// The high bit selects a borrowed-slice copy. Bit 62 distinguishes a
/// forty-eight-byte control run from a six-byte control escape; low 62 bits
/// select the source offset. A clear high bit selects the scalar store.
pub(crate) const SET1_OR6_OR48_COPY_TAG: u64 = 1_u64 << 63;
pub(crate) const SET1_OR6_OR48_WIDE48_TAG: u64 = 1_u64 << 62;
pub(crate) const SET1_OR6_OR48_OFFSET_MASK: u64 = (1_u64 << 62) - 1;
/// Maximum owned `Bytes` payload. This is deliberately larger than one
/// borrowed external root: an internal producer may return a bounded result
/// that expands a valid 64 KiB request without widening the input carrier.
pub(crate) const MAX_OWNED_BYTE_VALUE_BYTES: u64 = 131_072;
pub(crate) const MAX_EXTERNAL_ROOT_BYTES: u64 = 65_536;
pub(crate) const MAX_RANGE_DEPTH: usize = 64;
/// Owned Bounded Byte Buffer v1 capacity ceiling. One buffer never exceeds the
/// established owned byte payload extent of a single allocation site.
pub(crate) const MAX_BUFFER_CAPACITY_BYTES: u64 = MAX_OWNED_BYTE_VALUE_BYTES;
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
    /// Owned Bounded Byte Buffer v2: consume the buffer, preflight five
    /// contiguous elements, and return the same owner after ordered stores.
    Set5,
    /// Internal same-owner tagged one-or-five store: the tag's high bit selects
    /// a five-byte borrowed-slice read and its low bits select the source offset.
    Set1Or5,
    /// Internal same-owner tagged source store for canonical six-byte JSON
    /// control escapes and eight-atom forty-eight-byte control runs.
    Set1Or6Or48,
}

impl ByteOp {
    pub(crate) const ALL: [Self; 13] = [
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
        Self::Set5,
        Self::Set1Or5,
        Self::Set1Or6Or48,
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
            Self::Set5 => SET5_NAME,
            Self::Set1Or5 => SET1_OR5_NAME,
            Self::Set1Or6Or48 => SET1_OR6_OR48_NAME,
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
            Self::Set5 => SET5_ID,
            Self::Set1Or5 => SET1_OR5_ID,
            Self::Set1Or6Or48 => SET1_OR6_OR48_ID,
        }
    }
    pub(crate) const fn arity(self) -> usize {
        match self {
            Self::Len => 1,
            Self::Get => 2,
            Self::Range | Self::Set => 3,
            Self::Set5 => 7,
            Self::Set1Or5 | Self::Set1Or6Or48 => 5,
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
            Self::Set5 => &[
                ResolvedType::Bytes,
                ResolvedType::Usize,
                ResolvedType::U8,
                ResolvedType::U8,
                ResolvedType::U8,
                ResolvedType::U8,
                ResolvedType::U8,
            ],
            Self::Set1Or5 => &[
                ResolvedType::Bytes,
                ResolvedType::Usize,
                ResolvedType::U8,
                ResolvedType::SliceU8,
                ResolvedType::Usize,
            ],
            Self::Set1Or6Or48 => &[
                ResolvedType::Bytes,
                ResolvedType::Usize,
                ResolvedType::U8,
                ResolvedType::SliceU8,
                ResolvedType::Usize,
            ],
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
            Self::Copy
            | Self::Zeroed
            | Self::Set
            | Self::Set5
            | Self::Set1Or5
            | Self::Set1Or6Or48 => ResolvedType::Bytes,
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
            Self::Copy
            | Self::Zeroed
            | Self::Set
            | Self::Set5
            | Self::Set1Or5
            | Self::Set1Or6Or48 => Type::Bytes,
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
            (Self::Set5, 0) => *ty == ResolvedType::Bytes,
            (Self::Set5, 1) => *ty == ResolvedType::Usize,
            (Self::Set5, 2..=6) => *ty == ResolvedType::U8,
            (Self::Set1Or5, 0) => *ty == ResolvedType::Bytes,
            (Self::Set1Or5, 1 | 4) => *ty == ResolvedType::Usize,
            (Self::Set1Or5, 2) => *ty == ResolvedType::U8,
            (Self::Set1Or5, 3) => *ty == ResolvedType::SliceU8,
            (Self::Set1Or6Or48, 0) => *ty == ResolvedType::Bytes,
            (Self::Set1Or6Or48, 1 | 4) => *ty == ResolvedType::Usize,
            (Self::Set1Or6Or48, 2) => *ty == ResolvedType::U8,
            (Self::Set1Or6Or48, 3) => *ty == ResolvedType::SliceU8,
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
            (Self::Set5, 0) => *ty == Type::Bytes,
            (Self::Set5, 1) => *ty == Type::Usize,
            (Self::Set5, 2..=6) => *ty == Type::U8,
            (Self::Set1Or5, 0) => *ty == Type::Bytes,
            (Self::Set1Or5, 1 | 4) => *ty == Type::Usize,
            (Self::Set1Or5, 2) => *ty == Type::U8,
            (Self::Set1Or5, 3) => *ty == Type::SliceU8,
            (Self::Set1Or6Or48, 0) => *ty == Type::Bytes,
            (Self::Set1Or6Or48, 1 | 4) => *ty == Type::Usize,
            (Self::Set1Or6Or48, 2) => *ty == Type::U8,
            (Self::Set1Or6Or48, 3) => *ty == Type::SliceU8,
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
        matches!(
            self,
            Self::Zeroed | Self::Set | Self::Set5 | Self::Set1Or5 | Self::Set1Or6Or48
        )
    }

    /// `true` for a byte operation one bounded `while` condition or body
    /// admits: the exact read-only views plus the loop-carried `bytes_set`
    /// fill. `bytes_zeroed` is deliberately absent. The allocation stays
    /// outside the loop, which is what the target-neutral owned byte capacity
    /// analysis and the fixed Core-Wasm arena require; the owned byte
    /// allocation rule rejects it independently with `SPX-T267`.
    pub(crate) const fn admitted_in_while(self) -> bool {
        matches!(
            self,
            Self::Len
                | Self::Get
                | Self::Range
                | Self::Set
                | Self::Set5
                | Self::Set1Or5
                | Self::Set1Or6Or48
        )
    }

    /// `true` for the one byte operation that can select a runtime failure.
    ///
    /// `bytes_set` admits a computed element index, so the store is checked
    /// against the transferred buffer's length before the owner is committed.
    /// Every other operation in this family is total after HIR admission, and
    /// physical allocation failure stays invariant fail-stop.
    pub(crate) const fn is_fallible(self) -> bool {
        matches!(
            self,
            Self::Set | Self::Set5 | Self::Set1Or5 | Self::Set1Or6Or48
        )
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
            Self::Set5 => &[
                "buffer", "index", "first", "second", "third", "fourth", "fifth",
            ],
            Self::Set1Or5 => &["buffer", "index", "one", "source", "selector"],
            Self::Set1Or6Or48 => &["buffer", "index", "one", "source", "selector"],
        }
    }

    /// Canonical argument ownership. The first operand of a borrowed view or
    /// read is borrowed, `bytes_set` transfers its buffer, and every scalar
    /// operand is an ordinary copied value.
    pub(crate) const fn param_ownership(self, index: usize) -> OwnershipMode {
        match (self, index) {
            (Self::Set | Self::Set5 | Self::Set1Or5 | Self::Set1Or6Or48, 0) => OwnershipMode::Own,
            (Self::Set1Or5 | Self::Set1Or6Or48, 3) => OwnershipMode::Borrow,
            (Self::Zeroed, _) | (Self::Set | Self::Set5 | Self::Set1Or5 | Self::Set1Or6Or48, _) => {
                OwnershipMode::Value
            }
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
        SET5_NAME => Some(ByteOp::Set5),
        SET1_OR5_NAME => Some(ByteOp::Set1Or5),
        SET1_OR6_OR48_NAME => Some(ByteOp::Set1Or6Or48),
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
        SET5_ID => Some(ByteOp::Set5),
        SET1_OR5_ID => Some(ByteOp::Set1Or5),
        SET1_OR6_OR48_ID => Some(ByteOp::Set1Or6Or48),
        _ => None,
    }
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
            ByteOp::Set | ByteOp::Set5 | ByteOp::Set1Or5 | ByteOp::Set1Or6Or48 => {
                links += usize::try_from(if op == ByteOp::Set1Or6Or48 {
                    48
                } else if matches!(op, ByteOp::Set5 | ByteOp::Set1Or5) {
                    SET5_WIDTH
                } else {
                    1
                })
                .expect("set width fits usize");
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

/// Owned Bounded Byte Buffer v1 same-owner re-open: the buffer operand of a
/// `bytes_set` may be one whole named binding the call moves, instead of the
/// enclosing chain's previous link, when that call is the right-hand side of
/// the assignment that republishes the same binding.
pub(crate) fn owned_buffer_operand_is_binding(operand: &Expr) -> bool {
    matches!(operand.kind, ExprKind::Var(_))
}

/// The same-owner replacement `buffer = bytes_set(buffer, index, value)`: the
/// one assignment shape that re-opens an owned byte buffer binding for its next
/// generation. The right-hand side evaluates before publication, so no second
/// owner and no partially filled buffer is ever nameable.
pub(crate) fn is_same_owner_set_source(value: &Expr, name: &str, ty: &Type) -> bool {
    *ty == Type::Bytes && is_same_owner_set_shape(value, name)
}

/// The syntactic half of [`is_same_owner_set_source`], without the binding
/// type. The source verifier registers the admitted re-open call sites while
/// scheduling an assignment statement, before the binding's checked type is
/// known; the ordinary assignment rules reject every other type.
pub(crate) fn is_same_owner_set_shape(value: &Expr, name: &str) -> bool {
    let ExprKind::Call {
        name: callee,
        type_arguments,
        args,
    } = &value.kind
    else {
        return false;
    };
    matches!(
        by_name(callee),
        Some(ByteOp::Set | ByteOp::Set5 | ByteOp::Set1Or5 | ByteOp::Set1Or6Or48)
    ) && type_arguments.is_empty()
        && args.len() == by_name(callee).expect("matched owned buffer op").arity()
        && matches!(&args[0].kind, ExprKind::Var(source) if source == name)
}

/// Resolved-HIR twin of [`is_same_owner_set_source`]. Hostile HIR that never
/// passed through source text must re-derive the identical fact.
pub(crate) fn is_same_owner_set_hir(value: &crate::hir::ResolvedExpr, owner: &ValueId) -> bool {
    matches!(
        &value.kind,
        crate::hir::ResolvedExprKind::Call { callee, type_arguments, instance: None, args }
            if matches!(by_id(callee.as_str()), Some(ByteOp::Set | ByteOp::Set5 | ByteOp::Set1Or5 | ByteOp::Set1Or6Or48))
                && type_arguments.is_empty()
                && args.len() == by_id(callee.as_str()).expect("matched owned buffer op").arity()
                && matches!(
                    &args[0].kind,
                    crate::hir::ResolvedExprKind::Place(place)
                        if &place.root == owner && place.projections.is_empty()
                )
    )
}

#[cfg(test)]
mod tests {
    /// `tests/cleanup_backends.rs` path-includes `src/byte_data_capacity.rs`,
    /// which reads this ceiling through `crate::byte_ops`. That harness cannot
    /// reach the real module, so it mirrors the constant, and nothing there can
    /// detect drift: the included source reads the mirror itself. This is the
    /// guard, next to the source of truth.
    #[test]
    fn the_cleanup_backends_mirror_matches_this_ceiling() {
        let harness = include_str!("../tests/cleanup_backends.rs");
        let needle = "pub(crate) const MAX_OWNED_BYTE_VALUE_BYTES: u64 = ";
        let start = harness
            .find(needle)
            .expect("the cleanup_backends harness must mirror the owned-byte ceiling")
            + needle.len();
        let mirrored = harness[start..]
            .split(';')
            .next()
            .expect("the mirrored constant must be terminated")
            .trim()
            .replace('_', "")
            .parse::<u64>()
            .expect("the mirrored constant must be a plain integer literal");
        assert_eq!(
            mirrored,
            super::MAX_OWNED_BYTE_VALUE_BYTES,
            "tests/cleanup_backends.rs mirrors MAX_OWNED_BYTE_VALUE_BYTES; the two \
             must be updated together"
        );
    }
}
