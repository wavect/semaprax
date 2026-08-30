//! Executable evidence for String operations breadth v2 (`string_starts_with`,
//! `string_contains`, `string_len_chars`, `string_from_char`).
//!
//! Mirrors the sibling v1 suite: canonical round-trip, deterministic graph
//! JSON with intrinsic call nodes carrying their reserved `core.string.*`
//! identities, stable rejection diagnostics, compile-time use-after-move
//! rejection around borrowed v2 operations, interpreter agreement, native C11
//! O0/O2 execution equality over ASCII and multi-byte content, Node/Wasm
//! execution equality, and the group gating that keeps programs without v2
//! operations byte-identical.

use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::{codegen, format, graph, hir, parse, verify, wasm};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

/// Success corpus: borrowed v2 reads, a consuming user call after them,
/// multi-byte content (2-byte `é`, 3-byte snowman, 4-byte emoji), empty
/// strings, whole-value prefixes, and `string_from_char` round-trips.
const STRING_OPS_V2: &str = r#"
module test.string_ops_v2;

@id("ops.has_prefix")
fn has_prefix(value: string, prefix: string) -> bool { string_starts_with(value, prefix) }

@id("ops.holds")
fn holds(value: string) -> i64 {
    if string_contains(value, "\u{2603}") { string_len_chars(string_concat(value, "!")) } else { 0 }
}

@id("app.main")
fn main() -> i64 {
    let m = string_concat("h", "éllo");
    let reads = string_starts_with(m, "hé") && string_contains(m, "ll") && string_starts_with(m, m) && string_contains(m, "");
    if reads == false {
        1
    } else {
        if string_len_chars(m) == 5 && string_len(m) == 6 && has_prefix(m, "zzz") == false {
            let snow = string_from_char('\u{2603}');
            let snow_ok = string_len_chars(snow) == 1 && string_len(snow) == 3;
            if holds(snow) == 2 && snow_ok {
                let wave = string_from_char('\u{1F600}');
                let acute = string_concat(string_from_char('é'), "!");
                if string_contains(string_from_char('a'), "a") && string_contains(wave, "\u{1F600}") && string_len_chars(acute) == 2 && string_starts_with("", "") && string_len_chars("") == 0 && string_starts_with("", "h") == false {
                    7
                } else {
                    9
                }
            } else {
                11
            }
        } else {
            13
        }
    }
}
"#;

const V1_ONLY: &str = r#"
module test.string_ops_v1_only;

@id("app.main")
fn main() -> i64 {
    let m = string_concat("hello", "world");
    if string_is_empty(m) { 0 } else { if string_len(m) == 10 { 7 } else { 8 } }
}
"#;

const PLAIN_STRINGS: &str = r#"
module test.plain_strings_v2;

@id("app.main")
fn main() -> i64 {
    let a = "hello";
    let b = "hello";
    if a == b { 7 } else { 8 }
}
"#;

const V2_HELPER_NAMES: [&str; 4] = [
    "spx_string_starts_with",
    "spx_string_contains",
    "spx_string_len_chars",
    "spx_string_from_char",
];

fn diagnostics(source: &str) -> Vec<semaprax::diagnostic::Diagnostic> {
    let program = parse(source, Path::new("string_ops_v2.spx")).unwrap();
    verify::verify(&program)
}

fn command_available(command: &str) -> bool {
    Command::new(command).arg("--version").output().is_ok()
}

#[test]
fn string_ops_v2_programs_round_trip_canonically_and_hash_stably() {
    let program = parse(STRING_OPS_V2, Path::new("string_ops_v2.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    assert!(canonical.contains("string_starts_with(m, \"hé\")"));
    assert!(canonical.contains("string_contains(m, \"ll\")"));
    assert!(canonical.contains("string_len_chars(m)"));
    assert!(canonical.contains("string_from_char('\\u{2603}')"));
    let reparsed = parse(&canonical, Path::new("canonical.spx")).unwrap();
    assert!(verify::verify(&reparsed).is_empty());
    assert_eq!(graph::revision(&program), graph::revision(&reparsed));
    assert_eq!(format::canonical(&reparsed), canonical);
}

#[test]
fn graph_json_exposes_deterministic_breadth_v2_operation_nodes() {
    let program = parse(STRING_OPS_V2, Path::new("string_ops_v2.spx")).unwrap();
    let first = graph::to_json(&program).unwrap();
    let second = graph::to_json(&program).unwrap();
    assert_eq!(first, second);
    // Breadth-v2 intrinsic calls project as ordinary monomorphic call nodes
    // bound to their reserved compiler-owned identities.
    assert!(first.contains("\"kind\":\"call\",\"callee\":\"core.string.starts_with\""));
    assert!(first.contains("\"kind\":\"call\",\"callee\":\"core.string.contains\""));
    assert!(first.contains("\"kind\":\"call\",\"callee\":\"core.string.len_chars\""));
    assert!(first.contains("\"kind\":\"call\",\"callee\":\"core.string.from_char\""));

    // Programs without breadth-v2 operations keep projections free of the new
    // identities while first-wave nodes stay exactly as pinned by the v1
    // suite.
    let v1_only = parse(V1_ONLY, Path::new("v1_only.spx")).unwrap();
    let v1_only_json = graph::to_json(&v1_only).unwrap();
    assert_eq!(
        v1_only_json,
        graph::to_json(&parse(V1_ONLY, Path::new("v1_only.spx")).unwrap()).unwrap()
    );
    assert_eq!(
        graph::revision(&v1_only),
        graph::revision(&parse(V1_ONLY, Path::new("v1_only.spx")).unwrap())
    );
    assert!(v1_only_json.contains("\"callee\":\"core.string.concat\""));
    assert!(!v1_only_json.contains("core.string.starts_with"));
    assert!(!v1_only_json.contains("core.string.contains\""));
    assert!(!v1_only_json.contains("core.string.len_chars"));
    assert!(!v1_only_json.contains("core.string.from_char"));

    let plain = parse(PLAIN_STRINGS, Path::new("plain_strings.spx")).unwrap();
    let plain_json = graph::to_json(&plain).unwrap();
    assert!(!plain_json.contains("core.string."));
}

#[test]
fn resolved_hir_binds_breadth_v2_operations_to_reserved_identities() {
    let program =
        hir::resolve(&parse(STRING_OPS_V2, Path::new("string_ops_v2.spx")).unwrap()).unwrap();
    let mut starts_with_calls = 0;
    let mut contains_calls = 0;
    let mut len_chars_calls = 0;
    let mut from_char_calls = 0;
    let mut pending: Vec<&hir::ResolvedExpr> = Vec::new();
    for function in &program.functions {
        pending.push(&function.body);
    }
    while let Some(expression) = pending.pop() {
        if let hir::ResolvedExprKind::Call { callee, .. } = &expression.kind {
            match callee.as_str() {
                "core.string.starts_with" => {
                    starts_with_calls += 1;
                    assert_eq!(expression.ty, hir::ResolvedType::Bool);
                    assert_eq!(expression.ownership, hir::OwnershipMode::Value);
                }
                "core.string.contains" => {
                    contains_calls += 1;
                    assert_eq!(expression.ty, hir::ResolvedType::Bool);
                    assert_eq!(expression.ownership, hir::OwnershipMode::Value);
                }
                "core.string.len_chars" => {
                    len_chars_calls += 1;
                    assert_eq!(expression.ty, hir::ResolvedType::I64);
                    assert_eq!(expression.ownership, hir::OwnershipMode::Value);
                }
                "core.string.from_char" => {
                    from_char_calls += 1;
                    assert_eq!(args_of(expression).len(), 1);
                    assert_eq!(expression.ty, hir::ResolvedType::String);
                    assert_eq!(expression.ownership, hir::OwnershipMode::Own);
                }
                _ => {}
            }
        }
        pending.extend(hir_children(expression));
    }
    assert_eq!(starts_with_calls, 5);
    assert_eq!(contains_calls, 5);
    assert_eq!(len_chars_calls, 5);
    assert_eq!(from_char_calls, 4);
}

fn args_of(expression: &hir::ResolvedExpr) -> &[hir::ResolvedExpr] {
    match &expression.kind {
        hir::ResolvedExprKind::Call { args, .. } => args,
        _ => &[],
    }
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
        hir::ResolvedExprKind::Call { args, .. } => args.iter().collect(),
        _ => Vec::new(),
    }
}

#[test]
fn string_operations_v2_diagnostics_are_stable() {
    // Type errors reuse the ordinary argument mismatch family with the
    // synthetic parameter names and kinds.
    let string_argument = diagnostics(
        r#"
module test.v2_types_a;
@id("app.main")
fn main() -> i64 {
    if string_contains(42, "x") { 7 } else { 8 }
}
"#,
    );
    assert!(
        string_argument.iter().any(|item| item.code == "SPX-T205"
            && item
                .message
                .contains("argument `s` to `string_contains` expects string")),
        "passing an i64 to string_contains must be rejected: {string_argument:?}"
    );

    let char_argument = diagnostics(
        r#"
module test.v2_types_b;
@id("app.main")
fn main() -> i64 {
    let s = string_from_char("x");
    if string_len_chars(s) == 1 { 7 } else { 8 }
}
"#,
    );
    assert!(
        char_argument.iter().any(|item| item.code == "SPX-T205"
            && item
                .message
                .contains("argument `c` to `string_from_char` expects char")),
        "passing a string to string_from_char must be rejected: {char_argument:?}"
    );

    // Arity mismatches reuse the ordinary arity family for every new name.
    let arity_zero = diagnostics(
        r#"
module test.v2_arity_a;
@id("app.main")
fn main() -> i64 {
    if string_len_chars() == 0 { 7 } else { 8 }
}
"#,
    );
    assert!(
        arity_zero.iter().any(|item| item.code == "SPX-T204"
            && item
                .message
                .contains("`string_len_chars` expects 1 arguments, received 0")),
        "arity mismatches must be rejected: {arity_zero:?}"
    );

    let arity_two = diagnostics(
        r#"
module test.v2_arity_b;
@id("app.main")
fn main() -> i64 {
    if string_starts_with("a") { 7 } else { 8 }
}
"#,
    );
    assert!(
        arity_two.iter().any(|item| item.code == "SPX-T204"
            && item
                .message
                .contains("`string_starts_with` expects 2 arguments, received 1")),
        "arity mismatches must be rejected: {arity_two:?}"
    );

    // The breadth-v2 names are compiler-reserved like the first wave.
    for reserved_source in [
        r#"
module test.v2_reserved_a;
@id("t.shadow")
fn string_contains(value: string, needle: string) -> bool { true }
@id("app.main")
fn main() -> i64 { 7 }
"#,
        r#"
module test.v2_reserved_b;
@id("t.shadow")
fn string_from_char(scalar: char) -> string { "x" }
@id("app.main")
fn main() -> i64 { 7 }
"#,
    ] {
        let reserved = diagnostics(reserved_source);
        assert!(
            reserved.iter().any(|item| item.code == "SPX-S113"
                && item
                    .message
                    .contains("reserved by the compiler-owned string operations")),
            "shadowing a breadth-v2 operation name must be rejected: {reserved:?}"
        );
    }

    // Consuming transfers still reject later uses at compile time when the
    // later use is a borrowed breadth-v2 read; borrows themselves never move.
    let moved_source = r#"
module test.v2_moves;
@id("app.main")
fn main() -> i64 {
    let a = string_concat("hello", "world");
    let b = string_concat(a, "!");
    if string_starts_with(a, "he") { 7 } else { if string_len_chars(b) == 12 { 9 } else { 8 } }
}
"#;
    let moved = hir::resolve(&parse(moved_source, Path::new("moves_v2.spx")).unwrap()).unwrap_err();
    assert!(
        moved
            .iter()
            .any(|item| item.code == "SPX-O101"
                && item.message.contains("after ownership was moved")),
        "using a consumed operand behind a borrowed read must be rejected: {moved:?}"
    );

    // Borrowed breadth-v2 reads leave their operands available.
    let borrows_source = r#"
module test.v2_borrows;
@id("app.main")
fn main() -> i64 {
    let m = string_concat("héllo", "!");
    if string_starts_with(m, "h") && string_contains(m, "l") && string_len_chars(m) == 6 {
        7
    } else {
        8
    }
}
"#;
    let borrows = hir::resolve(&parse(borrows_source, Path::new("borrows_v2.spx")).unwrap())
        .expect("borrowed breadth-v2 reads keep their operand available");
    assert!(!borrows.functions.is_empty());
}

#[test]
fn interpreter_evaluates_breadth_v2_operations_like_the_backends() {
    use semaprax::interpreter::{self, InterpreterOptions};
    // The scalar interpreter profile admits only i64/bool signatures, so this
    // program reaches the operations through literals and locals only; the
    // compiled-backend suites cover user functions taking strings.
    const INTERPRETED: &str = r#"
module test.string_ops_v2_interp;
@id("test.main")
fn main() -> i64 {
    let m = string_concat("h", "éllo");
    if string_starts_with(m, "hé") {
        if string_contains(m, "ll") {
            if string_len_chars(m) == 5 && string_len(m) == 6 {
                let snow = string_from_char('\u{2603}');
                if string_len_chars(snow) == 1 && string_len(snow) == 3 && string_contains(snow, "\u{2603}") {
                    7
                } else {
                    8
                }
            } else {
                9
            }
        } else {
            10
        }
    } else {
        11
    }
}
"#;
    let path = std::env::temp_dir().join(format!(
        "semaprax-string-ops-v2-interp-{}.spx",
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
fn native_breadth_v2_operations_execute_identically_at_o0_o2() {
    if !command_available("clang") {
        return;
    }
    let program = parse(STRING_OPS_V2, Path::new("string-ops-v2-native.spx")).unwrap();
    let generated = codegen::emit_c(&program).unwrap();
    assert_eq!(generated, codegen::emit_c(&program).unwrap());
    // The dedicated v2 helpers are emitted only when a v2 operation is reached.
    for name in V2_HELPER_NAMES {
        assert!(generated.contains(name), "missing helper {name}");
    }

    // Programs without breadth-v2 operations keep their exact bytes: the
    // first-wave helpers appear alone and no v2 symbol is reachable.
    let v1_only = parse(V1_ONLY, Path::new("v1-only-native.spx")).unwrap();
    let v1_only_c = codegen::emit_c(&v1_only).unwrap();
    assert!(v1_only_c.contains("spx_string_len("));
    assert!(v1_only_c.contains("spx_string_concat"));
    assert!(v1_only_c.contains("spx_string_is_empty"));
    for name in V2_HELPER_NAMES {
        assert!(
            !v1_only_c.contains(name),
            "first-wave programs must not emit {name}"
        );
    }
    let plain = parse(PLAIN_STRINGS, Path::new("plain-v2-native.spx")).unwrap();
    let plain_c = codegen::emit_c(&plain).unwrap();
    for name in ["spx_string_len", "spx_string_concat", "spx_string_is_empty"]
        .into_iter()
        .chain(V2_HELPER_NAMES)
    {
        assert!(
            !plain_c.contains(&format!("{name}(")),
            "plain programs must not emit operation helper {name}"
        );
    }
    assert_eq!(plain_c, codegen::emit_c(&plain).unwrap());

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
    run_native_probe(&generated, &probe, "string-ops-v2");
}

#[test]
fn wasm_breadth_v2_operations_match_native_results_in_node() {
    if !command_available("node") {
        return;
    }
    let program = parse(STRING_OPS_V2, Path::new("string-ops-v2-wasm.spx")).unwrap();
    let bytes = wasm::emit_module(&program).unwrap();
    assert_eq!(bytes, wasm::emit_module(&program).unwrap());

    // Operation host imports never join modules that cannot reach them, and
    // first-wave modules gain none of the breadth-v2 imports.
    let v1_only = parse(V1_ONLY, Path::new("v1-only-wasm.spx")).unwrap();
    let v1_only_bytes = wasm::emit_module(&v1_only).unwrap();
    let v1_only_text = wasm_bytes_text(&v1_only_bytes);
    assert!(v1_only_text.contains("spx_string_len"));
    assert!(v1_only_text.contains("spx_string_concat"));
    for name in [
        "spx_string_starts_with",
        "spx_string_contains",
        "spx_string_len_chars",
        "spx_string_from_char",
    ] {
        assert!(
            !v1_only_text.contains(name),
            "first-wave modules must not import {name}"
        );
    }
    let plain = parse(PLAIN_STRINGS, Path::new("plain-v2-wasm.spx")).unwrap();
    let plain_text = wasm_bytes_text(&wasm::emit_module(&plain).unwrap());
    for name in [
        "spx_string_len",
        "spx_string_concat",
        "spx_string_starts_with",
        "spx_string_contains",
        "spx_string_len_chars",
        "spx_string_from_char",
    ] {
        assert!(
            !plain_text.contains(name),
            "plain modules must not import {name}"
        );
    }

    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!("semaprax-string-ops-v2-wasm-{}-{id}", std::process::id());
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
const handleOf = (text) => {
  const handle = nextHandle++;
  strings.set(handle, text);
  return BigInt(handle);
};
const { instance } = await WebAssembly.instantiate(bytes, { env: {
  spx_add: fail("spx_add"), spx_sub: fail("spx_sub"), spx_mul: fail("spx_mul"),
  spx_div: fail("spx_div"), spx_rem: fail("spx_rem"), spx_neg: fail("spx_neg"),
  spx_contract_fail: fail("spx_contract_fail"),
  spx_string_new: (ptr, len) => {
    const view = new Uint8Array(instance.exports.memory.buffer, Number(ptr), Number(len));
    return handleOf(new TextDecoder().decode(view));
  },
  spx_string_eq: (left, right) =>
    strings.get(Number(left)) === strings.get(Number(right)) ? 1 : 0,
  spx_string_clone: (handle) => handleOf(strings.get(Number(handle))),
  spx_string_len: (handle) => BigInt(materialize(handle).length),
  spx_string_concat: (left, right) =>
    handleOf(strings.get(Number(left)) + strings.get(Number(right))),
  spx_string_starts_with: (handle, prefix) =>
    strings.get(Number(handle)).startsWith(strings.get(Number(prefix))) ? 1 : 0,
  spx_string_contains: (handle, needle) =>
    strings.get(Number(handle)).includes(strings.get(Number(needle))) ? 1 : 0,
  spx_string_len_chars: (handle) => BigInt([...strings.get(Number(handle))].length),
  spx_string_from_char: (scalar) => handleOf(String.fromCodePoint(Number(scalar))),
} });
for (let index = 0; index < 4096; index += 1) {
  const observed = instance.exports.semaprax_main();
  if (observed !== expected) throw new Error(`result mismatch ${observed}`);
}
console.log("string-ops-v2-wasm-ok");
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
        "Node string ops v2 program failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "string-ops-v2-wasm-ok"
    );
}

fn wasm_bytes_text(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| *byte as char).collect()
}

fn run_native_probe(generated: &str, probe: &str, label: &str) {
    for optimization in ["-O0", "-O2"] {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!(
            "semaprax-string-ops-v2-native-{label}-{}-{id}",
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
