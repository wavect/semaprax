//! Executable evidence for String operations v1 (`string_len`, `string_concat`,
//! `string_is_empty`).
//!
//! Proves canonical round-trip, deterministic graph JSON with intrinsic call
//! nodes carrying their reserved `core.string.*` identities, stable rejection
//! diagnostics, compile-time use-after-move rejection for consumed concat
//! arguments, and identical results on the native C11 O0/O2 and Node/Wasm
//! backends used by the sibling scalar suites.

use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::{codegen, format, graph, hir, parse, verify, wasm};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const STRING_OPS: &str = r#"
module test.string_ops_v1;

@id("ops.combine")
fn combine(left: string, right: string) -> string { string_concat(left, right) }

@id("app.main")
fn main() -> i64 {
    let greeting = string_concat("hello", " ");
    let message = string_concat(greeting, combine("w", "orld"));
    if string_is_empty(message) {
        0
    } else {
        if string_len(message) == 11 && message == "hello world" { 7 } else { 8 }
    }
}
"#;

fn diagnostics(source: &str) -> Vec<semaprax::diagnostic::Diagnostic> {
    let program = parse(source, Path::new("string_ops.spx")).unwrap();
    verify::verify(&program)
}

fn command_available(command: &str) -> bool {
    Command::new(command).arg("--version").output().is_ok()
}

#[test]
fn string_ops_programs_round_trip_canonically_and_hash_stably() {
    let program = parse(STRING_OPS, Path::new("string_ops.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    assert!(canonical.contains("string_len(message)"));
    assert!(canonical.contains("string_concat(\"hello\", \" \")"));
    assert!(canonical.contains("string_is_empty(message)"));
    let reparsed = parse(&canonical, Path::new("canonical.spx")).unwrap();
    assert!(verify::verify(&reparsed).is_empty());
    assert_eq!(graph::revision(&program), graph::revision(&reparsed));
    assert_eq!(format::canonical(&reparsed), canonical);
}

#[test]
fn graph_json_exposes_deterministic_string_operation_nodes() {
    let program = parse(STRING_OPS, Path::new("string_ops.spx")).unwrap();
    let first = graph::to_json(&program).unwrap();
    let second = graph::to_json(&program).unwrap();
    assert_eq!(first, second);
    // Intrinsic calls project as ordinary monomorphic call nodes bound to
    // their reserved compiler-owned identities.
    assert!(first.contains("\"kind\":\"call\",\"callee\":\"core.string.concat\""));
    assert!(first.contains("\"kind\":\"call\",\"callee\":\"core.string.len\""));
    assert!(first.contains("\"kind\":\"call\",\"callee\":\"core.string.is_empty\""));
    // Concatenation of two literals carries both exact operand leaves.
    assert!(
        first.contains(
            "\"callee\":\"core.string.concat\",\"args\":[{\"id\":\
             \"declaration:8:app.main:expression:19:body.s0.value.arg.0\""
        ),
        "concat operand layout must stay pinned: {first}"
    );
}

#[test]
fn resolved_hir_binds_string_operations_to_reserved_identities() {
    let program = hir::resolve(&parse(STRING_OPS, Path::new("string_ops.spx")).unwrap()).unwrap();
    let mut len_calls = 0;
    let mut concat_calls = 0;
    let mut is_empty_calls = 0;
    let mut pending: Vec<&hir::ResolvedExpr> = Vec::new();
    for function in &program.functions {
        pending.push(&function.body);
    }
    while let Some(expression) = pending.pop() {
        if let hir::ResolvedExprKind::Call { callee, args, .. } = &expression.kind {
            match callee.as_str() {
                "core.string.len" => {
                    len_calls += 1;
                    assert_eq!(expression.ty, hir::ResolvedType::I64);
                }
                "core.string.concat" => {
                    concat_calls += 1;
                    assert_eq!(args.len(), 2);
                    assert_eq!(expression.ty, hir::ResolvedType::String);
                    assert_eq!(expression.ownership, hir::OwnershipMode::Own);
                }
                "core.string.is_empty" => {
                    is_empty_calls += 1;
                    assert_eq!(expression.ty, hir::ResolvedType::Bool);
                }
                _ => {}
            }
            pending.extend(args.iter());
        } else {
            pending.extend(hir_children(expression));
        }
    }
    assert_eq!(len_calls, 1);
    assert_eq!(concat_calls, 3);
    assert_eq!(is_empty_calls, 1);
}

fn hir_children(expression: &hir::ResolvedExpr) -> Vec<&hir::ResolvedExpr> {
    match &expression.kind {
        hir::ResolvedExprKind::Unary { value, .. } => vec![value.as_ref()],
        hir::ResolvedExprKind::Binary { left, right, .. } => vec![left.as_ref(), right.as_ref()],
        hir::ResolvedExprKind::Block { statements, tail } => statements
            .iter()
            .map(|statement| statement.value())
            .chain(std::iter::once(tail.as_ref()))
            .collect(),
        hir::ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => vec![
            condition.as_ref(),
            then_branch.as_ref(),
            else_branch.as_ref(),
        ],
        _ => Vec::new(),
    }
}

#[test]
fn string_operation_diagnostics_are_stable() {
    // Type errors reuse the ordinary argument mismatch family.
    let argument_type = diagnostics(
        r#"
module test.string_op_types;
@id("app.main")
fn main() -> i64 {
    let n = string_len(42);
    if n == 0 { 7 } else { 8 }
}
"#,
    );
    assert!(
        argument_type.iter().any(|item| item.code == "SPX-T205"
            && item
                .message
                .contains("argument `s` to `string_len` expects string")),
        "passing an i64 to string_len must be rejected: {argument_type:?}"
    );

    let boolean_argument = diagnostics(
        r#"
module test.string_op_bools;
@id("app.main")
fn main() -> i64 {
    if string_concat(true, false) { 7 } else { 8 }
}
"#,
    );
    assert!(
        boolean_argument.iter().any(|item| item.code == "SPX-T205"
            && item
                .message
                .contains("argument `a` to `string_concat` expects string")),
        "passing booleans to string_concat must be rejected: {boolean_argument:?}"
    );

    // Arity mismatches reuse the ordinary arity family.
    let arity = diagnostics(
        r#"
module test.string_op_arity;
@id("app.main")
fn main() -> i64 {
    let n = string_len();
    if n == 0 { 7 } else { 8 }
}
"#,
    );
    assert!(
        arity.iter().any(|item| item.code == "SPX-T204"
            && item
                .message
                .contains("`string_len` expects 1 arguments, received 0")),
        "arity mismatches must be rejected: {arity:?}"
    );

    // The operation names are compiler-reserved.
    let reserved = diagnostics(
        r#"
module test.string_op_reserved;
@id("t.shadow")
fn string_len(value: string) -> i64 { 0 }
@id("app.main")
fn main() -> i64 { 7 }
"#,
    );
    assert!(
        reserved.iter().any(|item| item.code == "SPX-S113"
            && item
                .message
                .contains("reserved by the compiler-owned string operations")),
        "shadowing a string operation name must be rejected: {reserved:?}"
    );

    // Consuming concatenation rejects later uses of its arguments at compile
    // time exactly like every other owned string transfer.
    let moved_source = r#"
module test.string_op_moves;
@id("app.main")
fn main() -> i64 {
    let a = string_concat("hello", "world");
    let b = string_concat(a, "!");
    if string_len(a) == 11 { 7 } else { 8 }
}
"#;
    let moved = hir::resolve(&parse(moved_source, Path::new("moves.spx")).unwrap()).unwrap_err();
    assert!(
        moved.iter().any(|item| item.code == "SPX-H006"
            && item.message.contains("used after it was moved")),
        "using a consumed concat argument must be rejected: {moved:?}"
    );
}

#[test]
fn borrowed_reads_do_not_move_their_operands() {
    let borrows_source = r#"
module test.string_op_borrows;
@id("app.main")
fn main() -> i64 {
    let m = string_concat("hello", "world");
    if string_is_empty(m) {
        0
    } else {
        if string_len(m) == 10 && m == "helloworld" { 7 } else { 8 }
    }
}
"#;
    let program = hir::resolve(&parse(borrows_source, Path::new("borrows.spx")).unwrap())
        .expect("borrowed reads keep their operand available");
    assert!(!program.functions.is_empty());
}

#[test]
fn interpreter_evaluates_string_operations_like_the_backends() {
    use semaprax::interpreter::{self, InterpreterOptions};
    // The scalar interpreter profile admits only i64/bool signatures, so this
    // program reaches the operations through literals and locals only; the
    // compiled-backend suites cover user functions taking strings.
    const INTERPRETED: &str = r#"
module test.string_ops_interp;
@id("test.main")
fn main() -> i64 {
    let m = string_concat("hello", "world");
    if string_is_empty(m) { 0 } else { if string_len(m) == 10 && m == "helloworld" { 7 } else { 8 } }
}
"#;
    let path = std::env::temp_dir().join(format!(
        "semaprax-string-ops-interp-{}.spx",
        std::process::id()
    ));
    std::fs::write(
        &path,
        format::canonical(&parse(INTERPRETED, &path).unwrap()),
    )
    .unwrap();
    let interpretation =
        interpreter::interpret(&path, "test.main", &[], &InterpreterOptions::default()).unwrap();
    let _ = std::fs::remove_file(&path);
    let text = interpretation.envelope;
    assert!(
        text.contains("\"kind\":\"returned\"") && text.contains("\"value\":\"7\""),
        "interpreter must agree with the compiled backends: {text}"
    );
}

#[test]
fn native_string_operations_execute_identically_at_o0_o2() {
    if !command_available("clang") {
        return;
    }
    let program = parse(STRING_OPS, Path::new("string-ops-native.spx")).unwrap();
    let generated = codegen::emit_c(&program).unwrap();
    assert_eq!(generated, codegen::emit_c(&program).unwrap());
    // The dedicated helpers are emitted only when the operations are reached.
    assert!(generated.contains("spx_string_from_literal"));
    assert!(generated.contains("spx_string_len"));
    assert!(generated.contains("spx_string_concat"));
    assert!(generated.contains("spx_string_is_empty"));

    let plain = parse(PLAIN_STRINGS, Path::new("plain-strings.spx")).unwrap();
    let plain_c = codegen::emit_c(&plain).unwrap();
    assert!(plain_c.contains("spx_string_from_literal"));
    assert!(!plain_c.contains("spx_string_len"));
    assert!(!plain_c.contains("spx_string_concat"));

    let symbol = |id: &str| format!("spx_decl_{}", hex_identity(id));
    let probe = format!(
        r#"
int main(void) {{
    struct spx_status_entry entries[UINT32_C(32)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(88), entries, UINT32_C(32), NULL, NULL, NULL)) return 10;
    int64_t result = INT64_C(0);
    if ({main_fn}(&context, &result) != SPX_STATUS_SUCCESS || result != INT64_C(7)) return 11;
    return 0;
}}
"#,
        main_fn = symbol("app.main"),
    );
    run_native_probe(&generated, &probe, "string ops");
}

const PLAIN_STRINGS: &str = r#"
module test.plain_strings;

@id("app.main")
fn main() -> i64 {
    let a = "hello";
    let b = "hello";
    if a == b { 7 } else { 8 }
}
"#;

#[test]
fn wasm_string_operations_match_native_results_in_node() {
    if !command_available("node") {
        return;
    }
    let program = parse(STRING_OPS, Path::new("string-ops-wasm.spx")).unwrap();
    let bytes = wasm::emit_module(&program).unwrap();
    assert_eq!(bytes, wasm::emit_module(&program).unwrap());

    let plain = parse(PLAIN_STRINGS, Path::new("plain-strings.spx")).unwrap();
    let plain_bytes = wasm::emit_module(&plain).unwrap();
    // Operation host imports never join modules that cannot reach them.
    let plain_text = plain_bytes
        .iter()
        .map(|byte| *byte as char)
        .collect::<String>();
    assert!(!plain_text.contains("spx_string_len"));
    assert!(!plain_text.contains("spx_string_concat"));

    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!("semaprax-string-ops-wasm-{}-{id}", std::process::id());
    let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
    let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
    std::fs::write(&wasm_path, bytes).unwrap();
    std::fs::write(
        &script_path,
        r#"import { readFile } from "node:fs/promises";
const fail = (name) => () => { throw new Error(`unexpected host import ${name}`); };
const bytes = await readFile(process.argv[2]);
const expected = BigInt(process.argv[3]);
const strings = new Map();
let nextHandle = 1;
const encoder = new TextEncoder();
const materialize = (handle) => encoder.encode(strings.get(Number(handle)));
const { instance } = await WebAssembly.instantiate(bytes, { env: {
  spx_add: fail("spx_add"), spx_sub: fail("spx_sub"), spx_mul: fail("spx_mul"),
  spx_div: fail("spx_div"), spx_rem: fail("spx_rem"), spx_neg: fail("spx_neg"),
  spx_contract_fail: fail("spx_contract_fail"),
  spx_string_new: (ptr, len) => {
    const view = new Uint8Array(instance.exports.memory.buffer, Number(ptr), Number(len));
    const handle = nextHandle++;
    strings.set(handle, new TextDecoder().decode(view));
    return BigInt(handle);
  },
  spx_string_eq: (left, right) =>
    strings.get(Number(left)) === strings.get(Number(right)) ? 1 : 0,
  spx_string_clone: (handle) => {
    const cloned = nextHandle++;
    strings.set(cloned, strings.get(Number(handle)));
    return BigInt(cloned);
  },
  spx_string_len: (handle) => BigInt(materialize(handle).length),
  spx_string_concat: (left, right) => {
    const joined = strings.get(Number(left)) + strings.get(Number(right));
    const handle = nextHandle++;
    strings.set(handle, joined);
    return BigInt(handle);
  },
} });
for (let index = 0; index < 4096; index += 1) {
  const observed = instance.exports.semaprax_main();
  if (observed !== expected) throw new Error(`result mismatch ${observed}`);
}
console.log("string-ops-wasm-ok");
"#,
    )
    .unwrap();
    let output = Command::new("node")
        .arg(&script_path)
        .arg(&wasm_path)
        .arg("7")
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&script_path);
    let _ = std::fs::remove_file(&wasm_path);
    assert!(
        output.status.success(),
        "Node string ops program failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "string-ops-wasm-ok"
    );
}

fn run_native_probe(generated: &str, probe: &str, label: &str) {
    for optimization in ["-O0", "-O2"] {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!(
            "semaprax-string-ops-native-{label}-{}-{id}",
            std::process::id()
        );
        let source = std::env::temp_dir().join(format!("{stem}.c"));
        let executable =
            std::env::temp_dir().join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
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
            "{label} C failed at {optimization}: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let executed = Command::new(&executable).output().unwrap();
        let status = executed.status.code();
        let stderr = String::from_utf8_lossy(&executed.stderr).into_owned();
        let _ = std::fs::remove_file(&source);
        let _ = std::fs::remove_file(&executable);
        assert!(
            executed.status.success(),
            "{label} failed at {optimization}: status={status:?} stderr={stderr}"
        );
    }
}

fn hex_identity(value: &str) -> String {
    use std::fmt::Write as _;
    let mut output = String::with_capacity(value.len() * 2);
    for byte in value.bytes() {
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}
