//! Borrowed shape reconstruction for the deliberately narrow private closure.
use super::authority::view::Lookup;
use super::{identity, private_without_work, refusal};
use crate::ast::{ModuleUseKind, Program, Type, TypeDeclaration, TypeDeclarationKind};
use crate::diagnostic::Diagnostic;

pub(super) fn debit(work: &mut usize, count: usize) -> Result<(), Vec<Diagnostic>> {
    *work = work.checked_sub(count).ok_or_else(refusal)?;
    Ok(())
}
// Prove complete absence by provider, so repeated root imports do not repeatedly
// scan the same provider. This grants no shape or visibility authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Absence {
    Absent,
    Possible,
    WorkExhausted,
}
fn observe(work: &mut usize, count: usize) -> bool {
    if let Some(left) = work.checked_sub(count) {
        *work = left;
        true
    } else {
        *work = 0;
        false
    }
}
pub(super) fn explicit_only<'x, A: Lookup<'x>>(
    caller: &Program,
    authored: &A,
    programs: &[Program],
    work: &mut usize,
) -> Result<Absence, Vec<Diagnostic>> {
    if !observe(work, caller.module_uses.len()) {
        return Ok(Absence::WorkExhausted);
    }
    for import in &caller.module_uses {
        if !matches!(import.kind, ModuleUseKind::Type | ModuleUseKind::Function) {
            continue;
        }
        let target = authored.get(import.persistent_id.as_str()).ok_or_else(|| {
            // Signature validation may request a closure before the ordinary
            // import loop reaches a later missing module. Keep its diagnostic
            // and source span instead of reporting a private-layout limit.
            if import.target_module == caller.module
                || !programs.iter().any(|p| p.module == import.target_module)
            {
                vec![crate::workspace_graph::use_error(
                    caller,
                    import,
                    "target module is missing or equals the caller module",
                )]
            } else {
                refusal()
            }
        })?;
        if !target.explicit || target.module != import.target_module {
            return Err(refusal());
        }
    }
    if !observe(work, programs.len()) {
        return Ok(Absence::WorkExhausted);
    }
    for provider in programs {
        if !observe(work, caller.module_uses.len()) {
            return Ok(Absence::WorkExhausted);
        }
        if !caller.module_uses.iter().any(|u| {
            matches!(u.kind, ModuleUseKind::Type | ModuleUseKind::Function)
                && u.target_module == provider.module
        }) {
            continue;
        }
        if !observe(work, provider.types.len() + provider.module_uses.len()) {
            return Ok(Absence::WorkExhausted);
        }
        for declaration in &provider.types {
            if !observe(work, caller.module_uses.len()) {
                return Ok(Absence::WorkExhausted);
            }
            if private_without_work(caller, &provider.module, &declaration.stable_id) {
                return Ok(Absence::Possible);
            }
        }
        for import in &provider.module_uses {
            if import.kind != ModuleUseKind::Type {
                continue;
            }
            if !observe(work, caller.module_uses.len()) {
                return Ok(Absence::WorkExhausted);
            }
            if !caller
                .module_uses
                .iter()
                .any(|u| u.kind == ModuleUseKind::Type && u.persistent_id == import.persistent_id)
            {
                return Ok(Absence::Possible);
            }
        }
    }
    Ok(Absence::Absent)
}
pub(super) fn scalar(ty: &Type) -> bool {
    matches!(
        ty,
        Type::I64
            | Type::I32
            | Type::U8
            | Type::Usize
            | Type::Char
            | Type::F32
            | Type::F64
            | Type::Bool
    )
}
fn header(declaration: &TypeDeclaration) -> bool {
    declaration.explicit_id
        && declaration.type_parameters.is_empty()
        && declaration.invariants().is_empty()
        && declaration.extends.is_none()
}
pub(super) fn validate_root<'x, A: Lookup<'x>>(
    declaration: &TypeDeclaration,
    module: &str,
    authored: &A,
    programs: &[Program],
    work: &mut usize,
) -> Result<(), Vec<Diagnostic>> {
    if !header(declaration) {
        return Err(refusal());
    }
    match &declaration.kind {
        TypeDeclarationKind::Record { .. } => Ok(()),
        TypeDeclarationKind::Variant { cases } => {
            debit(work, cases.len())?;
            let [a, b] = cases.as_slice() else {
                return Err(refusal());
            };
            let error = |c: &crate::ast::VariantCaseDeclaration| matches!(c.fields.as_slice(), [code, offset, field] if code.ty == Type::I64 && offset.ty == Type::Usize && field.ty == Type::I64);
            let payload = if error(b) {
                a
            } else if error(a) {
                b
            } else {
                return Err(refusal());
            };
            let [field] = payload.fields.as_slice() else {
                return Err(refusal());
            };
            let Type::Named { name, arguments } = &field.ty else {
                return Err(refusal());
            };
            if !arguments.is_empty() {
                return Err(refusal());
            }
            let id = identity(module, name, programs, work)?.ok_or_else(refusal)?;
            let target = authored.get(id).ok_or_else(refusal)?;
            if target.explicit
                && target.ty.is_some_and(|d| {
                    header(d) && matches!(d.kind, TypeDeclarationKind::Record { .. })
                })
            {
                Ok(())
            } else {
                Err(refusal())
            }
        }
        TypeDeclarationKind::Class { .. } | TypeDeclarationKind::Resource { .. } => Err(refusal()),
    }
}
pub(super) fn validate_declaration(
    declaration: &TypeDeclaration,
    total: &mut usize,
    work: &mut usize,
) -> Result<(), Vec<Diagnostic>> {
    if !header(declaration) {
        return Err(refusal());
    }
    let count = match &declaration.kind {
        TypeDeclarationKind::Record { fields } => {
            debit(work, fields.len())?;
            if fields.iter().any(|f| !f.explicit_id) {
                return Err(refusal());
            }
            fields.len()
        }
        TypeDeclarationKind::Variant { cases } => {
            debit(work, cases.len())?;
            let mut count = 0usize;
            for case in cases {
                debit(work, case.fields.len())?;
                if !case.explicit_id || case.fields.iter().any(|f| !f.explicit_id) {
                    return Err(refusal());
                }
                count = count.checked_add(case.fields.len()).ok_or_else(refusal)?;
            }
            count
        }
        TypeDeclarationKind::Class { .. } | TypeDeclarationKind::Resource { .. } => {
            return Err(refusal())
        }
    };
    *total = total.checked_add(count).ok_or_else(refusal)?;
    if *total > crate::cleanup::MAX_CLEANUP_VISITED_FIELDS {
        return Err(refusal());
    }
    Ok(())
}
