//! Scalar admission and the scalar spellings used by the C, Rust,
//! and descriptor projections.

use super::*;

pub(in crate::implementation) fn scalar_type(ty: &ResolvedType) -> Option<ScalarType> {
    match ty {
        ResolvedType::Unit => Some(ScalarType::Unit),
        ResolvedType::I64 => Some(ScalarType::I64),
        ResolvedType::Bool => Some(ScalarType::Bool),
        _ => None,
    }
}

pub(in crate::implementation) fn scalar_text(ty: ScalarType) -> &'static str {
    match ty {
        ScalarType::Unit => "unit",
        ScalarType::I64 => "i64",
        ScalarType::Bool => "bool",
    }
}

pub(in crate::implementation) fn c_type(ty: ScalarType) -> &'static str {
    match ty {
        ScalarType::Unit => "void",
        ScalarType::I64 => "int64_t",
        ScalarType::Bool => "uint8_t",
    }
}

pub(in crate::implementation) fn rust_type(ty: ScalarType) -> &'static str {
    match ty {
        ScalarType::Unit => "()",
        ScalarType::I64 => "i64",
        ScalarType::Bool => "bool",
    }
}

pub(in crate::implementation) fn rust_ffi_wire_type(ty: ScalarType) -> &'static str {
    match ty {
        ScalarType::Unit => "()",
        ScalarType::I64 => "i64",
        ScalarType::Bool => "u8",
    }
}
