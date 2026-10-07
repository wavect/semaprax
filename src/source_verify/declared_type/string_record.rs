//! String-bearing records fail closed at executable use sites.
//!
//! A monomorphic authored `record` may declare `string` fields: projects use
//! such declarations as schemas for generated mirrors, and their invariants
//! still verify. No backend has an aggregate value layout for an owned
//! `string` leaf inside a record, and no ownership mode admits one as a
//! parameter or result, so a signature carrying one is refused here with a
//! stable diagnostic instead of an internal HIR failure.
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
