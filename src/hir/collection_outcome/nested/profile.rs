//! Negative guards for frozen profiles, including unused module functions.
use crate::hir::*;
use std::collections::BTreeSet;

pub(crate) fn function_requires_profile_by<'a>(
    function: &ResolvedFunction,
    lookup: impl Fn(&DeclarationId) -> Option<&'a ResolvedTypeDeclaration>,
) -> bool {
    fn owning_record<'a>(
        root: &ResolvedType,
        lookup: &impl Fn(&DeclarationId) -> Option<&'a ResolvedTypeDeclaration>,
    ) -> bool {
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = root
        else {
            return false;
        };
        let Some(item) = lookup(declaration) else {
            return !crate::prelude::is_compiler_owned_id(declaration.as_str());
        };
        if !matches!(item.kind, ResolvedTypeDeclarationKind::Record { .. }) {
            return false;
        }
        let mut pending = vec![(declaration.clone(), arguments.clone())];
        let mut visited = BTreeSet::new();
        let mut work = 0usize;
        while let Some((id, arguments)) = pending.pop() {
            if !visited.insert((id.clone(), arguments.clone())) {
                continue;
            }
            let Some(item) = lookup(&id) else {
                return true;
            };
            let ResolvedTypeDeclarationKind::Record { fields } = &item.kind else {
                return true;
            };
            work = match work.checked_add(fields.len()) {
                Some(n) if n <= crate::cleanup::MAX_CLEANUP_VISITED_FIELDS => n,
                _ => return true,
            };
            for field in fields {
                let Ok(ty) = substitute_type(&field.ty, &id, &arguments) else {
                    return true;
                };
                match ty {
                    ResolvedType::String | ResolvedType::Bytes => return true,
                    ResolvedType::Nominal { declaration, .. }
                        if declaration.as_str() == crate::prelude::VEC_ID =>
                    {
                        return true
                    }
                    ResolvedType::Nominal {
                        declaration,
                        arguments,
                    } if !crate::prelude::is_compiler_owned_id(declaration.as_str()) => {
                        pending.push((declaration, arguments))
                    }
                    _ => {}
                }
            }
        }
        false
    }
    let contains = |ty: &ResolvedType| {
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = ty
        else {
            return false;
        };
        if crate::prelude::is_compiler_owned_id(declaration.as_str()) {
            return false;
        }
        let Some(item) = lookup(declaration) else {
            return true;
        };
        let ResolvedTypeDeclarationKind::Variant { cases } = &item.kind else {
            return false;
        };
        cases.iter().flat_map(|case| &case.fields).any(|field| {
            substitute_type(&field.ty, declaration, arguments)
                .map_or(true, |ty| owning_record(&ty, &lookup))
        })
    };
    if contains(&function.return_type) || function.params.iter().any(|p| contains(&p.ty)) {
        return true;
    }
    let mut found = false;
    crate::hir::function_value::walk(function, |expr| found |= contains(&expr.ty));
    found
}

pub(crate) fn program_requires_profile(program: &ResolvedProgram) -> bool {
    program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
        .any(|function| {
            function_requires_profile_by(function, |id| {
                program.types.iter().find(|item| &item.id == id)
            })
        })
}
