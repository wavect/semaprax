//! Compiler-owned string operation intrinsics v1.
//!
//! Three prelude-style intrinsic functions are admitted wherever owned
//! `string` values are already admitted. They carry reserved stable
//! identities in the compiler-owned `core.string.*` family so a call site
//! resolves to an ordinary monomorphic [`crate::hir::ResolvedExprKind::Call`]
//! node; no source declaration, prelude contract byte, or graph schema
//! version participates, so programs that never name the operations keep
//! byte-identical projections.
//!
//! - `string_len(s: string) -> i64` borrows its operand for a read.
//! - `string_concat(a: string, b: string) -> string` consumes both operands
//!   by move and returns one new owned string.
//! - `string_is_empty(s: string) -> bool` borrows its operand for a read.

use crate::ast::{Param, ParamMode, Span, Type};
use crate::hir::{OwnershipMode, ResolvedParam, ResolvedType, ValueId};

pub(crate) const LEN_NAME: &str = "string_len";
pub(crate) const CONCAT_NAME: &str = "string_concat";
pub(crate) const IS_EMPTY_NAME: &str = "string_is_empty";

pub(crate) const LEN_ID: &str = "core.string.len";
pub(crate) const CONCAT_ID: &str = "core.string.concat";
pub(crate) const IS_EMPTY_ID: &str = "core.string.is_empty";

/// One admitted string operation intrinsic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StringOp {
    /// Borrowed byte-length read.
    Len,
    /// Consuming concatenation of two owned strings.
    Concat,
    /// Borrowed emptiness read.
    IsEmpty,
}

impl StringOp {
    pub(crate) fn name(self) -> &'static str {
        match self {
            StringOp::Len => LEN_NAME,
            StringOp::Concat => CONCAT_NAME,
            StringOp::IsEmpty => IS_EMPTY_NAME,
        }
    }

    pub(crate) fn id(self) -> &'static str {
        match self {
            StringOp::Len => LEN_ID,
            StringOp::Concat => CONCAT_ID,
            StringOp::IsEmpty => IS_EMPTY_ID,
        }
    }

    pub(crate) fn arity(self) -> usize {
        match self {
            StringOp::Len | StringOp::IsEmpty => 1,
            StringOp::Concat => 2,
        }
    }

    /// Source parameter names in left-to-right order; they only label
    /// diagnostics because the operations have no authored declaration.
    pub(crate) fn param_names(self) -> &'static [&'static str] {
        match self {
            StringOp::Len | StringOp::IsEmpty => &["s"],
            StringOp::Concat => &["a", "b"],
        }
    }

    pub(crate) fn consumes_arguments(self) -> bool {
        matches!(self, StringOp::Concat)
    }

    pub(crate) fn return_type(self) -> ResolvedType {
        match self {
            StringOp::Len => ResolvedType::I64,
            StringOp::Concat => ResolvedType::String,
            StringOp::IsEmpty => ResolvedType::Bool,
        }
    }

    pub(crate) fn ast_return_type(self) -> Type {
        match self {
            StringOp::Len => Type::I64,
            StringOp::Concat => Type::String,
            StringOp::IsEmpty => Type::Bool,
        }
    }
}

/// Resolve a source-level call name to its intrinsic operation.
pub(crate) fn by_name(name: &str) -> Option<StringOp> {
    match name {
        LEN_NAME => Some(StringOp::Len),
        CONCAT_NAME => Some(StringOp::Concat),
        IS_EMPTY_NAME => Some(StringOp::IsEmpty),
        _ => None,
    }
}

/// Resolve a resolved-callee identity to its intrinsic operation.
pub(crate) fn by_id(id: &str) -> Option<StringOp> {
    match id {
        LEN_ID => Some(StringOp::Len),
        CONCAT_ID => Some(StringOp::Concat),
        IS_EMPTY_ID => Some(StringOp::IsEmpty),
        _ => None,
    }
}

/// Synthetic HIR parameters for one operation: consuming arguments carry
/// `Own` ownership exactly like an ordinary declared `string` parameter, and
/// borrowed arguments accept every argument ownership without a transfer.
pub(crate) fn resolved_params(op: StringOp) -> Vec<ResolvedParam> {
    let consumption = if op.consumes_arguments() {
        OwnershipMode::Own
    } else {
        OwnershipMode::Borrow
    };
    op.param_names()
        .iter()
        .enumerate()
        .map(|(index, name)| ResolvedParam {
            id: ValueId::intrinsic_parameter(op.id(), index),
            name: (*name).to_owned(),
            ownership: consumption,
            ty: ResolvedType::String,
            span: Span::default(),
        })
        .collect()
}

/// Synthetic AST parameters for source verification. Consuming arguments use
/// the established `own` transfer mode; borrowed arguments use the plain
/// value mode that never marks its sources moved.
pub(crate) fn ast_params(op: StringOp) -> Vec<Param> {
    let mode = if op.consumes_arguments() {
        ParamMode::Own
    } else {
        ParamMode::Value
    };
    op.param_names()
        .iter()
        .map(|name| Param {
            name: (*name).to_owned(),
            mode,
            ty: Type::String,
            span: Span::default(),
        })
        .collect()
}
