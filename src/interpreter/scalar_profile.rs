//! Shared Copy-scalar admission and match semantics for the interpreter.

use crate::hir::ResolvedType;

use super::Value;

pub(super) fn is_admitted_resolved_scalar(ty: &ResolvedType) -> bool {
    matches!(
        ty,
        ResolvedType::I64
            | ResolvedType::I32
            | ResolvedType::U8
            | ResolvedType::Usize
            | ResolvedType::F32
            | ResolvedType::F64
            | ResolvedType::Char
            | ResolvedType::Bool
    )
}

pub(super) fn pattern_value_matches(value: &Value, pattern: crate::hir::PatternValue) -> bool {
    match (value, pattern) {
        (Value::Int(actual), crate::hir::PatternValue::Int(expected)) => *actual == expected,
        (Value::Int32(actual), crate::hir::PatternValue::Int32(expected)) => *actual == expected,
        (Value::Uint8(actual), crate::hir::PatternValue::Uint8(expected)) => *actual == expected,
        (Value::Usize(actual), crate::hir::PatternValue::Usize(expected)) => *actual == expected,
        (Value::Char(actual), crate::hir::PatternValue::Char(expected)) => *actual == expected,
        (Value::Bool(actual), crate::hir::PatternValue::Bool(expected)) => *actual == expected,
        _ => false,
    }
}
