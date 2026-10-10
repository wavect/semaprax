//! Finite checked nested request derivation; all output is ordinary source.
pub(super) mod descriptor;
mod emit;
mod materialize;
mod stream;
#[cfg(test)]
mod tests;
mod validate;

use super::refusal;
use crate::ast::{FieldDeclaration, Program, Type, TypeDeclaration, TypeDeclarationKind};
use crate::diagnostic::Diagnostic;
use std::fmt::Write as _;

fn fields(record: &TypeDeclaration) -> &[FieldDeclaration] {
    match &record.kind {
        TypeDeclarationKind::Record { fields } => fields,
        _ => unreachable!("authenticated record"),
    }
}
fn key_array(name: &str) -> String {
    format!(
        "[{}]",
        name.bytes()
            .map(|byte| format!("{byte}u8"))
            .collect::<Vec<_>>()
            .join(",")
    )
}
fn type_name(ty: &Type) -> String {
    match ty {
        Type::I64 => "i64".into(),
        Type::U8 => "u8".into(),
        Type::Usize => "usize".into(),
        Type::Bool => "bool".into(),
        Type::String => "string".into(),
        Type::Named { name, arguments } if arguments.is_empty() => name.clone(),
        _ => unreachable!("validated element"),
    }
}

pub(super) fn derive(
    program: &Program,
    root: &TypeDeclaration,
    max_string_bytes: usize,
    max_array_items: usize,
) -> Result<String, Vec<Diagnostic>> {
    let shape = descriptor::validate(program, root, max_string_bytes, max_array_items)?;
    let mut output = super::views::imports(program);
    output.push_str(&super::utf8::nested_decode_text(root, max_string_bytes));
    output.push_str(&emit::source(root, &shape, max_array_items));
    Ok(output)
}

/// One-call streaming adapter over a source-derived worst-spelling envelope.
pub(super) fn derive_stream(
    program: &Program,
    root: &TypeDeclaration,
    max_string_bytes: usize,
    max_array_items: usize,
) -> Result<String, Vec<Diagnostic>> {
    let shape = descriptor::validate(program, root, max_string_bytes, max_array_items)?;
    stream::validate_envelope(&shape.root, max_string_bytes, max_array_items)?;
    let normalization = super::views::stream_normalizer_source(program, root)?;
    let mut output = derive(program, root, max_string_bytes, max_array_items)?;
    output.push_str(&normalization);
    output.push_str(&stream::wrapper(root));
    Ok(output)
}
