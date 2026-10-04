//! Structural referenced type-name inspection.
use super::*;

pub(super) fn type_contains_name_from(ty: &Type, names: &BTreeSet<&str>) -> bool {
    match ty {
        Type::I64
        | Type::I32
        | Type::Char
        | Type::U8
        | Type::Usize
        | Type::F32
        | Type::F64
        | Type::Bool
        | Type::String
        | Type::Str
        | Type::SliceU8
        | Type::ArrayU8(_)
        | Type::Bytes
        | Type::OnceFunction
        | Type::OnceFunctionI64
        | Type::Function { .. } => false,
        Type::Named { name, arguments } => {
            names.contains(name.as_str())
                || arguments
                    .iter()
                    .any(|argument| type_contains_name_from(argument, names))
        }
    }
}
