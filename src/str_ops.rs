//! Compiler-owned operations over non-escaping borrowed UTF-8 `str` views.
//!
//! These operations are deliberately distinct from the owned `string`
//! operations. They never allocate, clone, consume, retain, or return their
//! borrowed inputs. Calls resolve through reserved `core.str.*` identities so
//! every semantic consumer sees an ordinary monomorphic call with explicit
//! borrow ownership.

use crate::ast::{Param, ParamMode, Span, Type};
use crate::hir::{OwnershipMode, ResolvedParam, ResolvedType, ValueId};

pub(crate) const LEN_BYTES_NAME: &str = "str_len_bytes";
pub(crate) const IS_EMPTY_NAME: &str = "str_is_empty";
pub(crate) const STARTS_WITH_NAME: &str = "str_starts_with";
pub(crate) const CONTAINS_NAME: &str = "str_contains";

pub(crate) const LEN_BYTES_ID: &str = "core.str.len_bytes";
pub(crate) const IS_EMPTY_ID: &str = "core.str.is_empty";
pub(crate) const STARTS_WITH_ID: &str = "core.str.starts_with";
pub(crate) const CONTAINS_ID: &str = "core.str.contains";

/// Per-root length bound for borrowed UTF-8 input. The invocation boundary
/// combines text and arbitrary-byte roots under the shared Useful Data
/// cumulative budget; derived and forwarded views do not recharge it.
pub(crate) const MAX_BORROWED_STR_BYTES: usize = 65_536;

/// Allocation-bounded KMP over exact UTF-8 bytes.  UTF-8 validity is already
/// established at every profile boundary; searching bytes preserves embedded
/// NUL and never observes scalar boundaries or locale state.
pub(crate) fn contains(value: &str, needle: &str) -> Option<bool> {
    // The invocation boundary owns cumulative charging. This local check is
    // only a defensive bound on the fixed-width KMP prefix representation.
    if value.len() > MAX_BORROWED_STR_BYTES || needle.len() > MAX_BORROWED_STR_BYTES {
        return None;
    }
    if needle.is_empty() {
        return Some(true);
    }
    if needle.len() > value.len() {
        return Some(false);
    }

    let needle = needle.as_bytes();
    let mut prefix = vec![0_u16; needle.len()];
    let mut matched = 0_usize;
    for index in 1..needle.len() {
        while matched != 0 && needle[matched] != needle[index] {
            matched = usize::from(prefix[matched - 1]);
        }
        if needle[matched] == needle[index] {
            matched += 1;
        }
        prefix[index] = matched as u16;
    }

    matched = 0;
    for byte in value.bytes() {
        while matched != 0 && needle[matched] != byte {
            matched = usize::from(prefix[matched - 1]);
        }
        if needle[matched] == byte {
            matched += 1;
            if matched == needle.len() {
                return Some(true);
            }
        }
    }
    Some(false)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StrOp {
    LenBytes,
    IsEmpty,
    StartsWith,
    Contains,
}

impl StrOp {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::LenBytes => LEN_BYTES_NAME,
            Self::IsEmpty => IS_EMPTY_NAME,
            Self::StartsWith => STARTS_WITH_NAME,
            Self::Contains => CONTAINS_NAME,
        }
    }

    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::LenBytes => LEN_BYTES_ID,
            Self::IsEmpty => IS_EMPTY_ID,
            Self::StartsWith => STARTS_WITH_ID,
            Self::Contains => CONTAINS_ID,
        }
    }

    pub(crate) const fn arity(self) -> usize {
        match self {
            Self::LenBytes | Self::IsEmpty => 1,
            Self::StartsWith | Self::Contains => 2,
        }
    }

    pub(crate) const fn param_names(self) -> &'static [&'static str] {
        match self {
            Self::LenBytes | Self::IsEmpty => &["value"],
            Self::StartsWith => &["value", "prefix"],
            Self::Contains => &["value", "needle"],
        }
    }

    pub(crate) fn param_types(self) -> &'static [ResolvedType] {
        match self {
            Self::LenBytes | Self::IsEmpty => &[ResolvedType::Str],
            Self::StartsWith | Self::Contains => &[ResolvedType::Str, ResolvedType::Str],
        }
    }

    pub(crate) fn return_type(self) -> ResolvedType {
        match self {
            Self::LenBytes => ResolvedType::I64,
            Self::IsEmpty | Self::StartsWith | Self::Contains => ResolvedType::Bool,
        }
    }

    pub(crate) fn ast_return_type(self) -> Type {
        match self {
            Self::LenBytes => Type::I64,
            Self::IsEmpty | Self::StartsWith | Self::Contains => Type::Bool,
        }
    }
}

pub(crate) fn by_name(name: &str) -> Option<StrOp> {
    match name {
        LEN_BYTES_NAME => Some(StrOp::LenBytes),
        IS_EMPTY_NAME => Some(StrOp::IsEmpty),
        STARTS_WITH_NAME => Some(StrOp::StartsWith),
        CONTAINS_NAME => Some(StrOp::Contains),
        _ => None,
    }
}

pub(crate) fn by_id(id: &str) -> Option<StrOp> {
    match id {
        LEN_BYTES_ID => Some(StrOp::LenBytes),
        IS_EMPTY_ID => Some(StrOp::IsEmpty),
        STARTS_WITH_ID => Some(StrOp::StartsWith),
        CONTAINS_ID => Some(StrOp::Contains),
        _ => None,
    }
}

pub(crate) fn resolved_params(op: StrOp) -> Vec<ResolvedParam> {
    op.param_names()
        .iter()
        .enumerate()
        .map(|(index, name)| ResolvedParam {
            id: ValueId::intrinsic_parameter(op.id(), index),
            name: (*name).to_owned(),
            ownership: OwnershipMode::Borrow,
            ty: ResolvedType::Str,
            span: Span::default(),
        })
        .collect()
}

pub(crate) fn ast_params(op: StrOp) -> Vec<Param> {
    op.param_names()
        .iter()
        .map(|name| Param {
            name: (*name).to_owned(),
            mode: ParamMode::Borrow,
            ty: Type::Str,
            span: Span::default(),
        })
        .collect()
}
