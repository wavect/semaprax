//! Finite borrowed response projection over the independently checked request shape.
mod emit;
#[cfg(test)]
mod tests;

use super::nested_request::descriptor::{self, Kind, Record};
use super::refusal;
use crate::ast::{Program, Type, TypeDeclaration};
use crate::diagnostic::Diagnostic;
use std::fmt::Write as _;

pub(super) fn derive(
    program: &Program,
    root: &TypeDeclaration,
    max_string_bytes: usize,
    max_array_items: usize,
) -> Result<String, Vec<Diagnostic>> {
    let shape = descriptor::validate_response(program, root, max_string_bytes, max_array_items)?;
    validate_reads(&shape.root)?;
    let mut output = format!("module {};\n", program.module);
    for (module, name, alias) in [
        ("digits", "i64_len", "jv_i64_len"),
        ("write", "usize_len", "jv_usize_len"),
        ("utf8", "scalar_at", "ju_raw_scalar"),
        ("utf8", "sequence_end", "ju_raw_end"),
        ("utf8", "utf8_end", "ju_raw_utf8_end"),
    ] {
        writeln!(output, "use function @id(\"std.data.json.{module}.{name}\") from std.data.json.{module} as {alias};").unwrap();
    }
    output.push_str(&super::utf8::nested_response_text(root, max_string_bytes));
    output.push_str(&emit::source(root, &shape.root, max_array_items));
    Ok(output)
}

fn validate_reads(record: &Record<'_>) -> Result<(), Vec<Diagnostic>> {
    for field in &record.fields {
        match &field.kind {
            Kind::Record(child) => validate_reads(child)?,
            Kind::Vector { kind, .. } if matches!(**kind, Kind::Text) => {
                // Primitive String vectors have owning copy-out but no admitted
                // borrowed element projection. Do not silently clone in preflight.
                return Err(refusal(
                    "nested response requires scalar or flat record Vec elements; primitive String Vec has no nonallocating borrowed read",
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

fn owns(record: &Record<'_>) -> bool {
    record.fields.iter().any(|field| match &field.kind {
        Kind::Text | Kind::Vector { .. } => true,
        Kind::Record(child) => owns(child),
        Kind::Scalar(_) => false,
    })
}
fn type_name(ty: &Type) -> String {
    match ty {
        Type::I64 => "i64".into(),
        Type::U8 => "u8".into(),
        Type::Usize => "usize".into(),
        Type::Bool => "bool".into(),
        Type::Named { name, arguments } if arguments.is_empty() => name.clone(),
        _ => unreachable!("validated borrowed element"),
    }
}
fn literal(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}
