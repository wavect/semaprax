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
