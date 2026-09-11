//! The C11 consumer emitter.

use super::{byte_literal, template, Declaration};

pub(super) fn emit(metadata: &str, declarations: &[Declaration]) -> Vec<(String, String)> {
    let mut types = String::new();
    for declaration in declarations {
        types.push_str(&format!("/* {} */\n", declaration.term));
        types.push_str(&format!("struct spx_pg_{} {{\n", declaration.identifier));
        for (name, member) in &declaration.members {
            types.push_str(&format!("    {member} {name};\n"));
        }
        types.push_str("};\n\n");
    }
    let source = template(include_str!("c.txt"))
        .replace("__DECLARATIONS__\n", &types)
        .replace("__EXPECTED__\n", &byte_literal(metadata, "    "));
    vec![("consumer.c".to_owned(), source)]
}
