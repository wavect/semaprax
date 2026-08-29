use std::collections::BTreeMap;

use crate::diagnostic::quote_json;

use super::{FlatOwnedRecordApiDescriptor, FlatOwnedRecordExport, PublicApiParameterType};

pub fn render_flat_owned_record_typescript(descriptor: &FlatOwnedRecordApiDescriptor) -> String {
    let mut records = BTreeMap::<&str, &FlatOwnedRecordExport>::new();
    for export in &descriptor.exports {
        records.entry(&export.record_host_name).or_insert(export);
    }
    let mut output = String::new();
    for (name, export) in records {
        output.push_str("export interface ");
        output.push_str(name);
        output.push_str(" {\n");
        for field in &export.fields {
            output.push_str("  readonly ");
            output.push_str(&field.host_name);
            output.push_str(": ");
            output.push_str(field.ty.typescript());
            output.push_str(";\n");
        }
        output.push_str("}\n");
    }
    output.push_str("export interface SemapraxApi {\n");
    for export in &descriptor.exports {
        output.push_str("  readonly ");
        output.push_str(&quote_json(export.typescript_name()));
        output.push_str(": (");
        for (index, (_, _, ty)) in export.parameters.iter().enumerate() {
            if index != 0 {
                output.push_str(", ");
            }
            output.push_str("arg");
            output.push_str(&index.to_string());
            output.push_str(": ");
            output.push_str(parameter_typescript(*ty));
        }
        output.push_str(") => ");
        output.push_str(&export.record_host_name);
        output.push_str(";\n");
    }
    output.push_str("}\n");
    output
}

pub fn render_flat_owned_record_rust(descriptor: &FlatOwnedRecordApiDescriptor) -> String {
    let mut records = BTreeMap::<&str, &FlatOwnedRecordExport>::new();
    for export in &descriptor.exports {
        records.entry(&export.record_host_name).or_insert(export);
    }
    let mut output = String::from("#![forbid(unsafe_code)]\n#[derive(Clone, Debug, Eq, PartialEq)]\npub struct CallError { message: &'static str }\nimpl CallError { pub fn message(&self) -> &str { self.message } }\n");
    for (name, export) in records {
        output.push_str("#[derive(Clone, Debug, Eq, PartialEq)]\npub struct ");
        output.push_str(name);
        output.push_str(" {\n");
        for field in &export.fields {
            output.push_str("    pub ");
            output.push_str(&field.host_name);
            output.push_str(": ");
            output.push_str(field.ty.rust());
            output.push_str(",\n");
        }
        output.push_str("}\n");
    }
    output.push_str("pub trait SemapraxApi {\n");
    for export in &descriptor.exports {
        output.push_str("    fn ");
        output.push_str(export.rust_method_name());
        output.push_str("(&self");
        for (index, (_, _, ty)) in export.parameters.iter().enumerate() {
            output.push_str(", arg");
            output.push_str(&index.to_string());
            output.push_str(": ");
            output.push_str(parameter_rust(*ty));
        }
        output.push_str(") -> Result<");
        output.push_str(&export.record_host_name);
        output.push_str(", CallError>;\n");
    }
    output.push_str("}\n");
    output
}

fn parameter_typescript(ty: PublicApiParameterType) -> &'static str {
    match ty {
        PublicApiParameterType::I64 => "bigint",
        PublicApiParameterType::Bool => "boolean",
        PublicApiParameterType::BorrowStr => "string",
        PublicApiParameterType::BorrowSliceU8 => "Uint8Array",
    }
}

fn parameter_rust(ty: PublicApiParameterType) -> &'static str {
    match ty {
        PublicApiParameterType::I64 => "i64",
        PublicApiParameterType::Bool => "bool",
        PublicApiParameterType::BorrowStr => "&str",
        PublicApiParameterType::BorrowSliceU8 => "&[u8]",
    }
}
