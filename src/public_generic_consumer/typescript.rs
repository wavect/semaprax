//! The TypeScript/Wasm consumer emitter: the ES module a Wasm host would load
//! beside the ambient declarations a TypeScript author compiles against.

use super::{byte_literal, Declaration};

pub(super) fn emit(metadata: &str, declarations: &[Declaration]) -> Vec<(String, String)> {
    let module =
        include_str!("typescript.txt").replace("__EXPECTED__\n", &byte_literal(metadata, "  "));
    let mut types = String::new();
    for declaration in declarations {
        types.push_str(&format!("/** {} */\n", declaration.term));
        types.push_str(&format!(
            "export declare interface SpxPg{} {{\n",
            declaration.identifier
        ));
        for (name, member) in &declaration.members {
            types.push_str(&format!("  {name}: {member};\n"));
        }
        types.push_str("}\n\n");
    }
    let declarations =
        include_str!("typescript-declarations.txt").replace("__DECLARATIONS__\n", &types);
    vec![
        ("consumer.mjs".to_owned(), module),
        ("consumer.d.ts".to_owned(), declarations),
    ]
}
