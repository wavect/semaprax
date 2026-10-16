use super::*;
use crate::ast::program_source::ProgramSource;

pub(in crate::source_verify) fn source_capacity_functions(
    program: &dyn ProgramSource,
) -> Vec<(Option<&str>, &Function)> {
    let mut functions = program
        .functions()
        .iter()
        .map(|function| (None, function))
        .collect::<Vec<_>>();
    for declaration in program.types() {
        if let TypeDeclarationKind::Class { methods, .. } = &declaration.kind {
            functions.extend(
                methods
                    .iter()
                    .map(|method| (Some(declaration.name.as_str()), method)),
            );
        }
    }
    functions
}

pub(super) fn source_capacity_slot(
    slots: &mut Vec<crate::byte_data_capacity::ArrayStorageSlot>,
    types: &TypeTable<'_>,
    identity: String,
    kind: crate::byte_data_capacity::ArrayStorageKind,
    ty: &Type,
) -> Result<(), ()> {
    let length = source_array_payload(types, ty)?;
    if length != 0 || matches!(ty, Type::ArrayU8(0)) {
        slots.push(crate::byte_data_capacity::ArrayStorageSlot {
            identity,
            kind,
            length,
        });
    }
    Ok(())
}

pub(in crate::source_verify) fn source_array_payload(
    types: &TypeTable<'_>,
    ty: &Type,
) -> Result<u32, ()> {
    let mut total = 0_u32;
    let mut pending = vec![ty.clone()];
    let mut expanded = 0_usize;
    while let Some(ty) = pending.pop() {
        expanded = expanded.checked_add(1).ok_or(())?;
        if expanded > 65_536 {
            return Err(());
        }
        match ty {
            Type::ArrayU8(length) => total = total.checked_add(length).ok_or(())?,
            Type::Named { .. } => {
                if let Some(fields) = types.record_fields(&ty) {
                    for field in fields.iter().rev() {
                        pending.push(types.record_field_type(&ty, field).ok_or(())?);
                    }
                }
            }
            Type::Never
            | Type::I64
            | Type::I32
            | Type::Char
            | Type::U8
            | Type::Usize
            | Type::F32
            | Type::F64
            | Type::Bool
            | Type::String
            | Type::Bytes
            | Type::Str
            | Type::SliceU8
            | Type::StringMap
            | Type::OnceFunction
            | Type::OnceFunctionI64
            | Type::OnceFunctionI64Pair
            | Type::MutFunctionI64
            | Type::Function { .. } => {}
        }
    }
    Ok(total)
}
