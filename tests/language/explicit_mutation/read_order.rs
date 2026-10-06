//! Issue #561: an earlier operand or argument that reads a mutable Copy
//! scalar keeps the value it read, even when a later operand or argument
//! assigns to the same binding. Every backend that claims Explicit Mutation
//! v1 and Field Mutation v1 must agree with the interpreter, including which
//! checked arithmetic failure (if any) the evaluated values select.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::{codegen, format, parse, verify, wasm};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

/// The issue's first reproduction, verbatim: `11` on a correct backend.
const ISSUE_BINARY_AND_CALL: &str = r#"
module audit.native_read_order;

@id("audit.first")
fn first(left: i64, right: i64) -> i64
{
    left
}

@id("audit.main")
fn main() -> i64
{
    let mut value = 1;
    let binary_result = value + { value = 2; 0 };
    value = 1;
    let call_result = first(value, { value = 2; 0 });
    binary_result * 10 + call_result
}
"#;

/// The issue's second reproduction: the already-read `1` plus `1` is `2`,
/// not an invented addition overflow.
const ISSUE_FALSE_OVERFLOW: &str = r#"
module audit.native_read_overflow;

@id("audit.main")
fn main() -> i64
{
    let mut value = 1;
    value + { value = 9223372036854775807; 1 }
}
"#;

/// The converse: the earlier read is `i64::MAX`, so the addition must select
/// the overflow even though the later operand stores `0` first.
const SELECTED_OVERFLOW: &str = r#"
module audit.native_read_selected_overflow;

@id("audit.main")
fn main() -> i64
{
    let mut value = 9223372036854775807;
    value + { value = 0; 1 }
}
"#;

/// One bit per representative shape; a correct backend returns all ones.
const SHAPES: &str = r#"
module audit.native_read_shapes;

@id("shape.point")
record Point {
    @id("shape.point.x")
    x: i64,
    @id("shape.point.y")
    y: i64,
}

@id("shape.first")
fn first(left: i64, right: i64) -> i64 { left }

@id("shape.first_i32")
fn first_i32(left: i32, right: i32) -> i32 { left }

@id("shape.pick")
fn pick(left: bool, right: bool) -> i64 { if left { 1 } else { 0 } }

@id("shape.binary_i64")
fn binary_i64() -> i64 { let mut v = 1; if v + { v = 2; 0 } == 1 { 1 } else { 0 } }

@id("shape.call_i64")
fn call_i64() -> i64 { let mut v = 1; if first(v, { v = 2; 0 }) == 1 { 1 } else { 0 } }

@id("shape.compare_i32")
fn compare_i32() -> i64 { let mut v = 1i32; if v == { v = 5i32; 1i32 } { 1 } else { 0 } }

@id("shape.call_i32")
fn call_i32() -> i64 { let mut v = 2i32; if first_i32(v, { v = 4i32; 0i32 }) == 2i32 { 1 } else { 0 } }

@id("shape.compare_u8")
fn compare_u8() -> i64 { let mut v = 7u8; if v == { v = 9u8; 7u8 } { 1 } else { 0 } }

@id("shape.add_usize")
fn add_usize() -> i64 { let mut v = 3usize; let s = v + { v = 10usize; 1usize }; if s == 4usize { 1 } else { 0 } }

@id("shape.compare_char")
fn compare_char() -> i64 { let mut v = 'a'; if v == { v = 'b'; 'a' } { 1 } else { 0 } }

@id("shape.add_f64")
fn add_f64() -> i64 { let mut v = 1.5f64; let s = v + { v = 10.0f64; 0.5f64 }; if s == 2.0f64 { 1 } else { 0 } }

@id("shape.compare_f32")
fn compare_f32() -> i64 { let mut v = 1.5f32; if v < { v = 9.0f32; 2.0f32 } { 1 } else { 0 } }

@id("shape.call_bool")
fn call_bool() -> i64 { let mut v = true; pick(v, { v = false; true }) }

@id("shape.field")
fn field() -> i64 { let mut p = Point { x: 1, y: 2 }; if p.x + { p.x = 20; 0 } == 1 { 1 } else { 0 } }

@id("shape.lazy_and")
fn lazy_and() -> i64 { let mut v = true; if v && { v = false; true } { if v { 0 } else { 1 } } else { 0 } }

@id("shape.lazy_or_skips")
fn lazy_or_skips() -> i64 { let mut v = true; if v || { v = false; true } { if v { 1 } else { 0 } } else { 0 } }

@id("shape.main")
fn main() -> i64
{
    binary_i64() + call_i64() * 2 + compare_i32() * 4 + call_i32() * 8
        + compare_u8() * 16 + add_usize() * 32 + compare_char() * 64 + add_f64() * 128
        + compare_f32() * 256 + call_bool() * 512 + field() * 1024
        + lazy_and() * 2048 + lazy_or_skips() * 4096
}
"#;

/// A whole Copy record passed before a later argument mutates one of its
/// fields. The reference interpreter does not admit record-argument calls,
/// so this shape compares native C against Core Wasm only.
const RECORD_ARGUMENT: &str = r#"
module audit.native_read_record_argument;

@id("record.point")
record Point {
    @id("record.point.x")
    x: i64,
    @id("record.point.y")
    y: i64,
}

@id("record.point_x")
fn point_x(point: Point, ignored: i64) -> i64 { point.x }

@id("record.main")
fn main() -> i64 { let mut p = Point { x: 1, y: 2 }; point_x(p, { p.x = 30; 0 }) }
"#;

enum Expected {
    Value(i64),
    AdditionOverflow,
}

fn command_available(command: &str) -> bool {
    Command::new(command).arg("--version").output().is_ok()
}

fn required_or_available(command: &str, requirement: &str) -> bool {
    let available = command_available(command);
    assert!(
        available || std::env::var_os(requirement).is_none(),
        "{requirement} requires {command} for read-order runtime evidence",
    );
    available
}

fn fixture_path(suffix: &str) -> PathBuf {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "semaprax-mutation-read-order-{}-{id}.{suffix}",
        std::process::id()
    ))
}

fn checked(source: &str) -> semaprax::ast::Program {
    let program = parse(source, Path::new("mutation-read-order.spx")).unwrap();
    let diagnostics = verify::verify(&program);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let canonical = format::canonical(&program);
    assert_eq!(
        canonical,
        format::canonical(&parse(&canonical, Path::new("mutation-read-order.spx")).unwrap()),
        "canonical projection must round-trip"
    );
    program
}

fn interpreter_lane(program: &semaprax::ast::Program, entry: &str, expected: &Expected) {
    let path = fixture_path("spx");
    std::fs::write(&path, format::canonical(program)).unwrap();
    let result = interpreter::interpret(&path, entry, &[], &InterpreterOptions::default()).unwrap();
    let _ = std::fs::remove_file(&path);
    let envelope: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
    let outcome = &envelope["payload"]["outcome"];
    match expected {
        Expected::Value(value) => assert!(
            result.envelope.contains(&format!("\"value\":\"{value}\"")),
            "interpreter: {}",
            result.envelope
        ),
        Expected::AdditionOverflow => {
            assert_eq!(outcome["kind"], "failed", "{}", result.envelope);
            assert_eq!(outcome["status"]["domain_id"], "semaprax.arithmetic.v1");
            assert_eq!(outcome["status"]["code"], 1);
        }
    }
}

fn native_lane(program: &semaprax::ast::Program, entry: &str, expected: &Expected) {
    let generated = codegen::emit_c(program).unwrap();
    assert_eq!(generated, codegen::emit_c(program).unwrap());
    if !required_or_available("clang", "SPX_REQUIRE_CLANG") {
        return;
    }
    let symbol = format!(
        "spx_decl_{}",
        entry
            .bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    let check = match expected {
        Expected::Value(value) => format!(
            "if (status != SPX_STATUS_SUCCESS) return 2;\n    if (value != INT64_C({value})) {{ printf(\"%lld\\n\", (long long)value); return 3; }}"
        ),
        Expected::AdditionOverflow => "if (status == SPX_STATUS_SUCCESS) return 4;\n    \
             const struct spx_normalized_status *entry = spx_status_resolve(&context, status);\n    \
             if (entry == NULL || strcmp(entry->domain_id, \"semaprax.arithmetic.v1\") != 0 || entry->code != UINT32_C(1)) return 5;"
            .to_owned(),
    };
    let probe = format!(
        r#"
#include <stdio.h>
#include <string.h>
int main(void) {{
    struct spx_status_entry entries[UINT32_C(32)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(561), entries, UINT32_C(32), NULL, NULL, NULL)) return 1;
    int64_t value = INT64_C(0);
    spx_status_token status = {symbol}(&context, &value);
    {check}
    return 0;
}}
"#
    );
    for optimization in ["-O0", "-O2"] {
        let source = fixture_path("c");
        let executable = fixture_path("native");
        std::fs::write(&source, format!("{generated}\n{probe}")).unwrap();
        let compiled = Command::new("clang")
            .args([
                "-std=c11",
                optimization,
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DSPX_NO_ENTRY_WRAPPER",
            ])
            .arg(&source)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{optimization}: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let executed = Command::new(&executable).output().unwrap();
        let _ = std::fs::remove_file(&source);
        let _ = std::fs::remove_file(&executable);
        assert!(
            executed.status.success(),
            "native {optimization} diverged from the interpreter: {:?} stdout={}",
            executed.status.code(),
            String::from_utf8_lossy(&executed.stdout)
        );
    }
}

fn wasm_lane(program: &semaprax::ast::Program, expected: &Expected) {
    let bytes = wasm::emit_module(program).unwrap();
    assert_eq!(bytes, wasm::emit_module(program).unwrap());
    if !required_or_available("node", "SPX_REQUIRE_NODE") {
        return;
    }
    let root = fixture_path("web");
    wasm::build_web(program, &root).unwrap();
    let output = match expected {
        Expected::Value(value) => {
            let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/verify-web.mjs");
            let output = Command::new("node")
                .arg(script)
                .arg(&root)
                .arg(value.to_string())
                .output()
                .unwrap();
            assert_eq!(
                String::from_utf8_lossy(&output.stdout).trim(),
                value.to_string()
            );
            output
        }
        Expected::AdditionOverflow => {
            std::fs::write(root.join("package.json"), "{\"type\":\"module\"}\n").unwrap();
            std::fs::write(
                root.join("probe.mjs"),
                r#"
import {readFile} from 'node:fs/promises';
import {instantiateBytes,semanticStatus} from './semaprax.js';
const {instance}=await instantiateBytes(await readFile('./app.wasm'));
let failed=false;
try{instance.exports.semaprax_main();}catch(error){
 const status=semanticStatus(error);
 if(status===null || status.domain_id!=='semaprax.arithmetic.v1' || status.code!==1)throw error;
 failed=true;
}
if(!failed)throw Error('missing expected addition overflow');
"#,
            )
            .unwrap();
            Command::new("node")
                .arg("probe.mjs")
                .current_dir(&root)
                .output()
                .unwrap()
        }
    };
    let _ = std::fs::remove_dir_all(&root);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn all_backends(source: &str, entry: &str, expected: Expected) {
    let program = checked(source);
    interpreter_lane(&program, entry, &expected);
    native_lane(&program, entry, &expected);
    wasm_lane(&program, &expected);
}

#[test]
fn earlier_binary_operand_and_first_argument_keep_their_read_values() {
    all_backends(ISSUE_BINARY_AND_CALL, "audit.main", Expected::Value(11));
}

#[test]
fn later_store_of_i64_max_does_not_invent_an_addition_overflow() {
    all_backends(ISSUE_FALSE_OVERFLOW, "audit.main", Expected::Value(2));
}

#[test]
fn earlier_read_of_i64_max_still_selects_the_addition_overflow() {
    all_backends(SELECTED_OVERFLOW, "audit.main", Expected::AdditionOverflow);
}

#[test]
fn every_admitted_copy_scalar_shape_keeps_left_to_right_reads() {
    all_backends(SHAPES, "shape.main", Expected::Value(8191));
}

#[test]
fn whole_copy_record_argument_keeps_its_read_fields() {
    let program = checked(RECORD_ARGUMENT);
    native_lane(&program, "record.main", &Expected::Value(1));
    wasm_lane(&program, &Expected::Value(1));
}
