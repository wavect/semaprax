//! Canonical integer-to-decimal string operations across all three runtime lanes.

use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::{codegen, format, graph, hir, parse, verify, wasm};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const SOURCE: &str = r#"
module test.numeric_text;

@id("numeric_text.check")
fn check() -> i64
{
    let negative = string_from_i64(-42);
    let maximum = string_from_usize(18446744073709551615usize);
    if negative == "-42" && maximum == "18446744073709551615" && string_len(negative) == 3 && string_len(maximum) == 20 { 7 } else { 9 }
}

@id("app.main")
fn main() -> i64
{
    check()
}
"#;

fn command_available(command: &str) -> bool {
    Command::new(command).arg("--version").output().is_ok()
}

fn fixture_path(suffix: &str) -> std::path::PathBuf {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "semaprax-numeric-text-{}-{id}.{suffix}",
        std::process::id()
    ))
}

#[test]
fn numeric_text_calls_round_trip_and_bind_reserved_identities() {
    let program = parse(SOURCE, Path::new("numeric-text.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    let reparsed = parse(&canonical, Path::new("numeric-text-canonical.spx")).unwrap();
    assert_eq!(format::canonical(&reparsed), canonical);
    assert_eq!(graph::revision(&program), graph::revision(&reparsed));
    let graph = graph::to_json(&program).unwrap();
    assert!(graph.contains("\"callee\":\"core.string.from_i64\""));
    assert!(graph.contains("\"callee\":\"core.string.from_usize\""));

    let resolved = hir::resolve(&program).unwrap();
    let rendered = format!("{resolved:#?}");
    assert!(rendered.contains("core.string.from_i64"));
    assert!(rendered.contains("core.string.from_usize"));
}

#[test]
fn numeric_text_argument_types_are_exact_and_names_are_reserved() {
    let wrong = parse(
        "module test.wrong; @id(\"app.main\") fn main() -> i64 { let text = string_from_i64(1usize); string_len(text) }",
        Path::new("numeric-text-wrong.spx"),
    )
    .unwrap();
    let diagnostics = verify::verify(&wrong);
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "SPX-T205"
            && diagnostic.message.contains("string_from_i64")
            && diagnostic.message.contains("expects i64")
    }));

    let shadow = parse(
        "module test.shadow; @id(\"test.shadow.call\") fn string_from_usize(value: usize) -> string { \"x\" } @id(\"app.main\") fn main() -> i64 { 0 }",
        Path::new("numeric-text-shadow.spx"),
    )
    .unwrap();
    assert!(verify::verify(&shadow)
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-S113"));
}

#[test]
fn reference_interpreter_renders_signed_and_portable_size_values() {
    let path = fixture_path("spx");
    std::fs::write(&path, format::canonical(&parse(SOURCE, &path).unwrap())).unwrap();
    let interpreted = interpreter::interpret(
        &path,
        "numeric_text.check",
        &[],
        &InterpreterOptions::default(),
    )
    .unwrap();
    let _ = std::fs::remove_file(&path);
    assert!(
        interpreted.envelope.contains("\"kind\":\"returned\"")
            && interpreted.envelope.contains("\"value\":\"7\""),
        "{}",
        interpreted.envelope
    );
}

#[test]
fn native_numeric_text_helpers_are_gated_and_execute() {
    let program = parse(SOURCE, Path::new("numeric-text-native.spx")).unwrap();
    let generated = codegen::emit_c(&program).unwrap();
    assert!(generated.contains("spx_string_from_i64(int64_t value)"));
    assert!(generated.contains("spx_string_from_usize(uint64_t value)"));

    let plain = parse(
        "module test.plain; @id(\"app.main\") fn main() -> i64 { 7 }",
        Path::new("numeric-text-plain.spx"),
    )
    .unwrap();
    let plain = codegen::emit_c(&plain).unwrap();
    assert!(!plain.contains("spx_string_from_i64(int64_t value)"));
    assert!(!plain.contains("spx_string_from_usize(uint64_t value)"));

    if command_available("clang") {
        let executable = fixture_path(std::env::consts::EXE_EXTENSION);
        codegen::build(&program, &executable).unwrap();
        let output = Command::new(&executable).output().unwrap();
        let _ = std::fs::remove_file(&executable);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "7");
    }
}

#[test]
fn core_wasm_numeric_text_operations_execute_in_node() {
    if !command_available("node") {
        return;
    }
    let program = parse(SOURCE, Path::new("numeric-text-wasm.spx")).unwrap();
    let bytes = wasm::emit_module(&program).unwrap();
    wasmparser::Validator::new().validate_all(&bytes).unwrap();
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("spx_string_from_i64"));
    assert!(text.contains("spx_string_from_usize"));

    let wasm_path = fixture_path("wasm");
    let script_path = fixture_path("mjs");
    std::fs::write(&wasm_path, bytes).unwrap();
    std::fs::write(
        &script_path,
        r#"import { readFile } from "node:fs/promises";
const bytes = await readFile(process.argv[2]);
const strings = new Map(); let next = 1;
const handle = value => { const id = next++; strings.set(id, value); return BigInt(id); };
let instance;
const env = {
  spx_add:(a,b)=>a+b, spx_sub:(a,b)=>a-b, spx_mul:(a,b)=>a*b,
  spx_div:(a,b)=>a/b, spx_rem:(a,b)=>a%b, spx_neg:a=>-a,
  spx_contract_fail:()=>{throw Error("contract")},
  spx_string_new:(ptr,len)=>handle(new TextDecoder().decode(new Uint8Array(instance.exports.memory.buffer,ptr,len))),
  spx_string_eq:(a,b)=>strings.get(Number(a))===strings.get(Number(b))?1:0,
  spx_string_clone:a=>handle(strings.get(Number(a))),
  spx_string_len:a=>BigInt(new TextEncoder().encode(strings.get(Number(a))).length),
  spx_string_concat:(a,b)=>handle(strings.get(Number(a))+strings.get(Number(b))),
  spx_string_from_i64:value=>handle(value.toString()),
  spx_string_from_usize:value=>handle(BigInt.asUintN(64,value).toString()),
};
({instance}=await WebAssembly.instantiate(bytes,{env}));
const result=instance.exports.semaprax_main();
if(result!==7n)throw Error(`unexpected result ${result}`);
console.log(result.toString());
"#,
    )
    .unwrap();
    let output = Command::new("node")
        .arg(&script_path)
        .arg(&wasm_path)
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&script_path);
    let _ = std::fs::remove_file(&wasm_path);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "7");
}
