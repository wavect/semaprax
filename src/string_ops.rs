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
//!
//! Text Toolkit v1 (`docs/TEXT-TOOLKIT-V1.md`, gated as its own backend group
//! so every earlier program keeps its exact bytes):
//!
//! - `string_slice(s: string, start: i64, end: i64) -> string` copies a byte
//!   range that must lie on UTF-8 character boundaries.
//! - `string_find(s: string, needle: string, from: i64) -> i64` returns the
//!   first byte offset at or after `from`, or -1.
//! - `string_to_i64(s: string) -> Option<i64>` parses strict decimal text.
//! - `string_trim(s: string) -> string` strips ASCII whitespace at both ends.
//! - `string_byte_at(s: string, index: i64) -> i64` reads one byte value.
//! - `file_read_text(path: borrow str) -> string` reads one bounded UTF-8 file
//!   under the explicit `fs.read` effect. It is the only effectful member of
//!   this family; its identity lives in the `core.host.*` namespace.
//!
//! Out-of-range offsets, non-boundary slices, and invalid UTF-8 select the
//! checked `semaprax.text.v1` status; file failures select the existing
//! `semaprax.filesystem.v1` codes.
//!
//! String Collections v1 (`docs/STRING-COLLECTIONS-V1.md`, its own backend
//! group) adds bytewise string ordering and the one admitted string-keyed
//! map, `Map<string, i64>`, kept in ascending bytewise key order:
//!
//! - `string_compare(a: string, b: string) -> i64` is -1, 0, or 1.
//! - `map_new(capacity: usize) -> Map<string, i64>` creates an empty map that
//!   holds at most `capacity` (at most 65,536) entries.
//! - `map_add(map: own Map, key: string, delta: i64) -> Map` and
//!   `map_set(map: own Map, key: string, value: i64) -> Map` are same-owner
//!   reopens: `counts = map_add(counts, key, 1);`.
//! - `map_get_or`, `map_has`, `map_len`, `map_key_at`, and `map_value_at`
//!   borrow the map.
//!
//! Map failures select the checked `semaprax.map.v1` status.
//!
//! Conversions v1 (`docs/LANGUAGE-ERGONOMICS-V1.md`, its own backend group)
//! adds the scalar conversions and the borrowed-to-owned text copy. The
//! numeric conversions carry `core.num.*` identities; they share this
//! intrinsic table only for dispatch and touch no string value:
//!
//! - `f64_from_i64(value: i64) -> f64` rounds to nearest, ties to even (exact
//!   for every magnitude up to 2^53).
//! - `i64_from_f64(value: f64) -> i64` truncates toward zero.
//! - `usize_from_i64(value: i64) -> usize` and
//!   `i64_from_usize(value: usize) -> i64` copy the value exactly.
//! - `string_from_str(s: borrow str) -> string` copies a borrowed view into a
//!   new owned string.
//!
//! A value the target type cannot hold selects the checked
//! `semaprax.convert.v1` status: code 1 for out of range, code 2 for NaN.

pub(crate) mod conditions;

pub(crate) mod replacement;
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
pub(crate) const SLICE_NAME: &str = "string_slice";
pub(crate) const FIND_NAME: &str = "string_find";
pub(crate) const TO_I64_NAME: &str = "string_to_i64";
pub(crate) const TRIM_NAME: &str = "string_trim";
pub(crate) const BYTE_AT_NAME: &str = "string_byte_at";
pub(crate) const FILE_READ_TEXT_NAME: &str = "file_read_text";
pub(crate) const COMPARE_NAME: &str = "string_compare";
pub(crate) const MAP_NEW_NAME: &str = "map_new";
pub(crate) const MAP_ADD_NAME: &str = "map_add";
pub(crate) const MAP_SET_NAME: &str = "map_set";
pub(crate) const MAP_GET_OR_NAME: &str = "map_get_or";
pub(crate) const MAP_HAS_NAME: &str = "map_has";
pub(crate) const MAP_LEN_NAME: &str = "map_len";
pub(crate) const MAP_KEY_AT_NAME: &str = "map_key_at";
pub(crate) const MAP_VALUE_AT_NAME: &str = "map_value_at";
pub(crate) const MAP_REMOVE_NAME: &str = "map_remove";
pub(crate) const FROM_STR_NAME: &str = "string_from_str";
pub(crate) const F64_FROM_I64_NAME: &str = "f64_from_i64";
pub(crate) const I64_FROM_F64_NAME: &str = "i64_from_f64";
pub(crate) const USIZE_FROM_I64_NAME: &str = "usize_from_i64";
pub(crate) const I64_FROM_USIZE_NAME: &str = "i64_from_usize";

pub(crate) const LEN_ID: &str = "core.string.len";
pub(crate) const CONCAT_ID: &str = "core.string.concat";
pub(crate) const IS_EMPTY_ID: &str = "core.string.is_empty";
pub(crate) const STARTS_WITH_ID: &str = "core.string.starts_with";
pub(crate) const CONTAINS_ID: &str = "core.string.contains";
pub(crate) const LEN_CHARS_ID: &str = "core.string.len_chars";
pub(crate) const FROM_CHAR_ID: &str = "core.string.from_char";
pub(crate) const FROM_I64_ID: &str = "core.string.from_i64";
pub(crate) const FROM_USIZE_ID: &str = "core.string.from_usize";
pub(crate) const SLICE_ID: &str = "core.string.slice";
pub(crate) const FIND_ID: &str = "core.string.find";
pub(crate) const TO_I64_ID: &str = "core.string.to_i64";
pub(crate) const TRIM_ID: &str = "core.string.trim";
pub(crate) const BYTE_AT_ID: &str = "core.string.byte_at";
pub(crate) const FILE_READ_TEXT_ID: &str = "core.host.file-read-text";
pub(crate) const COMPARE_ID: &str = "core.string.compare";
pub(crate) const MAP_NEW_ID: &str = "core.map.new";
pub(crate) const MAP_ADD_ID: &str = "core.map.add";
pub(crate) const MAP_SET_ID: &str = "core.map.set";
pub(crate) const MAP_GET_OR_ID: &str = "core.map.get_or";
pub(crate) const MAP_HAS_ID: &str = "core.map.has";
pub(crate) const MAP_LEN_ID: &str = "core.map.len";
pub(crate) const MAP_KEY_AT_ID: &str = "core.map.key_at";
pub(crate) const MAP_VALUE_AT_ID: &str = "core.map.value_at";
pub(crate) const MAP_REMOVE_ID: &str = "core.map.remove.v2";
pub(crate) const FROM_STR_ID: &str = "core.string.from_str";
pub(crate) const F64_FROM_I64_ID: &str = "core.num.f64_from_i64";
pub(crate) const I64_FROM_F64_ID: &str = "core.num.i64_from_f64";
pub(crate) const USIZE_FROM_I64_ID: &str = "core.num.usize_from_i64";
pub(crate) const I64_FROM_USIZE_ID: &str = "core.num.i64_from_usize";

pub(crate) const I64_FROM_U8_NAME: &str = "i64_from_u8";
pub(crate) const I64_FROM_U8_ID: &str = "core.num.i64_from_u8";

pub(crate) const I64_FROM_I32_NAME: &str = "i64_from_i32";
pub(crate) const I64_FROM_I32_ID: &str = "core.num.i64_from_i32";

pub(crate) const USIZE_FROM_U8_NAME: &str = "usize_from_u8";
pub(crate) const USIZE_FROM_U8_ID: &str = "core.num.usize_from_u8";

/// The checked status domain of Conversions v1.
pub(crate) const CONVERT_STATUS_DOMAIN: &str = "semaprax.convert.v1";
/// The value lies outside the target type's range.
pub(crate) const CONVERT_OUT_OF_RANGE_CODE: u32 = 1;
/// `i64_from_f64` received NaN.
pub(crate) const CONVERT_NAN_CODE: u32 = 2;

/// The checked status domain of String Collections v1 maps.
pub(crate) const MAP_STATUS_DOMAIN: &str = "semaprax.map.v1";
/// A new key does not fit: the map already holds `capacity` entries.
pub(crate) const MAP_FULL_CODE: u32 = 1;
/// `map_key_at` / `map_value_at` index is not below `map_len`.
pub(crate) const MAP_INDEX_OUT_OF_RANGE_CODE: u32 = 2;
/// `map_new` capacity is above [`MAX_MAP_CAPACITY`].
pub(crate) const MAP_CAPACITY_CODE: u32 = 3;
/// `map_add` would overflow the entry's `i64` value.
pub(crate) const MAP_VALUE_OVERFLOW_CODE: u32 = 4;
/// Canonical compiler-owned cleanup lifecycle for one `Map<string, i64>`
/// carrier; it releases every key and the entry table.
pub const MAP_DROP_LIFECYCLE_ID: &str = "core.map.drop";
/// The largest entry count a map may declare.
pub(crate) const MAX_MAP_CAPACITY: u64 = 65_536;

/// The checked status domain of Text Toolkit v1.
pub(crate) const TEXT_STATUS_DOMAIN: &str = "semaprax.text.v1";
/// A byte offset or index lies outside the string (or `start > end`).
pub(crate) const TEXT_OUT_OF_RANGE_CODE: u32 = 1;
/// A slice bound splits a multi-byte UTF-8 character.
pub(crate) const TEXT_NOT_CHAR_BOUNDARY_CODE: u32 = 2;
/// File bytes are not valid UTF-8.
pub(crate) const TEXT_INVALID_UTF8_CODE: u32 = 3;
/// The effect `file_read_text` requires.
pub(crate) const FILE_READ_TEXT_EFFECT: &str = "fs.read";
/// The bounded file size `file_read_text` admits, equal to the Filesystem
/// I/O per-file maximum.
pub(crate) const MAX_FILE_TEXT_BYTES: u64 = 65_536;

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
    /// Checked byte-range copy on UTF-8 boundaries.
    Slice,
    /// Borrowed forward substring search from a checked offset.
    Find,
    /// Strict decimal parse into `Option<i64>`.
    ToI64,
    /// Copy without leading and trailing ASCII whitespace.
    Trim,
    /// Checked single byte read.
    ByteAt,
    /// Bounded UTF-8 file read under `fs.read`.
    FileReadText,
    /// Bytewise three-way comparison of two borrowed strings.
    Compare,
    /// An empty `Map<string, i64>` with a checked entry capacity.
    MapNew,
    /// Same-owner reopen: insert the key with `delta`, or add `delta`.
    MapAdd,
    /// Same-owner reopen: insert or replace the key's value.
    MapSet,
    /// Borrowed lookup with a default.
    MapGetOr,
    /// Borrowed membership test.
    MapHas,
    /// Borrowed entry count.
    MapLen,
    /// Borrowed key at an ascending-order index, as a new string.
    MapKeyAt,
    /// Borrowed value at an ascending-order index.
    MapValueAt,
    /// Remove a key if present, transferring the map at call commit.
    MapRemove,
    /// Copy of a borrowed `str` view into a new owned string.
    FromStr,
    /// `i64` to the nearest `f64`, ties to even.
    F64FromI64,
    /// Checked `f64` to `i64`, truncating toward zero.
    I64FromF64,
    /// Checked `i64` to `usize`.
    UsizeFromI64,
    /// Checked `usize` to `i64`.
    I64FromUsize,
    /// Exact infallible unsigned byte widening (Byte Widening v1).
    I64FromU8,
    /// Exact sign extension from a narrow signed integer.
    I64FromI32,
    /// Exact zero extension to portable u64 size.
    UsizeFromU8,
}

impl StringOp {
    /// The first three waves. Frozen Project candidate schemas enumerate
    /// exactly these operations, so Text Toolkit v1 is listed separately.
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

    /// Text Toolkit v1 operations, outside the frozen [`Self::ALL`] catalog.
    pub(crate) const TEXT_TOOLKIT: [Self; 6] = [
        Self::Slice,
        Self::Find,
        Self::ToI64,
        Self::Trim,
        Self::ByteAt,
        Self::FileReadText,
    ];

    /// String Collections v1 operations, outside the frozen [`Self::ALL`]
    /// catalog and its own backend group.
    pub(crate) const COLLECTIONS: [Self; 9] = [
        Self::Compare,
        Self::MapNew,
        Self::MapAdd,
        Self::MapSet,
        Self::MapGetOr,
        Self::MapHas,
        Self::MapLen,
        Self::MapKeyAt,
        Self::MapValueAt,
    ];

    /// Conversions v1 operations, outside the frozen [`Self::ALL`] catalog
    /// and their own backend group.
    pub(crate) const CONVERSIONS: [Self; 5] = [
        Self::FromStr,
        Self::F64FromI64,
        Self::I64FromF64,
        Self::UsizeFromI64,
        Self::I64FromUsize,
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
            StringOp::Slice => SLICE_NAME,
            StringOp::Find => FIND_NAME,
            StringOp::ToI64 => TO_I64_NAME,
            StringOp::Trim => TRIM_NAME,
            StringOp::ByteAt => BYTE_AT_NAME,
            StringOp::FileReadText => FILE_READ_TEXT_NAME,
            StringOp::Compare => COMPARE_NAME,
            StringOp::MapNew => MAP_NEW_NAME,
            StringOp::MapAdd => MAP_ADD_NAME,
            StringOp::MapSet => MAP_SET_NAME,
            StringOp::MapGetOr => MAP_GET_OR_NAME,
            StringOp::MapHas => MAP_HAS_NAME,
            StringOp::MapLen => MAP_LEN_NAME,
            StringOp::MapKeyAt => MAP_KEY_AT_NAME,
            StringOp::MapValueAt => MAP_VALUE_AT_NAME,
            StringOp::MapRemove => MAP_REMOVE_NAME,
            StringOp::FromStr => FROM_STR_NAME,
            StringOp::F64FromI64 => F64_FROM_I64_NAME,
            StringOp::I64FromF64 => I64_FROM_F64_NAME,
            StringOp::UsizeFromI64 => USIZE_FROM_I64_NAME,
            StringOp::I64FromU8 => I64_FROM_U8_NAME,
            StringOp::I64FromI32 => I64_FROM_I32_NAME,
            StringOp::UsizeFromU8 => USIZE_FROM_U8_NAME,
            StringOp::I64FromUsize => I64_FROM_USIZE_NAME,
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
            StringOp::Slice => SLICE_ID,
            StringOp::Find => FIND_ID,
            StringOp::ToI64 => TO_I64_ID,
            StringOp::Trim => TRIM_ID,
            StringOp::ByteAt => BYTE_AT_ID,
            StringOp::FileReadText => FILE_READ_TEXT_ID,
            StringOp::Compare => COMPARE_ID,
            StringOp::MapNew => MAP_NEW_ID,
            StringOp::MapAdd => MAP_ADD_ID,
            StringOp::MapSet => MAP_SET_ID,
            StringOp::MapGetOr => MAP_GET_OR_ID,
            StringOp::MapHas => MAP_HAS_ID,
            StringOp::MapLen => MAP_LEN_ID,
            StringOp::MapKeyAt => MAP_KEY_AT_ID,
            StringOp::MapValueAt => MAP_VALUE_AT_ID,
            StringOp::MapRemove => MAP_REMOVE_ID,
            StringOp::FromStr => FROM_STR_ID,
            StringOp::F64FromI64 => F64_FROM_I64_ID,
            StringOp::I64FromF64 => I64_FROM_F64_ID,
            StringOp::UsizeFromI64 => USIZE_FROM_I64_ID,
            StringOp::I64FromU8 => I64_FROM_U8_ID,
            StringOp::I64FromI32 => I64_FROM_I32_ID,
            StringOp::UsizeFromU8 => USIZE_FROM_U8_ID,
            StringOp::I64FromUsize => I64_FROM_USIZE_ID,
        }
    }

    pub(crate) fn arity(self) -> usize {
        self.param_names().len()
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
            StringOp::Slice => &["s", "start", "end"],
            StringOp::Find => &["s", "needle", "from"],
            StringOp::ToI64 | StringOp::Trim => &["s"],
            StringOp::ByteAt => &["s", "index"],
            StringOp::FileReadText => &["path"],
            StringOp::Compare => &["a", "b"],
            StringOp::MapNew => &["capacity"],
            StringOp::MapAdd => &["map", "key", "delta"],
            StringOp::MapSet => &["map", "key", "value"],
            StringOp::MapGetOr => &["map", "key", "default"],
            StringOp::MapHas | StringOp::MapRemove => &["map", "key"],
            StringOp::MapLen => &["map"],
            StringOp::MapKeyAt | StringOp::MapValueAt => &["map", "index"],
            StringOp::FromStr => &["s"],
            StringOp::F64FromI64
            | StringOp::I64FromF64
            | StringOp::UsizeFromI64
            | StringOp::I64FromU8
            | StringOp::I64FromI32
            | StringOp::UsizeFromU8
            | StringOp::I64FromUsize => &["value"],
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
            StringOp::Slice => &[ResolvedType::String, ResolvedType::I64, ResolvedType::I64],
            StringOp::Find => &[
                ResolvedType::String,
                ResolvedType::String,
                ResolvedType::I64,
            ],
            StringOp::ToI64 | StringOp::Trim => &[ResolvedType::String],
            StringOp::ByteAt => &[ResolvedType::String, ResolvedType::I64],
            StringOp::FileReadText => &[ResolvedType::Str],
            StringOp::Compare => &[ResolvedType::String, ResolvedType::String],
            StringOp::MapNew => &[ResolvedType::Usize],
            StringOp::MapAdd | StringOp::MapSet | StringOp::MapGetOr => &[
                ResolvedType::StringMap,
                ResolvedType::String,
                ResolvedType::I64,
            ],
            StringOp::MapHas | StringOp::MapRemove => &[ResolvedType::StringMap, ResolvedType::String],
            StringOp::MapLen => &[ResolvedType::StringMap],
            StringOp::MapKeyAt | StringOp::MapValueAt => {
                &[ResolvedType::StringMap, ResolvedType::Usize]
            }
            StringOp::FromStr => &[ResolvedType::Str],
            StringOp::F64FromI64 | StringOp::UsizeFromI64 => &[ResolvedType::I64],
            StringOp::I64FromF64 => &[ResolvedType::F64],
            StringOp::I64FromU8 => &[ResolvedType::U8],
            StringOp::I64FromI32 => &[ResolvedType::I32],
            StringOp::UsizeFromU8 => &[ResolvedType::U8],
            StringOp::I64FromUsize => &[ResolvedType::Usize],
        }
    }

    /// Whether every non-scalar operand is consumed (only `string_concat`).
    pub(crate) fn consumes_arguments(self) -> bool {
        matches!(self, StringOp::Concat)
    }

    /// The ownership mode of one parameter. `string_concat` consumes both
    /// operands; `map_add` and `map_set` consume only their map (the
    /// same-owner reopen) and borrow the key, which the map copies when it
    /// inserts; every other non-scalar operand is borrowed; scalars are
    /// copied.
    pub(crate) fn param_ownership(self, index: usize) -> OwnershipMode {
        match self.param_types().get(index) {
            Some(
                ResolvedType::Char
                | ResolvedType::I64
                | ResolvedType::I32
                | ResolvedType::U8
                | ResolvedType::Usize
                | ResolvedType::F64,
            ) => OwnershipMode::Value,
            _ if self.consumes_arguments() => OwnershipMode::Own,
            _ if index == 0 && self.reopens_map() => OwnershipMode::Own,
            _ => OwnershipMode::Borrow,
        }
    }

    /// `map_add` and `map_set` return their consumed map as the next
    /// generation; they are only admitted as a same-owner reopen.
    pub(crate) fn reopens_map(self) -> bool {
        matches!(self, StringOp::MapAdd | StringOp::MapSet | StringOp::MapRemove)
    }

    /// String Collections v1 forms a fifth optional backend group.
    pub(crate) fn is_collection(self) -> bool {
        Self::COLLECTIONS.contains(&self) || self == Self::MapRemove
    }

    /// Conversions v1 forms a sixth optional backend group.
    pub(crate) fn is_conversion(self) -> bool {
        Self::CONVERSIONS.contains(&self) || self.is_integer_conversion()
    }

    /// Additive integer conversion profile, with direct extension and checked ranges.
    pub(crate) fn is_integer_conversion(self) -> bool {
        matches!(
            self,
            Self::I64FromU8
                | Self::I64FromI32
                | Self::UsizeFromU8
                | Self::I64FromUsize
                | Self::UsizeFromI64
        )
    }

    /// Whether the operation reads or produces a String value. The numeric
    /// conversions do not, so they never select a String runtime.
    pub(crate) fn touches_string(self) -> bool {
        !matches!(
            self,
            StringOp::F64FromI64
                | StringOp::I64FromF64
                | StringOp::UsizeFromI64
                | StringOp::I64FromU8
                | StringOp::I64FromI32
                | StringOp::UsizeFromU8
                | StringOp::I64FromUsize
        )
    }

    /// Operations no Core Wasm lane lowers: Text Toolkit v1, String
    /// Collections v1, and Conversions v1.
    pub(crate) fn is_wasm_refused(self) -> bool {
        self.is_text_toolkit()
            || self.is_collection()
            || (self.is_conversion() && !self.is_integer_conversion())
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

    /// Text Toolkit v1 forms a fourth optional backend group.
    pub(crate) fn is_text_toolkit(self) -> bool {
        Self::TEXT_TOOLKIT.contains(&self)
    }

    /// The host effect a call requires, if any. Only `file_read_text` has one.
    pub(crate) fn effect(self) -> Option<&'static str> {
        (self == StringOp::FileReadText).then_some(FILE_READ_TEXT_EFFECT)
    }

    pub(crate) fn return_type(self) -> ResolvedType {
        match self {
            StringOp::Len
            | StringOp::LenChars
            | StringOp::Find
            | StringOp::ByteAt
            | StringOp::Compare
            | StringOp::MapGetOr
            | StringOp::MapValueAt
            | StringOp::I64FromF64
            | StringOp::I64FromUsize => ResolvedType::I64,
            StringOp::I64FromU8 => ResolvedType::I64,
            StringOp::I64FromI32 => ResolvedType::I64,
            StringOp::UsizeFromU8 => ResolvedType::Usize,
            StringOp::F64FromI64 => ResolvedType::F64,
            StringOp::UsizeFromI64 => ResolvedType::Usize,
            StringOp::MapNew | StringOp::MapAdd | StringOp::MapSet | StringOp::MapRemove => ResolvedType::StringMap,
            StringOp::MapLen => ResolvedType::Usize,
            StringOp::MapHas => ResolvedType::Bool,
            StringOp::MapKeyAt => ResolvedType::String,
            StringOp::Concat
            | StringOp::FromChar
            | StringOp::FromI64
            | StringOp::FromUsize
            | StringOp::Slice
            | StringOp::Trim
            | StringOp::FileReadText
            | StringOp::FromStr => ResolvedType::String,
            StringOp::IsEmpty | StringOp::StartsWith | StringOp::Contains => ResolvedType::Bool,
            StringOp::ToI64 => option_i64(),
        }
    }

    pub(crate) fn ast_return_type(self) -> Type {
        match self {
            StringOp::Len
            | StringOp::LenChars
            | StringOp::Find
            | StringOp::ByteAt
            | StringOp::Compare
            | StringOp::MapGetOr
            | StringOp::MapValueAt
            | StringOp::I64FromF64
            | StringOp::I64FromUsize => Type::I64,
            StringOp::I64FromU8 => Type::I64,
            StringOp::I64FromI32 => Type::I64,
            StringOp::UsizeFromU8 => Type::Usize,
            StringOp::F64FromI64 => Type::F64,
            StringOp::UsizeFromI64 => Type::Usize,
            StringOp::MapNew | StringOp::MapAdd | StringOp::MapSet | StringOp::MapRemove => Type::StringMap,
            StringOp::MapLen => Type::Usize,
            StringOp::MapHas => Type::Bool,
            StringOp::MapKeyAt => Type::String,
            StringOp::Concat
            | StringOp::FromChar
            | StringOp::FromI64
            | StringOp::FromUsize
            | StringOp::Slice
            | StringOp::Trim
            | StringOp::FileReadText
            | StringOp::FromStr => Type::String,
            StringOp::IsEmpty | StringOp::StartsWith | StringOp::Contains => Type::Bool,
            StringOp::ToI64 => Type::Named {
                name: "Option".to_owned(),
                arguments: vec![Type::I64],
            },
        }
    }
}

/// The compiler-owned `Option<i64>` that `string_to_i64` produces.
pub(crate) fn option_i64() -> ResolvedType {
    ResolvedType::Nominal {
        declaration: crate::hir::DeclarationId::new(crate::prelude::OPTION_ID),
        arguments: vec![ResolvedType::I64],
    }
}

/// Whether `scrutinee` is a direct `string_to_i64` call, the one
/// `Option<i64>` producer the bounded interpreter and the while-body match
/// admission accept.
pub(crate) fn is_to_i64_call_hir(scrutinee: &crate::hir::ResolvedExpr) -> bool {
    matches!(
        &scrutinee.kind,
        crate::hir::ResolvedExprKind::Call { callee, instance: None, type_arguments, args }
            if by_id(callee.as_str()) == Some(StringOp::ToI64)
                && type_arguments.is_empty()
                && args.len() == 1
                && scrutinee.ty == option_i64()
    )
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
        SLICE_NAME => Some(StringOp::Slice),
        FIND_NAME => Some(StringOp::Find),
        TO_I64_NAME => Some(StringOp::ToI64),
        TRIM_NAME => Some(StringOp::Trim),
        BYTE_AT_NAME => Some(StringOp::ByteAt),
        FILE_READ_TEXT_NAME => Some(StringOp::FileReadText),
        COMPARE_NAME => Some(StringOp::Compare),
        MAP_NEW_NAME => Some(StringOp::MapNew),
        MAP_ADD_NAME => Some(StringOp::MapAdd),
        MAP_SET_NAME => Some(StringOp::MapSet),
        MAP_GET_OR_NAME => Some(StringOp::MapGetOr),
        MAP_HAS_NAME => Some(StringOp::MapHas),
        MAP_LEN_NAME => Some(StringOp::MapLen),
        MAP_KEY_AT_NAME => Some(StringOp::MapKeyAt),
        MAP_VALUE_AT_NAME => Some(StringOp::MapValueAt),
        MAP_REMOVE_NAME => Some(StringOp::MapRemove),
        FROM_STR_NAME => Some(StringOp::FromStr),
        F64_FROM_I64_NAME => Some(StringOp::F64FromI64),
        I64_FROM_F64_NAME => Some(StringOp::I64FromF64),
        USIZE_FROM_I64_NAME => Some(StringOp::UsizeFromI64),
        I64_FROM_U8_NAME => Some(StringOp::I64FromU8),
        I64_FROM_I32_NAME => Some(StringOp::I64FromI32),
        USIZE_FROM_U8_NAME => Some(StringOp::UsizeFromU8),
        I64_FROM_USIZE_NAME => Some(StringOp::I64FromUsize),
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
        SLICE_ID => Some(StringOp::Slice),
        FIND_ID => Some(StringOp::Find),
        TO_I64_ID => Some(StringOp::ToI64),
        TRIM_ID => Some(StringOp::Trim),
        BYTE_AT_ID => Some(StringOp::ByteAt),
        FILE_READ_TEXT_ID => Some(StringOp::FileReadText),
        COMPARE_ID => Some(StringOp::Compare),
        MAP_NEW_ID => Some(StringOp::MapNew),
        MAP_ADD_ID => Some(StringOp::MapAdd),
        MAP_SET_ID => Some(StringOp::MapSet),
        MAP_GET_OR_ID => Some(StringOp::MapGetOr),
        MAP_HAS_ID => Some(StringOp::MapHas),
        MAP_LEN_ID => Some(StringOp::MapLen),
        MAP_KEY_AT_ID => Some(StringOp::MapKeyAt),
        MAP_VALUE_AT_ID => Some(StringOp::MapValueAt),
        MAP_REMOVE_ID => Some(StringOp::MapRemove),
        FROM_STR_ID => Some(StringOp::FromStr),
        F64_FROM_I64_ID => Some(StringOp::F64FromI64),
        I64_FROM_F64_ID => Some(StringOp::I64FromF64),
        USIZE_FROM_I64_ID => Some(StringOp::UsizeFromI64),
        I64_FROM_U8_ID => Some(StringOp::I64FromU8),
        I64_FROM_I32_ID => Some(StringOp::I64FromI32),
        USIZE_FROM_U8_ID => Some(StringOp::UsizeFromU8),
        I64_FROM_USIZE_ID => Some(StringOp::I64FromUsize),
        _ => None,
    }
}

/// Synthetic HIR parameters for one operation: consuming arguments carry
/// `Own` ownership exactly like an ordinary declared `string` parameter,
/// borrowed arguments accept every argument ownership without a transfer,
/// and copied scalar arguments use the ordinary `Value` mode of their kind.
pub(crate) fn resolved_params(op: StringOp) -> Vec<ResolvedParam> {
    op.param_names()
        .iter()
        .zip(op.param_types())
        .enumerate()
        .map(|(index, (name, ty))| ResolvedParam {
            id: ValueId::intrinsic_parameter(op.id(), index),
            name: (*name).to_owned(),
            ownership: op.param_ownership(index),
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
        .enumerate()
        .map(|(index, (name, ty))| Param {
            name: (*name).to_owned(),
            mode: if *ty == ResolvedType::Str {
                ParamMode::Borrow
            } else if op.param_ownership(index) == OwnershipMode::Own {
                ParamMode::Own
            } else {
                ParamMode::Value
            },
            ty: match ty {
                ResolvedType::Char => Type::Char,
                ResolvedType::I64 => Type::I64,
                ResolvedType::I32 => Type::I32,
                ResolvedType::U8 => Type::U8,
                ResolvedType::Usize => Type::Usize,
                ResolvedType::F64 => Type::F64,
                ResolvedType::Str => Type::Str,
                ResolvedType::StringMap => Type::StringMap,
                _ => Type::String,
            },
            span: Span::default(),
        })
        .collect()
}

/// Owned String Loops v1 same-owner append: `text = string_concat(text, …)`,
/// where the assignment target and the first operand are the same whole
/// `let mut` binding. The call consumes the current generation as its first
/// staged argument and the assignment publishes the next one, so exactly one
/// generation of the owner is live and no release happens at the assignment.
///
/// String Collections v1 adds the map reopens `counts = map_add(counts, …)`
/// and `counts = map_set(counts, …)` on a `let mut` `Map<string, i64>`; they
/// share this whole protocol.
pub(crate) fn is_same_owner_concat_source(value: &crate::ast::Expr, name: &str, ty: &Type) -> bool {
    let crate::ast::ExprKind::Call { name: callee, .. } = &value.kind else {
        return false;
    };
    let expected = match by_name(callee) {
        Some(StringOp::Concat) => Type::String,
        Some(op) if op.reopens_map() => Type::StringMap,
        _ => return false,
    };
    *ty == expected && is_same_owner_concat_shape(value, name)
}

/// The syntactic half of [`is_same_owner_concat_source`], without the binding
/// type.
pub(crate) fn is_same_owner_concat_shape(value: &crate::ast::Expr, name: &str) -> bool {
    let crate::ast::ExprKind::Call {
        name: callee,
        type_arguments,
        args,
    } = &value.kind
    else {
        return false;
    };
    by_name(callee).is_some_and(StringOp::is_same_owner_reopen)
        && type_arguments.is_empty()
        && args.len() == by_name(callee).map_or(0, StringOp::arity)
        && matches!(&args[0].kind, crate::ast::ExprKind::Var(source) if source == name)
        && !args[1..]
            .iter()
            .any(|argument| mentions_owner(argument, name))
}

impl StringOp {
    /// The operations whose first operand moves a same-owner `let mut`
    /// binding into the call: the String append and the two map reopens.
    pub(crate) fn is_same_owner_reopen(self) -> bool {
        self == StringOp::Concat || self.reopens_map()
    }
}

/// The first operand consumes the owner, so a second operand that also names
/// it (`string_concat(text, text)`) would read a moved value: that is not the
/// admitted reopen and must take the ordinary ownership diagnostic. A `Var`
/// prints as `Var("name")` while string-literal quotes print escaped, so the
/// structured debug form cannot confuse a literal with a reference.
fn mentions_owner(expression: &crate::ast::Expr, name: &str) -> bool {
    format!("{:?}", expression.kind).contains(&format!("Var({name:?})"))
}

/// Resolved-HIR twin of [`is_same_owner_concat_source`]. Hostile HIR that
/// never passed through source text must re-derive the identical fact.
pub(crate) fn is_same_owner_concat_hir(value: &crate::hir::ResolvedExpr, owner: &ValueId) -> bool {
    matches!(
        &value.kind,
        crate::hir::ResolvedExprKind::Call { callee, type_arguments, instance: None, args }
            if by_id(callee.as_str()).is_some_and(|op| {
                op.is_same_owner_reopen()
                    && args.len() == op.arity()
                    && value.ty == op.return_type()
            })
                && type_arguments.is_empty()
                && matches!(
                    &args[0].kind,
                    crate::hir::ResolvedExprKind::Place(place)
                        if &place.root == owner && place.projections.is_empty()
                )
                // As in the source twin: a later operand that reads the
                // consumed owner is not the admitted reopen.
                && !args[1..]
                    .iter()
                    .any(|argument| format!("{argument:?}").contains(&format!("{owner:?}")))
    )
}

/// Owned String Loops v1: every same-owner append
/// `text = string_concat(text, …)` in one function, keyed by its moving
/// first-operand place and mapped to the appended call value. Every other
/// owning String place read allocates a clone; these reads instead move the
/// binding's current generation into the call, so the assignment can publish
/// the next generation into the then-dead binding slot. The cleanup builder,
/// its independent replay, and every lowering derive this exact map.
pub(crate) fn same_owner_concat_appends(
    function: &crate::hir::ResolvedFunction,
) -> std::collections::BTreeMap<crate::hir::ExpressionId, crate::hir::ExpressionId> {
    use crate::hir::{ResolvedExprKind, ResolvedStatement};
    let mut appends = std::collections::BTreeMap::new();
    let mut pending = vec![&function.body];
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Block { statements, .. } = &expression.kind {
            for statement in statements {
                if let ResolvedStatement::Assign {
                    binding,
                    field: None,
                    value,
                    ..
                } = statement
                {
                    if let (true, ResolvedExprKind::Call { args, .. }) =
                        (is_same_owner_concat_hir(value, &binding.id), &value.kind)
                    {
                        appends.insert(args[0].id.clone(), value.id.clone());
                    }
                }
            }
        }
        pending.extend(crate::interpreter::trace_child_expressions(expression));
    }
    appends
}

/// The moving first-operand places of [`same_owner_concat_appends`].
pub(crate) fn same_owner_concat_operands(
    function: &crate::hir::ResolvedFunction,
) -> std::collections::BTreeSet<crate::hir::ExpressionId> {
    same_owner_concat_appends(function).into_keys().collect()
}

/// Cleanup order after a same-owner append publishes the next generation:
/// the binding keeps the position its previous generation held when it was
/// staged (`history`), so loop iterations and branch joins see one stable
/// initialization history. Flags initialized meanwhile keep their order after
/// the reserved history.
pub(crate) fn append_publication_order(
    history: &[crate::cleanup::LivenessFlagId],
    live: &[crate::cleanup::LivenessFlagId],
) -> Vec<crate::cleanup::LivenessFlagId> {
    history
        .iter()
        .filter(|flag| live.contains(flag))
        .chain(live.iter().filter(|flag| !history.contains(flag)))
        .copied()
        .collect()
}

/// Text Toolkit v1 is not lowered by any Core Wasm lane. Every lane refuses
/// it with this one stable diagnostic before emitting any operand.
pub(crate) fn text_toolkit_wasm_refusal(op: StringOp) -> crate::diagnostic::Diagnostic {
    crate::diagnostic::Diagnostic::io(
        "SPX-W116",
        format!(
            "{} operation `{}` is not lowered to Core Wasm; run it on the reference interpreter or native C11",
            if op.is_collection() {
                "String Collections v1"
            } else if op.is_conversion() {
                "Conversions v1"
            } else {
                "Text Toolkit v1"
            },
            op.name()
        ),
    )
}

/// Every Core Wasm lane refuses String Collections v1 and Conversions v1 up
/// front with the one stable `SPX-W116` diagnostic, naming the first such
/// operation in authored order, before any lane-specific admission can report a less
/// specific profile error.
pub(crate) fn refuse_collections_for_wasm(
    program: &crate::hir::ResolvedProgram,
) -> Result<(), crate::diagnostic::Diagnostic> {
    let mut pending = Vec::new();
    for function in program.functions.iter().chain(
        program
            .function_instances
            .iter()
            .map(|instance| &instance.function),
    ) {
        pending.push(&function.body);
        pending.extend(function.requires.iter().chain(&function.ensures));
    }
    pending.reverse();
    while let Some(expression) = pending.pop() {
        if let crate::hir::ResolvedExprKind::Call { callee, .. } = &expression.kind {
            if let Some(op) = by_id(callee.as_str()).filter(|op| {
                op.is_collection() || (op.is_conversion() && !op.is_integer_conversion())
            }) {
                return Err(text_toolkit_wasm_refusal(op));
            }
        }
        if expression.ty == ResolvedType::StringMap {
            return Err(text_toolkit_wasm_refusal(StringOp::MapNew));
        }
        crate::hir::push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    Ok(())
}

/// Bytewise three-way order of `string_compare`.
pub(crate) fn compare_bytes(left: &[u8], right: &[u8]) -> i64 {
    match left.cmp(right) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

/// The shared byte semantics of `string_find`: the first offset at or after
/// `from` where `needle` occurs. An empty needle matches at `from`.
pub(crate) fn find_bytes(text: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if from > text.len() {
        return None;
    }
    if needle.is_empty() {
        return Some(from);
    }
    text[from..]
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|offset| from + offset)
}

/// `string_to_i64`: an optional leading `-`, then one or more ASCII digits
/// and nothing else, within the i64 range. Everything else is `None`.
pub(crate) fn parse_i64(text: &[u8]) -> Option<i64> {
    let (negative, digits) = match text.split_first() {
        Some((b'-', rest)) => (true, rest),
        _ => (false, text),
    };
    if digits.is_empty() {
        return None;
    }
    let mut value: i64 = 0;
    for byte in digits {
        if !byte.is_ascii_digit() {
            return None;
        }
        let digit = i64::from(byte - b'0');
        value = value.checked_mul(10)?;
        value = if negative {
            value.checked_sub(digit)?
        } else {
            value.checked_add(digit)?
        };
    }
    Some(value)
}

/// ASCII whitespace for `string_trim`: space, tab, line feed, vertical tab,
/// form feed, and carriage return.
pub(crate) fn is_text_whitespace(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

/// The byte range `string_trim` keeps. Whitespace bytes are ASCII, so both
/// ends always lie on character boundaries.
pub(crate) fn trim_range(text: &[u8]) -> std::ops::Range<usize> {
    let start = text
        .iter()
        .position(|byte| !is_text_whitespace(*byte))
        .unwrap_or(text.len());
    let end = text
        .iter()
        .rposition(|byte| !is_text_whitespace(*byte))
        .map_or(start, |index| index + 1);
    start..end
}

/// Whether any resolved body or contract calls `op`.
pub(crate) fn program_uses_op(program: &crate::hir::ResolvedProgram, op: StringOp) -> bool {
    let mut pending = Vec::new();
    for function in program.functions.iter().chain(
        program
            .function_instances
            .iter()
            .map(|instance| &instance.function),
    ) {
        pending.push(&function.body);
        pending.extend(function.requires.iter().chain(&function.ensures));
    }
    while let Some(expression) = pending.pop() {
        if let crate::hir::ResolvedExprKind::Call { callee, .. } = &expression.kind {
            if by_id(callee.as_str()) == Some(op) {
                return true;
            }
        }
        crate::hir::push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    false
}
