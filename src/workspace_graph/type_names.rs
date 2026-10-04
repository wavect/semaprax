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
        | Type::OnceFunctionI64Pair
        | Type::MutFunctionI64
        | Type::Function { .. } => false,
        Type::Named { name, arguments } => {
            names.contains(name.as_str())
                || arguments
                    .iter()
                    .any(|argument| type_contains_name_from(argument, names))
        }
    }
}

pub(super) fn type_reference_is_admitted(
    module: &str,
    ty: &Type,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
    visiting: &mut BTreeSet<String>,
) -> bool {
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
        | Type::Str => true,
        Type::SliceU8
        | Type::ArrayU8(_)
        | Type::OnceFunction
        | Type::OnceFunctionI64
        | Type::OnceFunctionI64Pair
        | Type::MutFunctionI64
        | Type::Function { .. } => false,
        Type::Bytes => true,
        Type::Named { name, arguments } if arguments.is_empty() => {
            let Some(program) = programs.iter().find(|item| item.module == module) else {
                return false;
            };
            let local_target = program
                .types
                .iter()
                .find(|item| item.name == *name)
                .and_then(|item| authored.get(item.stable_id.as_str()));
            if local_target.is_some_and(|target| {
                target.ty.is_some_and(|declaration| {
                    type_is_admitted(module, declaration, authored, programs, visiting)
                })
            }) {
                return true;
            }
            program
                .module_uses
                .iter()
                .find(|item| item.kind == ModuleUseKind::Type && item.alias == *name)
                .and_then(|item| authored.get(item.persistent_id.as_str()))
                .is_some_and(|target| {
                    target.ty.is_some_and(|declaration| {
                        type_is_admitted(target.module, declaration, authored, programs, visiting)
                    })
                })
        }
        Type::Named { .. } => false,
    }
}
