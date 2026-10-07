//! Test-only owned-capacity accounting for resolved types in the cleanup
//! inventory's capacity high-water evidence.

use crate::hir::ResolvedType;

pub(super) fn resolved_type_owned_capacity(ty: &ResolvedType) -> usize {
    match ty {
        ResolvedType::Function { parameters, result } => {
            parameters.capacity() * std::mem::size_of::<ResolvedType>()
                + std::mem::size_of::<ResolvedType>()
                + parameters
                    .iter()
                    .map(resolved_type_owned_capacity)
                    .sum::<usize>()
                + resolved_type_owned_capacity(result)
        }
        ResolvedType::OnceFunction
        | ResolvedType::OnceFunctionI64
        | ResolvedType::OnceFunctionI64Pair
        | ResolvedType::MutFunctionI64
        | ResolvedType::Unit
        | ResolvedType::I64
        | ResolvedType::I32
        | ResolvedType::Char
        | ResolvedType::U8
        | ResolvedType::Usize
        | ResolvedType::ArrayU8(_)
        | ResolvedType::F32
        | ResolvedType::F64
        | ResolvedType::Bool => 0,
        ResolvedType::String
        | ResolvedType::Bytes
        | ResolvedType::Str
        | ResolvedType::SliceU8
        | ResolvedType::StringMap => 0,
        ResolvedType::TypeParameter { owner, .. } => owner.as_str().len(),
        ResolvedType::Nominal {
            declaration,
            arguments,
        } => {
            declaration.as_str().len()
                + arguments.capacity() * std::mem::size_of::<ResolvedType>()
                + arguments
                    .iter()
                    .map(resolved_type_owned_capacity)
                    .sum::<usize>()
        }
    }
}
