//! Borrowed import-signature roots, never a source-visible type namespace.
use super::{authority::view::Lookup, debit, identity, refusal};
use crate::ast::{ModuleUseKind, Program, Type, TypeDeclaration};
use crate::diagnostic::Diagnostic;

pub(super) fn visit<'a, A: Lookup<'a>>(
    caller: &Program,
    authored: &A,
    programs: &'a [Program],
    work: &mut usize,
    mut found: impl FnMut(&'a str, &'a TypeDeclaration, bool, &mut usize) -> Result<(), Vec<Diagnostic>>,
) -> Result<(), Vec<Diagnostic>> {
    debit(work, caller.module_uses.len())?;
    for import in &caller.module_uses {
        if !matches!(import.kind, ModuleUseKind::Type | ModuleUseKind::Function) {
            continue;
        }
        debit(
            work,
            import
                .persistent_id
                .len()
                .checked_add(import.target_module.len())
                .and_then(|n| n.checked_add(1))
                .ok_or_else(refusal)?,
        )?;
        let target = authored
            .get(import.persistent_id.as_str())
            .ok_or_else(refusal)?;
        debit(work, programs.len())?;
        let provider = programs
            .iter()
            .find(|p| p.module == target.module && p.path == target.path)
            .ok_or_else(refusal)?;
        if !target.explicit || target.module != import.target_module {
            return Err(refusal());
        }
        if import.kind == ModuleUseKind::Type {
            let declaration = target.ty.ok_or_else(refusal)?;
            debit(work, provider.types.len())?;
            if !provider.types.iter().any(|held| {
                std::ptr::eq(held, declaration) && held.stable_id == import.persistent_id
            }) {
                return Err(refusal());
            }
            found(target.module, declaration, false, work)?;
            continue;
        }
        let function = target.function.ok_or_else(refusal)?;
        debit(work, provider.functions.len())?;
        if !provider
            .functions
            .iter()
            .any(|held| std::ptr::eq(held, function))
            || !function.explicit_id
            || function.stable_id != import.persistent_id
        {
            return Err(refusal());
        }
        // Genuine generic signatures retain their previous explicit route.
        if !function.type_parameters.is_empty()
            || function.yields.is_some()
            || function.follows.is_some()
        {
            continue;
        }
        debit(work, function.params.len() + 1)?;
        for ty in
            std::iter::once(&function.return_type).chain(function.params.iter().map(|p| &p.ty))
        {
            let Type::Named { name, arguments } = ty else {
                continue;
            };
            let id = identity(target.module, name, programs, work)?;
            let vector = id == Some(crate::prelude::VEC_ID) || (id.is_none() && name == "Vec");
            let (name, arguments) = if vector {
                let [Type::Named { name, arguments }] = arguments.as_slice() else {
                    continue;
                };
                (name, arguments)
            } else {
                (name, arguments)
            };
            if !arguments.is_empty() {
                continue;
            }
            let id = identity(target.module, name, programs, work)?.ok_or_else(refusal)?;
            let root = authored.get(id).ok_or_else(refusal)?;
            let declaration = root.ty.ok_or_else(refusal)?;
            debit(work, programs.len())?;
            let held = programs
                .iter()
                .find(|p| p.module == root.module && p.path == root.path)
                .ok_or_else(refusal)?;
            debit(work, held.types.len())?;
            if !held.types.iter().any(|ty| std::ptr::eq(ty, declaration))
                || !root.explicit
                || declaration.stable_id != id
                || !declaration.explicit_id
            {
                return Err(refusal());
            }
            if vector {
                let fields = match &declaration.kind {
                    crate::ast::TypeDeclarationKind::Record { fields } => fields.len(),
                    _ => return Err(refusal()),
                };
                debit(work, fields.checked_mul(2).ok_or_else(refusal)?)?;
                if declaration.extends.is_some()
                    || !super::super::type_rewrite::record_element(declaration)
                {
                    return Err(refusal());
                }
            }
            found(root.module, declaration, true, work)?;
        }
    }
    Ok(())
}
