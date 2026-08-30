use super::*;
use std::path::Path;

const SOURCE: &str = r#"
module test.standalone_strings;
@id("s.keep")
fn keep(value: string) -> string { value }
@id("s.flag")
fn flag(value: bool) -> bool { value }
@id("s.main")
fn main() -> i64 {
    let value = keep("a\u{0}λ");
    match string_len(value) {
        4 if string_contains(value, "λ") => 42,
        _ => 0,
    }
}
"#;

fn program(source: &str) -> Program {
    let program = crate::parse(source, Path::new("standalone-strings.spx")).unwrap();
    let diagnostics = crate::verify::verify(&program);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    program
}

#[test]
fn canonical_selection_validates_guarded_module_and_exact_closed_inventory() {
    let source = program(SOURCE);
    let canonical = crate::format::canonical(&source);
    let reparsed = program(&canonical);
    assert_eq!(
        crate::graph::revision(&source),
        crate::graph::revision(&reparsed)
    );
    let first = emit_module(
        &source,
        &["s.main".into(), "s.flag".into()],
        InternalStringOptions::default(),
    )
    .unwrap();
    let second = emit_module(
        &reparsed,
        &["s.flag".into(), "s.main".into()],
        InternalStringOptions::default(),
    )
    .unwrap();
    assert_eq!(first.wasm_bytes(), second.wasm_bytes());
    assert_eq!(first.descriptor(), second.descriptor());
    assert_eq!(first.runtime_source(), second.runtime_source());
    wasmparser::Validator::new()
        .validate_all(first.wasm_bytes())
        .unwrap();
    let mut imports = Vec::new();
    let mut exports = Vec::new();
    let mut memories = 0;
    for payload in wasmparser::Parser::new(0).parse_all(first.wasm_bytes()) {
        match payload.unwrap() {
            wasmparser::Payload::ImportSection(section) => {
                for import in section.into_imports() {
                    let import = import.unwrap();
                    assert_eq!(import.module, "semaprax.internal-strings.v1");
                    imports.push(import.name.to_owned());
                }
            }
            wasmparser::Payload::ExportSection(section) => {
                for export in section {
                    exports.push(export.unwrap().name.to_owned());
                }
            }
            wasmparser::Payload::MemorySection(section) => {
                for memory in section {
                    let memory = memory.unwrap();
                    assert_eq!(memory.initial, 4);
                    assert_eq!(memory.maximum, Some(4));
                    assert!(!memory.shared && !memory.memory64);
                    memories += 1;
                }
            }
            wasmparser::Payload::StartSection { .. } | wasmparser::Payload::TableSection(_) => {
                panic!("unexpected authority")
            }
            _ => {}
        }
    }
    assert_eq!(memories, 1);
    assert_eq!(
        imports,
        [
            "literal",
            "clone",
            "concat",
            "from_char",
            "byte_len",
            "char_len",
            "eq",
            "starts_with",
            "contains",
            "drop"
        ]
    );
    assert_eq!(
        exports,
        [
            "memory",
            "__spx_stack_pointer",
            "__spx_call_0",
            "__spx_call_1"
        ]
    );
}

#[test]
fn selection_and_options_fail_closed_without_widening_public_strings() {
    let source = program(SOURCE);
    for ids in [
        vec![],
        vec!["s.main".into(), "s.main".into()],
        vec!["s.keep".into()],
        vec!["absent".into()],
    ] {
        assert_eq!(
            emit_module(&source, &ids, InternalStringOptions::default())
                .unwrap_err()
                .code,
            "SPX-W111"
        );
    }
    for options in [
        InternalStringOptions {
            max_string_bytes: 65_537,
            ..Default::default()
        },
        InternalStringOptions {
            max_live_bytes: 16_777_217,
            ..Default::default()
        },
        InternalStringOptions {
            max_cumulative_bytes: 67_108_865,
            ..Default::default()
        },
        InternalStringOptions {
            max_live_owners: Some(0),
            ..Default::default()
        },
        InternalStringOptions {
            max_live_owners: Some(65_537),
            ..Default::default()
        },
    ] {
        assert_eq!(
            emit_module(&source, &["s.main".into()], options)
                .unwrap_err()
                .code,
            "SPX-W111"
        );
    }
    let zero = InternalStringOptions {
        max_string_bytes: 0,
        max_live_bytes: 0,
        max_cumulative_bytes: 0,
        max_live_owners: Some(1),
    };
    assert!(emit_module(&source, &["s.main".into()], zero).is_ok());
    let recursive = program("module test.recursive; @id(\"r.main\") fn main() -> i64 { main() }");
    assert_eq!(
        emit_module(
            &recursive,
            &["r.main".into()],
            InternalStringOptions::default()
        )
        .unwrap_err()
        .code,
        "SPX-W111"
    );
}

#[test]
fn string_mutation_and_direct_string_loop_storage_keep_source_rejections() {
    for (body, code) in [
        ("let mut value = \"x\"; value = \"y\"; 0", "SPX-U105"),
        (
            "let mut index = 0; while index < 1 { let value = \"x\"; index = index + 1; 0 } 0",
            "SPX-T252",
        ),
    ] {
        let source = format!("module test.rejected; @id(\"r.main\") fn main() -> i64 {{ {body} }}");
        let program = crate::parse(&source, Path::new("rejected.spx")).unwrap();
        assert!(crate::verify::verify(&program)
            .iter()
            .any(|diagnostic| diagnostic.code == code));
        assert!(emit_module(
            &program,
            &["r.main".into()],
            InternalStringOptions::default()
        )
        .is_err());
    }
}
