//! Machine shapes for the Wasm32 scalar carriers this lane lowers.
//!
//! These answer "what word does this scalar type occupy", which is a different
//! question from admission: the compiler-owned `Vec<T>` carrier is one `i64`
//! host handle for every admitted element, including the SPX-AI-019
//! owned-record element, and the union predicate in `wasm::vec_ops` is
//! what decides which carriers this lane lowers at all.
use super::*;

pub(super) fn scalar_wasm_type(
    program: &ResolvedProgram,
    ty: &ResolvedType,
) -> Result<u8, Diagnostic> {
    match ty {
        ResolvedType::I64 => Ok(I64),
        ResolvedType::I32 => Ok(I32),
        ResolvedType::Char => Ok(I32),
        ResolvedType::U8 => Ok(I32),
        ResolvedType::Usize => Ok(I64),
        ResolvedType::SliceU8 | ResolvedType::Str => Ok(I64),
        ResolvedType::Bytes => Ok(I64),
        ty if crate::wasm::vec_ops::is_wasm_owned_vec_type(program, ty) => Ok(I64),
        ty if crate::cleanup::is_owned_bounded_box_type(ty) => Ok(I64),
        ResolvedType::String => Ok(I64),
        ResolvedType::F32 => Ok(F32),
        ResolvedType::F64 => Ok(F64),
        ResolvedType::Bool => Ok(I32),
        ResolvedType::Function { .. } => Ok(I32),
        _ => Err(error(format!(
            "non-scalar type `{}` reached scalar aggregate lowering",
            ty.identity_key()
        ))),
    }
}

pub(super) fn vec_element_tag(ty: &ResolvedType) -> Result<i32, Diagnostic> {
    match ty {
        ResolvedType::I64 => Ok(1),
        ResolvedType::I32 => Ok(2),
        ResolvedType::U8 => Ok(3),
        ResolvedType::Usize => Ok(4),
        ResolvedType::Char => Ok(5),
        ResolvedType::F32 => Ok(6),
        ResolvedType::F64 => Ok(7),
        ResolvedType::Bool => Ok(8),
        _ => Err(error(
            "Vec element type is outside the admitted scalar profile",
        )),
    }
}

pub(super) fn scalar_local(value: &Value) -> Result<u32, Diagnostic> {
    match value {
        Value::Scalar { local, .. } => Ok(*local),
        _ => Err(error("Vec operation requires an exact scalar local")),
    }
}

pub(super) fn scalar_size_align(
    program: &ResolvedProgram,
    ty: &ResolvedType,
) -> Result<(u32, u32), Diagnostic> {
    match ty {
        ResolvedType::I64 => Ok((8, 8)),
        ResolvedType::I32 => Ok((4, 4)),
        ResolvedType::Char => Ok((4, 4)),
        ResolvedType::U8 => Ok((4, 4)),
        ResolvedType::Usize => Ok((8, 8)),
        ResolvedType::SliceU8 | ResolvedType::Str => Ok((8, 8)),
        ResolvedType::Bytes => Ok((8, 8)),
        ty if crate::wasm::vec_ops::is_wasm_owned_vec_type(program, ty) => Ok((8, 8)),
        ty if crate::cleanup::is_owned_bounded_box_type(ty) => Ok((8, 8)),
        ResolvedType::String => Ok((8, 8)),
        ResolvedType::F32 => Ok((4, 4)),
        ResolvedType::F64 => Ok((8, 8)),
        ResolvedType::Bool => Ok((4, 4)),
        ResolvedType::Function { .. } => Ok((4, 4)),
        _ => Err(error(format!(
            "non-scalar type `{}` has no Wasm32 scalar layout",
            ty.identity_key()
        ))),
    }
}
