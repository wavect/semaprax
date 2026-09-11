//! The C++ consumer emitter.

use super::{byte_literal, Declaration};

pub(super) fn emit(metadata: &str, declarations: &[Declaration]) -> Vec<(String, String)> {
    let mut types = String::new();
    for declaration in declarations {
        types.push_str(&format!("// {}\n", declaration.term));
        types.push_str(&format!("struct SpxPg{} {{\n", declaration.identifier));
        for (name, member) in &declaration.members {
            types.push_str(&format!("    {member} {name};\n"));
        }
        types.push_str("};\n\n");
    }
    let source = include_str!("cxx.txt")
        .replace("__DECLARATIONS__\n", &types)
        .replace("__EXPECTED_LEN__", &metadata.len().to_string())
        .replace("__EXPECTED__\n", &byte_literal(metadata, "    "));
    vec![("consumer.cpp".to_owned(), source)]
}
