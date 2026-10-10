//! A finite response source projection. Ordinary Project admission remains authoritative.
mod descriptor;
mod emit;
mod record;
#[cfg(test)]
mod tests;

use crate::ast::{FieldDeclaration, Program, Type, TypeDeclaration, TypeDeclarationKind};
use crate::diagnostic::Diagnostic;
use std::fmt::Write as _;

use super::refusal;

fn fields(record: &TypeDeclaration) -> &[FieldDeclaration] {
    match &record.kind {
        TypeDeclarationKind::Record { fields } => fields,
        _ => unreachable!("validated record"),
    }
}

fn literal(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

pub(super) fn source(
    program: &Program,
    root: &TypeDeclaration,
    max_string_bytes: usize,
) -> Result<String, Vec<Diagnostic>> {
    let shape = descriptor::validate(program, root, max_string_bytes)?;
    let mut out = format!("module {};\n", program.module);
    for (module, name, alias) in [
        ("digits", "i64_len", "jv_i64_len"),
        ("write", "usize_len", "jv_usize_len"),
        ("utf8", "scalar_at", "ju_raw_scalar"),
        ("utf8", "sequence_end", "ju_raw_end"),
        ("utf8", "utf8_end", "ju_raw_utf8_end"),
    ] {
        writeln!(out, "use function @id(\"std.data.json.{module}.{name}\") from std.data.json.{module} as {alias};").unwrap();
    }
    out.push_str(&super::utf8::response_text(shape.row, max_string_bytes));
    out.push_str(&record::source(shape.row));
    out.push_str(&record::indexed_source(shape.row));
    out.push_str(&record::source(shape.metrics));
    out.push_str(&emit::source(root, &shape));
    Ok(out)
}
