//! Scalar admission and the scalar spellings used by the C, Rust,
//! and descriptor projections.

use super::*;

pub(in crate::implementation) fn scalar_type(ty: &ResolvedType) -> Option<ScalarType> {
    match ty {
        ResolvedType::Unit => Some(ScalarType::Unit),
        ResolvedType::I64 => Some(ScalarType::I64),
        ResolvedType::Bool => Some(ScalarType::Bool),
        ty if ty.is_compiler_i64_result() => Some(ScalarType::ResultI64I64),
        _ => None,
    }
}

pub(in crate::implementation) fn scalar_text(ty: ScalarType) -> &'static str {
    match ty {
        ScalarType::Unit => "unit",
        ScalarType::I64 => "i64",
        ScalarType::Bool => "bool",
        ScalarType::ResultI64I64 => "result<i64,i64>",
    }
}

pub(in crate::implementation) fn c_type(ty: ScalarType) -> &'static str {
    match ty {
        ScalarType::Unit => "void",
        ScalarType::I64 => "int64_t",
        ScalarType::Bool => "uint8_t",
        ScalarType::ResultI64I64 => "spxnr_result_i64_i64_v1",
    }
}

pub(in crate::implementation) fn rust_type(ty: ScalarType) -> &'static str {
    match ty {
        ScalarType::Unit => "()",
        ScalarType::I64 => "i64",
        ScalarType::Bool => "bool",
        ScalarType::ResultI64I64 => "core::result::Result<i64,i64>",
    }
}

pub(in crate::implementation) fn rust_ffi_wire_type(ty: ScalarType) -> &'static str {
    match ty {
        ScalarType::Unit => "()",
        ScalarType::I64 => "i64",
        ScalarType::Bool => "u8",
        ScalarType::ResultI64I64 => "ResultWire",
    }
}
