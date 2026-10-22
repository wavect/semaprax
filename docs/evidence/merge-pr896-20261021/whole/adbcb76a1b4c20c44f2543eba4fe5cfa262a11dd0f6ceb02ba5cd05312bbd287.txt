//! Exact finite defaults used by ordinary primitive/string imported signatures.
use crate::ast::{Expr, ExprKind, Span};
const ZERO: Span = Span {
    start: 0,
    end: 0,
    line: 0,
    column: 0,
};
pub(super) static I64: Expr = Expr {
    kind: ExprKind::Int(0),
    span: ZERO,
};
pub(super) static I32: Expr = Expr {
    kind: ExprKind::Int32(0),
    span: ZERO,
};
pub(super) static CHAR: Expr = Expr {
    kind: ExprKind::Char(0),
    span: ZERO,
};
pub(super) static U8: Expr = Expr {
    kind: ExprKind::Uint8(0),
    span: ZERO,
};
pub(super) static USIZE: Expr = Expr {
    kind: ExprKind::Usize(0),
    span: ZERO,
};
pub(super) static F32: Expr = Expr {
    kind: ExprKind::Float32(0),
    span: ZERO,
};
pub(super) static F64: Expr = Expr {
    kind: ExprKind::Float64(0),
    span: ZERO,
};
pub(super) static BOOL: Expr = Expr {
    kind: ExprKind::Bool(false),
    span: ZERO,
};

pub(super) static STRING: Expr = Expr {
    kind: ExprKind::String(String::new()),
    span: ZERO,
};

pub(super) static EMPTY: Vec<Expr> = Vec::new();

/// Construction is private: a raw provider body is never its effective body.
/// The ordinary producer admits the complete check bank before calling this.
#[derive(Debug)]
pub(crate) struct ScalarSignature<'a> {
    // Publishing consumes this exact field, without a forwarding helper ABI.
    // Only FunctionView's private provider word selects the presentation;
    // wrapping a retained Some-provider view cannot forge a finite view.
    pub(crate) view: super::FunctionView<'a>,
}
impl<'a> ScalarSignature<'a> {
    pub(crate) fn checked(
        provider: &'a crate::ast::Program,
        original: &'a crate::ast::Function,
        alias: &'a str,
    ) -> Option<Self> {
        use crate::ast::{ParamMode, Type};
        let mut scalar = original.type_parameters.is_empty()
            && original.yields.is_none()
            && original.follows.is_none()
            && !original.stable_id.starts_with("std.")
            && matches!(
                original.return_type,
                Type::I64
                    | Type::I32
                    | Type::Char
                    | Type::U8
                    | Type::Usize
                    | Type::F32
                    | Type::F64
                    | Type::Bool
                    | Type::String
            );
        // Complete bounded checks: the caller's source-length invoice pays
        // exactly these iterations, including both terminal iterator results.
        for param in &original.params {
            scalar &= param.mode == ParamMode::Value
                && matches!(
                    param.ty,
                    Type::I64
                        | Type::I32
                        | Type::Char
                        | Type::U8
                        | Type::Usize
                        | Type::F32
                        | Type::F64
                        | Type::Bool
                        | Type::String
                );
        }
        let mut member = false;
        for function in &provider.functions {
            member |= std::ptr::eq(function, original);
        }
        if !scalar || !member {
            return None;
        }

        Some(Self {
            view: super::FunctionView {
                _provider: None,
                original,
                _alias: alias,
            },
        })
    }
}
