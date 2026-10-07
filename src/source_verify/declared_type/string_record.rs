//! Additive bounded String/Bytes record admission for private runtime values.
//! Schema-only records outside that executable profile retain SPX-T309.
use super::super::diagnostics::error;
use super::super::type_table::TypeTable;
use crate::ast::{Program, Span, Type, TypeDeclarationKind};
use crate::diagnostic::Diagnostic;

/// The first `string`-bearing field of a monomorphic authored record.
fn string_field<'a>(ty: &Type, types: &TypeTable<'a>) -> Option<(&'a str, &'a str)> {
    let Type::Named { name, arguments } = ty else {
        return None;
    };
    if !arguments.is_empty() {
        return None;
    }
    let declaration = types.declaration(name)?;
    if !declaration.type_parameters.is_empty() {
        return None;
    }
    let TypeDeclarationKind::Record { fields } = &declaration.kind else {
        return None;
    };
    fields
        .iter()
        .find(|field| types.contains_string(&field.ty))
        .map(|field| (declaration.name.as_str(), field.name.as_str()))
}

/// Push `SPX-T309` when `ty` is a string-bearing record used as `role`.
pub(in crate::source_verify) fn reject(
    program: &Program,
    ty: &Type,
    role: &str,
    span: Span,
    types: &TypeTable<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    if admitted(ty, types) {
        return false;
    }
    let Some((record, field)) = string_field(ty, types) else {
        return false;
    };
    diagnostics.push(
        error(
            program,
            "SPX-T309",
            format!(
                "record `{record}` carries `string` field `{field}`; a string-bearing record \
                 cannot be {role} in executable code"
            ),
            span,
        )
        .with_help(format!(
            "pass the `string` as its own parameter instead (`{field}: string`) and keep \
             Copy fields in the record, or carry it in a variant case taken as `own`"
        )),
    );
    true
}

/// Additive runtime record admission, independent of the frozen Bytes ABI.
pub(in crate::source_verify) fn admitted(root: &Type, types: &TypeTable<'_>) -> bool {
    if !matches!(root, Type::Named { .. }) {
        return false;
    }
    enum Frame {
        Enter(Type, usize),
        Leave(String),
    }
    let mut pending = vec![Frame::Enter(root.clone(), 1)];
    let mut active = std::collections::HashSet::new();
    let mut fields = 0usize;
    let mut leaves = 0usize;
    let mut text = false;
    while let Some(frame) = pending.pop() {
        match frame {
            Frame::Enter(Type::String, _) => {
                text = true;
                leaves += 1;
            }
            Frame::Enter(Type::Bytes, _) => {
                leaves += 1;
            }
            Frame::Enter(
                Type::I64
                | Type::I32
                | Type::U8
                | Type::Usize
                | Type::Char
                | Type::F32
                | Type::F64
                | Type::Bool,
                _,
            ) => {}
            Frame::Enter(Type::Named { name, arguments }, depth) => {
                if depth > crate::cleanup::MAX_CLEANUP_SHAPE_DEPTH {
                    return false;
                }
                let Some(declaration) = types.declaration(&name) else {
                    return false;
                };
                if !arguments.is_empty() || !declaration.type_parameters.is_empty() {
                    return false;
                }
                let TypeDeclarationKind::Record { fields: declared } = &declaration.kind else {
                    return false;
                };
                if !active.insert(name.clone()) {
                    return false;
                }
                fields += declared.len();
                if fields > crate::cleanup::MAX_CLEANUP_VISITED_FIELDS {
                    return false;
                }
                pending.push(Frame::Leave(name));
                pending.extend(
                    declared
                        .iter()
                        .rev()
                        .map(|field| Frame::Enter(field.ty.clone(), depth + 1)),
                );
            }
            Frame::Leave(name) => {
                active.remove(&name);
            }
            Frame::Enter(_, _) => return false,
        }
        if leaves > crate::cleanup::MAX_CLEANUP_OWNED_LEAVES {
            return false;
        }
    }
    text
}
