//! Executable evidence for owned String v1 (RFC-STRING-OO Badge 1+2).
//!
//! Proves canonical round-trip, deterministic graph JSON with string nodes
//! and type facts, HIR resolution of literals, stable rejection diagnostics
//! for unsupported operators, and identical results on the native C11 O0/O2
//! and Node/Wasm backends used by the sibling scalar suites.

use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::hir::OwnershipMode;
use semaprax::{codegen, format, graph, hir, parse, verify, wasm};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const STRINGS: &str = r#"
module test.string_scalars;

@id("s.first")
fn first() -> string { "ada" }

@id("s.same")
fn same(left: string, right: string) -> i64 {
    if left == right { 1 } else { 0 }
}

@id("s.pick")
fn pick(which: i64) -> string {
    if which == 0 { "ada" } else { "bob" }
}

@id("app.main")
fn main() -> i64 {
    let a = "hello";
    let b = "hello";
    let c = "world";
    if a == b {
        if a != c && same(first(), pick(0)) == 1 && same(pick(1), "ada") == 0 {
            7
        } else {
            8
        }
    } else {
        9
    }
}
"#;

fn diagnostics(source: &str) -> Vec<semaprax::diagnostic::Diagnostic> {
    let program = parse(source, Path::new("strings.spx")).unwrap();
    verify::verify(&program)
}

fn command_available(command: &str) -> bool {
    Command::new(command).arg("--version").output().is_ok()
}

#[test]
fn string_programs_round_trip_canonically_and_hash_stably() {
    let program = parse(STRINGS, Path::new("strings.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    let reparsed = parse(&canonical, Path::new("canonical.spx")).unwrap();
    assert!(verify::verify(&reparsed).is_empty());
    assert_eq!(graph::revision(&program), graph::revision(&reparsed));
    assert_eq!(format::canonical(&reparsed), canonical);
}

#[test]
fn escapes_and_unicode_round_trip_exactly() {
    let source = r#"
module test.string_escapes;
@id("t.pick")
fn pick(which: i64) -> i64 {
    if which == 0 { if "a\n\t\r\\\"?" == "a\n\t\r\\\"?" { 1 } else { 0 } } else {
        if "\u{1f600}" != "plain" { 2 } else { 3 }
    }
}
@id("app.main")
fn main() -> i64 { pick(0) + pick(1) }
"#;
    let program = parse(source, Path::new("escapes.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    for fragment in ["\\n", "\\t", "\\r", "\\\\", "\\\"", "\u{1f600}"] {
        assert!(
            canonical.contains(fragment),
            "escape {fragment} must survive the canonical projection: {canonical}"
        );
    }
    let reparsed = parse(&canonical, Path::new("canonical.spx")).unwrap();
    assert_eq!(format::canonical(&reparsed), canonical);
    assert_eq!(graph::revision(&program), graph::revision(&reparsed));
}

#[test]
fn graph_json_exposes_deterministic_string_nodes() {
    let program = parse(STRINGS, Path::new("strings.spx")).unwrap();
    let first = graph::to_json(&program).unwrap();
    let second = graph::to_json(&program).unwrap();
    assert_eq!(first, second);
    // Literal leaves carry their exact contents plus the canonical display.
    assert!(first.contains("\"kind\":\"string\""));
    assert!(first.contains("\"value\":\"hello\",\"display\":\"\\\"hello\\\"\""));
    // The primitive type node and its non-Copy facts are projected.
    assert!(first.contains("\"name\":\"string\""));
    assert!(first.contains("\"id\":\"string\",\"type\":{\"kind\":\"primitive\",\"name\":\"string\"},\"facts\":{\"copy\":false,\"contains_resource\":false,\"sized\":true,\"needs_drop\":true,\"layout_key\":\"owned:string\"}"));
}

#[test]
fn resolved_hir_types_strings_as_owned_values() {
    let program = hir::resolve(&parse(STRINGS, Path::new("strings.spx")).unwrap()).unwrap();
    let first = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "s.first")
        .unwrap();
    assert_eq!(first.return_type, hir::ResolvedType::String);
    let hir::ResolvedExprKind::Block { tail, .. } = &first.body.kind else {
        panic!("function bodies are blocks");
    };
    let hir::ResolvedExprKind::String(value) = &tail.kind else {
        panic!("first body tail must be a string literal");
    };
    assert_eq!(value.as_str(), "ada");
    assert_eq!(tail.ty, hir::ResolvedType::String);
    assert_eq!(tail.ownership, OwnershipMode::Own);

    let same = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "s.same")
        .unwrap();
    assert_eq!(same.params[0].ty, hir::ResolvedType::String);
    assert_eq!(same.params[0].ownership, OwnershipMode::Own);
}

#[test]
fn string_operator_diagnostics_are_stable() {
    let arithmetic = diagnostics(
        r#"
module test.string_arith;
@id("app.main")
fn main() -> i64 { if "a" + "b" == "ab" { 7 } else { 8 } }
"#,
    );
    assert!(
        arithmetic.iter().any(|item| item.code == "SPX-T250"),
        "string concatenation via + must be rejected: {arithmetic:?}"
    );

    let ordering = diagnostics(
        r#"
module test.string_order;
@id("app.main")
fn main() -> i64 { if "a" < "b" { 1 } else { 0 } }
"#,
    );
    assert!(
        ordering
            .iter()
            .any(|item| item.code == "SPX-T250" && item.message.contains('<')),
        "string ordering must be rejected: {ordering:?}"
    );

    let mixed_equality = diagnostics(
        r#"
module test.string_equal;
@id("app.main")
fn main() -> i64 { if "a" == 97 { 7 } else { 8 } }
"#,
    );
    assert!(
        mixed_equality.iter().any(|item| item.code == "SPX-T207"),
        "mixed string-integer equality must be rejected"
    );

    let argument = diagnostics(
        r#"
module test.string_arg;
@id("t.take")
fn take(value: i64) -> i64 { value }
@id("app.main")
fn main() -> i64 { take("a") }
"#,
    );
    assert!(
        argument.iter().any(|item| item.code == "SPX-T205"),
        "implicit string-to-int conversion must be rejected: {argument:?}"
    );
}

#[test]
fn interpreter_evaluates_string_equality_like_the_backends() {
    use semaprax::interpreter::{self, InterpreterOptions};
    let path = Path::new("examples/strings.spx");
    let interpretation =
        interpreter::interpret(path, "test.main", &[], &InterpreterOptions::default()).unwrap();
    let text = interpretation.envelope;
    assert!(
        text.contains("\"kind\":\"returned\"") && text.contains("\"value\":\"1\""),
        "interpreter must agree with the compiled backends: {text}"
    );
}

#[test]
fn native_string_programs_execute_identically_at_o0_o2() {
    if !command_available("clang") {
        return;
    }
    let program = parse(STRINGS, Path::new("strings-native.spx")).unwrap();
    let generated = codegen::emit_c(&program).unwrap();
    assert_eq!(generated, codegen::emit_c(&program).unwrap());
    // Heap-backed values lower through malloc/memcpy construction and free.
    assert!(generated.contains("spx_string_from_literal"));
    assert!(generated.contains("malloc"));
    assert!(generated.contains("memcpy"));
    assert!(generated.contains("free("));

    let symbol = |id: &str| format!("spx_decl_{}", hex_identity(id));
    let probe = format!(
        r#"
int main(void) {{
    struct spx_status_entry entries[UINT32_C(32)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(88), entries, UINT32_C(32), NULL, NULL, NULL)) return 10;
    int64_t tag = INT64_C(0);
    int64_t entry = INT64_C(0);
    if ({entry_fn}(&context, &entry) != SPX_STATUS_SUCCESS || entry != INT64_C(7)) return 11;
    if ({main_fn}(&context, &tag) != SPX_STATUS_SUCCESS || tag != INT64_C(7)) return 12;
    return 0;
}}
"#,
        entry_fn = symbol("app.main"),
        main_fn = symbol("app.main"),
    );
    run_native_probe(&generated, &probe, "string scalars");
}

#[test]
fn native_string_inequality_and_pick_paths_stay_exact() {
    if !command_available("clang") {
        return;
    }
    let program = parse(
        r#"
module test.string_branches;

@id("b.same")
fn same(left: string, right: string) -> i64 {
    if left == right { 1 } else { 0 }
}

@id("b.pick")
fn pick(which: i64) -> string {
    if which == 0 { "ada" } else { "bob" }
}

@id("app.main")
fn main() -> i64 {
    if same(pick(0), "ada") == 1 && same(pick(0), "bob") == 0 && same(pick(1), pick(1)) == 1 {
        7
    } else {
        8
    }
}
"#,
        Path::new("strings-branch.spx"),
    )
    .unwrap();
    let generated = codegen::emit_c(&program).unwrap();
    let main_symbol = format!("spx_decl_{}", hex_identity("app.main"));
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
        main_fn = main_symbol,
    );
    run_native_probe(&generated, &probe, "string branches");
}

fn run_native_probe(generated: &str, probe: &str, label: &str) {
    for optimization in ["-O0", "-O2"] {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!("semaprax-string-native-{label}-{}-{id}", std::process::id());
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

#[test]
fn wasm_string_programs_match_native_results_in_node() {
    if !command_available("node") {
        return;
    }
    let program = parse(STRINGS, Path::new("strings-wasm.spx")).unwrap();
    let bytes = wasm::emit_module(&program).unwrap();
    assert_eq!(bytes, wasm::emit_module(&program).unwrap());
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!("semaprax-string-wasm-{}-{id}", std::process::id());
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
} });
for (let index = 0; index < 4096; index += 1) {
  const observed = instance.exports.semaprax_main();
  if (observed !== expected) throw new Error(`result mismatch ${observed}`);
}
console.log("string-wasm-ok");
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
        "Node string program failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "string-wasm-ok"
    );
}

fn hex_identity(value: &str) -> String {
    use std::fmt::Write as _;
    let mut output = String::with_capacity(value.len() * 2);
    for byte in value.bytes() {
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}
