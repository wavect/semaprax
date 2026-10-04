use super::*;

pub(in crate::source_verify) fn source_capacity_functions(
    program: &Program,
) -> Vec<(Option<&str>, &Function)> {
    let mut functions = program
        .functions
        .iter()
        .map(|function| (None, function))
        .collect::<Vec<_>>();
    for declaration in &program.types {
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
