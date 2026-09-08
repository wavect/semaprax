//! Backend-only Function Value v1 regressions for table immediates and
//! callback-only helpers that have no concrete reference expression.

use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::{parse, verify, wasm};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn checked(source: &str) -> semaprax::ast::Program {
    let program = parse(source, Path::new("function-values-backend.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    program
}

fn temporary(suffix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "semaprax-function-value-backend-{}-{}.{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
        suffix
    ))
}

#[test]
fn function_table_index_uses_signed_i32_immediates() {
    let targets = (0..=64)
        .map(|index| format!("@id(\"fv.target.{index:02}\") fn target_{index:02}(value: i64) -> i64 {{ {index} }}"))
        .collect::<Vec<_>>()
        .join("\n");
    let retainers = (0..64)
        .map(|index| {
            format!(
                "@id(\"fv.retain.{index:02}\") fn retain_{index:02}() -> fn(i64) -> i64 {{ target_{index:02} }}"
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let source = format!(
        "module test.function_value_table;\n{targets}\n{retainers}\n@id(\"fv.choose\") fn choose() -> fn(i64) -> i64 {{ target_64 }}\n@id(\"app.main\") fn main() -> i64 {{ let callback = choose(); callback(0) }}\n"
    );
    let program = checked(&source);
    let bytes = wasm::emit_module(&program).unwrap();
    wasmparser::Validator::new().validate_all(&bytes).unwrap();
    let mut has_index_64 = false;
    for payload in wasmparser::Parser::new(0).parse_all(&bytes) {
        if let wasmparser::Payload::CodeSectionEntry(body) = payload.unwrap() {
            for operator in body.get_operators_reader().unwrap() {
                if matches!(
                    operator.unwrap(),
                    wasmparser::Operator::I32Const { value: 64 }
                ) {
                    has_index_64 = true;
                }
            }
        }
    }
    assert!(has_index_64, "the 65th function table slot must remain +64");

    let node = Command::new("node").arg("--version").output().is_ok();
    assert!(
        node || std::env::var_os("SPX_REQUIRE_NODE").is_none(),
        "SPX_REQUIRE_NODE requires node for Function Value v1 table evidence"
    );
    if node {
        let root = temporary("web");
        wasm::build_web(&program, &root).unwrap();
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/verify-web.mjs");
        let output = Command::new("node")
            .arg(script)
            .arg(&root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "64");
        let _ = std::fs::remove_dir_all(root);
    }
}

#[test]
fn callback_only_helper_interns_indirect_signature_without_a_reference() {
    let source = r#"
module test.function_value_empty_table;
@id("fv.apply") fn apply(callback: fn(i64) -> i64, value: i64) -> i64 { callback(value) }
@id("app.main") fn main() -> i64 { 0 }
"#;
    let bytes = wasm::emit_module(&checked(source)).unwrap();
    wasmparser::Validator::new().validate_all(&bytes).unwrap();
}
