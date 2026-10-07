//! Scalar type projection shared by the property analyzer.
use super::*;

pub(super) fn scalar_type_text(ty: &Type) -> &'static str {
    match ty {
        Type::I64 => "i64",
        Type::I32 => "i32",
        Type::U8 => "u8",
        Type::Char => "char",
        Type::F32 => "f32",
        Type::F64 => "f64",
        Type::Bool => "bool",
        Type::Usize
        | Type::String
        | Type::Str
        | Type::SliceU8 | Type::StringMap
        | Type::ArrayU8(_)
        | Type::Bytes
        | Type::OnceFunction
        | Type::OnceFunctionI64
        | Type::OnceFunctionI64Pair
        | Type::MutFunctionI64
        | Type::Function { .. }
        | Type::Named { .. } => unreachable!(
            "scalar_type_text called for unsupported type `{:?}`; admitted scalars are the seven primitive Copy types",
            ty
        ),
    }
}
