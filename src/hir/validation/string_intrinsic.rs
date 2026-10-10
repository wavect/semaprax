//! Authored/retained declarations cannot impersonate reserved String operations.
use super::*;

pub(super) fn reject_reserved_identities(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    for declaration in program.declarations.declarations() {
        if crate::string_ops::by_id(declaration.id.as_str()).is_some()
            || (declaration.kind == DeclarationKind::Function
                && crate::string_ops::by_name(&declaration.name).is_some())
        {
            return Err(hir_error(format!(
                "resolved {:?} declaration `{}` aliases a compiler-owned string operation",
                declaration.kind, declaration.id
            )));
        }
    }
    for function in program.functions.iter().chain(
        program
            .function_instances
            .iter()
            .map(|instance| &instance.function),
    ) {
        reject_function(&function.id, &function.name)?;
    }
    for template in &program.function_templates {
        reject_function(&template.id, &template.name)?;
    }
    Ok(())
}

fn reject_function(id: &DeclarationId, name: &str) -> Result<(), Diagnostic> {
    if crate::string_ops::by_id(id.as_str()).is_some() || crate::string_ops::by_name(name).is_some()
    {
        return Err(hir_error(format!(
            "resolved function `{id}` aliases a compiler-owned string operation"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
