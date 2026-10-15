//! Independent additive internal-record profile; public Bytes ABIs remain frozen.
use super::{DeclarationIndex, DeclarationKind, ResolvedType};
use std::borrow::Cow;
use std::collections::BTreeSet;

/// Ordinary internal record runtime only. Frozen Project selectors use
/// `admitted` so a new composition cannot widen their private ABI silently.
pub(crate) fn runtime_admitted(root: &ResolvedType, declarations: &DeclarationIndex) -> bool {
    admitted(root, declarations) || super::owned_collection_record::admitted(root, declarations)
}

pub(crate) fn admitted(root: &ResolvedType, declarations: &DeclarationIndex) -> bool {
    if !matches!(root, ResolvedType::Nominal { .. }) {
        return false;
    }
    enum Frame<'a> {
        Enter(&'a ResolvedType, usize),
        Leave,
    }
    let mut pending = vec![Frame::Enter(root, 1)];
    let mut active = Vec::new();
    let mut fields = 0usize;
    let mut leaves = 0usize;
    let mut text = false;
    while let Some(frame) = pending.pop() {
        match frame {
            Frame::Enter(ty, _) if crate::map_ops::is_collection(ty) => {
                text = true;
                leaves += 1;
            }
            Frame::Enter(ResolvedType::String, _) => {
                text = true;
                leaves += 1;
            }
            Frame::Enter(ResolvedType::Bytes, _) => {
                leaves += 1;
            }
            Frame::Enter(
                ResolvedType::I64
                | ResolvedType::I32
                | ResolvedType::U8
                | ResolvedType::Usize
                | ResolvedType::Char
                | ResolvedType::F32
                | ResolvedType::F64
                | ResolvedType::Bool,
                _,
            ) => {}
            Frame::Enter(ty @ ResolvedType::Nominal { .. }, depth) => {
                if depth > crate::cleanup::MAX_CLEANUP_SHAPE_DEPTH {
                    return false;
                }
                let ResolvedType::Nominal {
                    declaration,
                    arguments,
                } = ty
                else {
                    unreachable!()
                };
                if declarations.has_record_invariant(declaration)
                    || !arguments.is_empty()
                    || declarations
                        .type_parameters(declaration)
                        .is_none_or(|p| !p.is_empty())
                    || declarations
                        .declaration(declaration)
                        .is_none_or(|d| d.kind != DeclarationKind::Record)
                {
                    return false;
                }
                let Some(declared) = declarations.record_fields(declaration) else {
                    return false;
                };
                // Same complete structural identity; only the active path
                // participates in cycle detection, in the original order.
                if active.contains(&ty) {
                    return false;
                }
                active.push(ty);
                fields += declared.len();
                if fields > crate::cleanup::MAX_CLEANUP_VISITED_FIELDS {
                    return false;
                }
                pending.push(Frame::Leave);
                pending.extend(
                    declared
                        .iter()
                        .rev()
                        .map(|field| Frame::Enter(&field.ty, depth + 1)),
                );
            }
            Frame::Leave => {
                active.pop();
            }
            Frame::Enter(_, _) => return false,
        }
        if leaves > crate::cleanup::MAX_CLEANUP_OWNED_LEAVES {
            return false;
        }
    }
    text
}

/// Select a String-bearing record from exact declaration fields. This is a
/// separate walk from admission: hostile fields cannot hide behind a failed
/// or truncated admission traversal.
pub(crate) fn contains_string(root: &ResolvedType, declarations: &DeclarationIndex) -> bool {
    if !matches!(root, ResolvedType::Nominal { declaration, .. }
        if declarations.declaration(declaration).is_some_and(|item| item.kind == DeclarationKind::Record))
    {
        return false;
    }
    let mut pending = vec![Cow::Borrowed(root)];
    let mut visited = BTreeSet::new();
    while let Some(ty) = pending.pop() {
        if ty.as_ref() == &ResolvedType::String || crate::map_ops::is_collection(&ty) {
            return true;
        }
        if visited.contains(&ty) {
            continue;
        }
        if let ResolvedType::Nominal {
            declaration,
            arguments,
        } = ty.as_ref()
        {
            if let Some(fields) = declarations.record_fields(declaration) {
                for field in fields {
                    // Leaf and zero-argument nominal substitution is identity.
                    // Parameter and compound templates retain the exact fallible
                    // substitute_type route, including foreign-owner refusal.
                    let identity = match &field.ty {
                        ResolvedType::Nominal { arguments, .. } => arguments.is_empty(),
                        ResolvedType::Function { .. } | ResolvedType::TypeParameter { .. } => false,
                        ResolvedType::Never
                        | ResolvedType::OnceFunction
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
                        | ResolvedType::Bool
                        | ResolvedType::String
                        | ResolvedType::Bytes
                        | ResolvedType::Str
                        | ResolvedType::SliceU8
                        | ResolvedType::StringMap => true,
                    };
                    if identity {
                        pending.push(Cow::Borrowed(&field.ty));
                    } else if let Ok(ty) = super::substitute_type(&field.ty, declaration, arguments)
                    {
                        pending.push(Cow::Owned(ty));
                    }
                }
            }
        }
        // Move owned substitutions once; borrowed roots/fields stay references.
        visited.insert(ty);
    }
    false
}

pub(crate) fn validate_use(
    root: &ResolvedType,
    declarations: &DeclarationIndex,
) -> Result<(), crate::diagnostic::Diagnostic> {
    if contains_string(root, declarations) && !runtime_admitted(root, declarations) {
        return Err(super::hir_error(
            "String-bearing executable record is outside the bounded owned-text profile",
        ));
    }
    Ok(())
}

/// Additive owning-record selection for runtime String helpers. The ordinary
/// scalar signature traversal stays unchanged.
pub(crate) fn program_uses_strings(program: &super::ResolvedProgram) -> bool {
    program.types.iter().any(|declaration| {
        admitted(
            &ResolvedType::Nominal {
                declaration: declaration.id.clone(),
                arguments: Vec::new(),
            },
            &program.declarations,
        )
    })
}
