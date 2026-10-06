//! SPX-AI-021 / issue #120: the bounded owning-capture closure profile
//! (`own fn() -> R { target(payload) }`) now lowers through
//! `hir::closure::desugar_owning_closures` on all three claiming backends --
//! `tests/language/function_values/owning_closures.rs`'s `backend_execution`
//! module proves the interpreter executes it end to end and that native C11
//! (`codegen::emit_c`) and Core Wasm (`wasm::emit_module`) both emit a
//! translation unit / module for it instead of refusing. Emitting is not
//! executing: this module is the missing piece the repository's first
//! invariant actually requires -- "a safe source program has equivalent
//! checked behavior on every backend that claims to implement the admitted
//! feature" -- so it compiles the native output with `clang` and runs it,
//! instantiates the Wasm module with `node`, and compares the OBSERVABLE
//! RESULT against the interpreter and against each other, for both required
//! shapes:
//!
//! - the CALLED shape: the captured owner is transferred into the target
//!   call and never finalized directly (the call's own unused-parameter drop
//!   settles it);
//! - the UNCALLED shape: the closure is constructed and dropped without
//!   ever being called, so its captured owner must settle through the
//!   ordinary scope-exit drop for an unmoved owned local -- the shape most
//!   likely to diverge if the substitution in `owning_desugar.rs` dropped a
//!   cleanup obligation, per `hir::closure::owning_desugar`'s own module doc.
//!
//! Every case's native run also counts allocations/frees through a
//! malloc/calloc/free-intercepting probe (the same technique
//! `tests/native_owned_utf8_settlement_v1/allocations.c` established for
//! owned `String`/`Bytes` settlement elsewhere in this suite, reproduced
//! locally here because `spx_bytes_zeroed` allocates via `calloc`, which
//! that shared fixture does not intercept) and the Core-Wasm run counts
//! `spx_bytes_zeroed`/`spx_bytes_drop` host-import calls the same way, so a
//! leaked or double-freed capture would fail the count assertion even if the
//! returned scalar happened to still match.

use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::{codegen, parse, verify, wasm};

#[path = "affine_capture.rs"]
mod affine_capture;

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

/// `checksum` ignores its payload's content -- this bounded profile's body
/// grammar admits only a bare transferring call, not byte inspection -- but
/// it is a genuine one-`own Bytes`-parameter function that legitimately
/// consumes and drops its argument, which is all the profile requires of a
/// target. Matches `tests/language/function_values/owning_closures.rs`'s own
/// `TARGET` fixture (duplicated here rather than shared across binaries,
/// matching this repository's existing test convention of each harness
/// module owning its own fixture text).
const CALLED_SOURCE: &str = r#"
module test.owning_closure_execution_called;

@id("owning.checksum") fn checksum(payload: own Bytes) -> i64 {
    42
}

@id("app.main") fn main() -> i64 {
    let payload = bytes_zeroed(4usize);
    let clo = own fn() -> i64 { checksum(payload) };
    clo()
}
"#;

const UNCALLED_SOURCE: &str = r#"
module test.owning_closure_execution_uncalled;

@id("owning.checksum") fn checksum(payload: own Bytes) -> i64 {
    42
}

@id("app.main") fn main() -> i64 {
    let payload = bytes_zeroed(4usize);
    let clo = own fn() -> i64 { checksum(payload) };
    99
}
"#;

const CALLED_EXPECTED: i64 = 42;
const UNCALLED_EXPECTED: i64 = 99;

fn command_available(command: &str) -> bool {
    Command::new(command).arg("--version").output().is_ok()
}

fn hex_identity(value: &str) -> String {
    use std::fmt::Write as _;
    let mut output = String::with_capacity(value.len() * 2);
    for byte in value.bytes() {
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn checked_program(source: &str, label: &str) -> semaprax::ast::Program {
    let program = parse(source, Path::new(&format!("{label}.spx"))).unwrap();
    let diagnostics = verify::verify(&program);
    assert!(
        diagnostics.iter().all(|d| !d.severity.is_error()),
        "{label} unexpectedly failed source verification: {diagnostics:?}"
    );
    program
}

fn extract_returned_value(envelope: &str) -> i64 {
    let marker = "\"value\":\"";
    let start = envelope
        .find(marker)
        .unwrap_or_else(|| panic!("no returned value in envelope: {envelope}"))
        + marker.len();
    let rest = &envelope[start..];
    let end = rest
        .find('"')
        .unwrap_or_else(|| panic!("unterminated value in envelope: {envelope}"));
    rest[..end]
        .parse()
        .unwrap_or_else(|error| panic!("non-integer returned value {:?}: {error}", &rest[..end]))
}

fn run_interpreter(source: &str, label: &str) -> i64 {
    let ordinal = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "semaprax-owning-closure-exec-{label}-{}-{ordinal}.spx",
        std::process::id()
    ));
    std::fs::write(&path, source).unwrap();
    let interpretation =
        interpreter::interpret(&path, "app.main", &[], &InterpreterOptions::default());
    let _ = std::fs::remove_file(&path);
    let interpretation = interpretation
        .unwrap_or_else(|diagnostics| panic!("interpreter rejected {label}: {diagnostics:?}"));
    assert!(
        interpretation.returned,
        "{label} did not return on the interpreter: {}",
        interpretation.envelope
    );
    interpreter::verify_envelope(&interpretation.envelope).unwrap_or_else(|error| {
        panic!("{label} interpreter envelope failed verification: {error:?}")
    });
    extract_returned_value(&interpretation.envelope)
}

/// Compiles `generated` (the exact bytes `codegen::emit_c` returned for the
/// desugared program) with an allocation-counting `main` that calls
/// `app.main` directly through its declared C ABI, at the given
/// optimization level. Returns `(result, allocations, frees)`.
fn run_native(generated: &str, optimization: &str, label: &str, nonce: u64) -> (i64, u64, u64) {
    let ordinal = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!(
        "semaprax-owning-closure-exec-{label}-{optimization}-{}-{ordinal}",
        std::process::id()
    );
    let source = std::env::temp_dir().join(format!("{stem}.c"));
    let executable = std::env::temp_dir().join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
    let main_symbol = format!("spx_decl_{}", hex_identity("app.main"));
    let probe = format!(
        r#"#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

/* Test-only allocator observation, intercepting `calloc` as well as
 * `malloc`/`free`: `spx_bytes_zeroed` allocates with `calloc`, which the
 * shared `tests/native_owned_utf8_settlement_v1/allocations.c` fixture used
 * elsewhere in this suite does not intercept. Nothing here changes a
 * production ABI; the macros are undefined again before this file's own
 * `main`. */
static unsigned long spx_fixture_allocations = 0;
static unsigned long spx_fixture_frees = 0;
static void *spx_fixture_malloc(size_t size) {{
    spx_fixture_allocations += 1;
    return malloc(size);
}}
static void *spx_fixture_calloc(size_t count, size_t size) {{
    spx_fixture_allocations += 1;
    return calloc(count, size);
}}
static void spx_fixture_free(void *pointer) {{
    if (pointer != NULL) {{
        spx_fixture_frees += 1;
    }}
    free(pointer);
}}
#define malloc spx_fixture_malloc
#define calloc spx_fixture_calloc
#define free spx_fixture_free

{generated}

#undef malloc
#undef calloc
#undef free

int main(void) {{
    struct spx_status_entry entries[UINT32_C(8)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C({nonce}), entries, UINT32_C(8), NULL, NULL, NULL)) {{
        return 90;
    }}
    int64_t result = INT64_MIN;
    if ({main_symbol}(&context, &result) != SPX_STATUS_SUCCESS) {{
        return 91;
    }}
    printf("%lld %lu %lu\n", (long long)result, spx_fixture_allocations, spx_fixture_frees);
    return 0;
}}
"#
    );
    std::fs::write(&source, probe).unwrap();
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
        "{label} native {optimization} compilation failed: {}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let executed = Command::new(&executable).output().unwrap();
    let _ = std::fs::remove_file(&source);
    let _ = std::fs::remove_file(&executable);
    assert!(
        executed.status.success(),
        "{label} native {optimization} run failed: status={:?} stderr={}",
        executed.status.code(),
        String::from_utf8_lossy(&executed.stderr)
    );
    let stdout = String::from_utf8_lossy(&executed.stdout).trim().to_owned();
    let mut parts = stdout.split_whitespace();
    let result: i64 = parts
        .next()
        .unwrap_or_else(|| panic!("{label} native {optimization} produced no stdout"))
        .parse()
        .unwrap_or_else(|error| panic!("{label} native {optimization} malformed result: {error}"));
    let allocations: u64 = parts.next().unwrap().parse().unwrap();
    let frees: u64 = parts.next().unwrap().parse().unwrap();
    (result, allocations, frees)
}

/// Instantiates `wasm::emit_module`'s bytes in Node, providing the exact
/// host-import protocol `Bytes` needs on Core Wasm: `spx_bytes_zeroed`
/// allocates one tagged, tokenized entry and `spx_bytes_drop` frees it by
/// token, counted the same way the native probe counts `calloc`/`free`, so a
/// leaked or double-freed capture fails here too. `spx_bytes_copy`,
/// `spx_bytes_get` and `spx_bytes_as_slice` are provided as hard failures:
/// this bounded profile's body grammar never inspects the payload, so none
/// of the three cases may legitimately call them.
fn run_core_wasm(program: &semaprax::ast::Program, label: &str) -> (i64, u64, u64) {
    let bytes = wasm::emit_module(program).unwrap();
    assert_eq!(
        bytes,
        wasm::emit_module(program).unwrap(),
        "{label} Core Wasm emission is not deterministic"
    );
    let ordinal = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!(
        "semaprax-owning-closure-exec-{label}-wasm-{}-{ordinal}",
        std::process::id()
    );
    let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
    let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
    std::fs::write(&wasm_path, bytes).unwrap();
    std::fs::write(
        &script_path,
        r#"import { readFile } from "node:fs/promises";
const fail = (name) => () => { throw new Error(`unexpected host import ${name}`); };
const checked = (operation) => (a, b) => {
  const value = operation(a, b);
  if (value < -(1n << 63n) || value > (1n << 63n) - 1n) throw new RangeError();
  return value;
};
const bytes = await readFile(process.argv[2]);
const entries = new Map();
let nextToken = 1;
let allocations = 0;
let frees = 0;
const imports = { env: {
  spx_add: checked((a, b) => a + b),
  spx_sub: checked((a, b) => a - b),
  spx_mul: checked((a, b) => a * b),
  spx_div: (a, b) => a / b,
  spx_rem: (a, b) => a % b,
  spx_neg: (a) => -a,
  spx_contract_fail: () => { throw new Error("contract failure"); },
  spx_bytes_copy: fail("spx_bytes_copy"),
  spx_bytes_get: fail("spx_bytes_get"),
  spx_bytes_as_slice: fail("spx_bytes_as_slice"),
  spx_bytes_set: fail("spx_bytes_set"),
  spx_bytes_set5: fail("spx_bytes_set5"),
  spx_bytes_set1_or5: fail("spx_bytes_set1_or5"),
  spx_bytes_set1_or6_or48: fail("spx_bytes_set1_or6_or48"),
  spx_bytes_zeroed: (count) => {
    if (typeof count !== "bigint" || count < 0n || count > 131072n) {
      throw new Error("owned byte buffer capacity invariant");
    }
    const token = nextToken++;
    entries.set(token, new Uint8Array(Number(count)));
    allocations += 1;
    return BigInt.asIntN(64, ((0x80000000n | BigInt(token)) << 32n) | count);
  },
  spx_bytes_drop: (carrier) => {
    const word = BigInt.asUintN(64, carrier);
    const root = Number((word >> 32n) & 0xffffffffn);
    const token = root & 0x7fffffff;
    if ((root & 0x80000000) === 0 || !entries.delete(token)) {
      throw new Error("dropped an unknown or foreign byte token (leak or double free)");
    }
    frees += 1;
  },
} };
const module = await WebAssembly.compile(bytes);
for (const item of WebAssembly.Module.imports(module)) {
  if (item.module === "env" && item.kind === "function" && !(item.name in imports.env)) {
    imports.env[item.name] = fail(item.name);
  }
}
const instance = await WebAssembly.instantiate(module, imports);
const result = instance.exports.semaprax_main();
process.stdout.write(`${result.toString()} ${allocations} ${frees}\n`);
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
        "{label} Core Wasm run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let mut parts = stdout.split_whitespace();
    let result: i64 = parts
        .next()
        .unwrap_or_else(|| panic!("{label} Core Wasm produced no stdout"))
        .parse()
        .unwrap_or_else(|error| panic!("{label} Core Wasm malformed result: {error}"));
    let allocations: u64 = parts.next().unwrap().parse().unwrap();
    let frees: u64 = parts.next().unwrap().parse().unwrap();
    (result, allocations, frees)
}

/// Runs one shape's source through all three backends and asserts they
/// agree on the observable result AND on the capture's settlement count
/// (`expected_settlements`: 1 for both this profile's shapes -- the CALLED
/// shape settles by transfer into the target's own unused-parameter drop,
/// the UNCALLED shape settles by the ordinary scope-exit drop of an unmoved
/// owned local -- never 0 (a leak) and never 2 (a double free)).
fn assert_shape_agrees(
    source: &str,
    label: &str,
    expected_result: i64,
    expected_settlements: u64,
    nonce: u64,
) {
    if !command_available("clang") || !command_available("node") {
        return;
    }

    let program = checked_program(source, label);
    let generated = codegen::emit_c(&program).unwrap();
    assert_eq!(
        generated,
        codegen::emit_c(&program).unwrap(),
        "{label} native emission is not deterministic"
    );

    let interpreter_value = run_interpreter(source, label);
    let (native_o0, native_o0_allocations, native_o0_frees) =
        run_native(&generated, "-O0", label, nonce);
    let (native_o2, native_o2_allocations, native_o2_frees) =
        run_native(&generated, "-O2", label, nonce.wrapping_add(1));
    let (core_wasm, wasm_allocations, wasm_frees) = run_core_wasm(&program, label);

    assert_eq!(
        interpreter_value, expected_result,
        "{label}: interpreter disagrees with the independently pinned expectation"
    );
    assert_eq!(
        native_o0, expected_result,
        "{label}: native -O0 disagrees with the independently pinned expectation"
    );
    assert_eq!(
        native_o2, expected_result,
        "{label}: native -O2 disagrees with the independently pinned expectation"
    );
    assert_eq!(
        core_wasm, expected_result,
        "{label}: Core Wasm disagrees with the independently pinned expectation"
    );
    assert_eq!(
        native_o0, native_o2,
        "{label}: native optimization level changed the observable result"
    );
    assert_eq!(
        interpreter_value, core_wasm,
        "{label}: interpreter and Core Wasm disagree with each other"
    );

    for (engine, allocations, frees) in [
        ("native -O0", native_o0_allocations, native_o0_frees),
        ("native -O2", native_o2_allocations, native_o2_frees),
        ("Core Wasm", wasm_allocations, wasm_frees),
    ] {
        assert_eq!(
            allocations, expected_settlements,
            "{label} on {engine}: expected exactly {expected_settlements} allocation(s) of the \
             captured owner, got {allocations}"
        );
        assert_eq!(
            frees, expected_settlements,
            "{label} on {engine}: expected exactly {expected_settlements} settlement(s) of the \
             captured owner (a mismatch against allocations means a leak or a double free), got \
             {frees}"
        );
    }
}

/// Required evidence (issue #120): the CALLED shape -- the captured owner is
/// transferred into `checksum` and never finalized directly -- executes
/// identically on the interpreter, native C11 at `-O0`/`-O2`, and Core Wasm,
/// and settles its one captured `Bytes` allocation exactly once on every
/// backend that has a real allocator to observe.
#[test]
fn called_owning_closure_agrees_across_interpreter_native_and_core_wasm() {
    assert_shape_agrees(CALLED_SOURCE, "called", CALLED_EXPECTED, 1, 7001);
}

/// Required evidence (issue #120): the UNCALLED shape -- the closure is
/// constructed and dropped without ever being called -- executes
/// identically on the interpreter, native C11 at `-O0`/`-O2`, and Core Wasm,
/// and settles its captured `Bytes` allocation through the ordinary
/// scope-exit drop exactly once (not zero -- a leak -- and not two -- a
/// double free) on every backend that has a real allocator to observe. This
/// is the shape most likely to diverge if `owning_desugar.rs`'s substitution
/// dropped a cleanup obligation, per that module's own doc comment.
#[test]
fn uncalled_owning_closure_agrees_across_interpreter_native_and_core_wasm() {
    assert_shape_agrees(UNCALLED_SOURCE, "uncalled", UNCALLED_EXPECTED, 1, 7101);
}

/// Issue #578 (DV-18): an owning callback's substitution is lexical. The
/// first block's `own fn` callback must not rewrite the second, disjoint
/// block's ordinary zero-capture `callback()`; the independently expected
/// result is 11 + 33 = 44 (all three backends previously agreed on 22).
const DISJOINT_SCOPES_SOURCE: &str = r#"
module audit.own_closure_scope;

@id("audit.inspect")
fn inspect(payload: own Bytes) -> i64
{
    11
}

@id("app.main")
fn main() -> i64
{
    let first = { let payload = bytes_zeroed(1usize); let callback = own fn() -> i64 { inspect(payload) }; callback() };
    let second = { let payload = bytes_zeroed(2usize); let callback = fn() -> i64 { 33 }; callback() };
    first + second
}
"#;

/// The secondary manifestation: without the unrelated second `payload`, the
/// leaked substitution previously named an out-of-scope capture (SPX-T202).
const DISJOINT_SCOPES_NO_SECOND_PAYLOAD_SOURCE: &str = r#"
module audit.own_closure_scope_single;

@id("audit.inspect")
fn inspect(payload: own Bytes) -> i64
{
    11
}

@id("app.main")
fn main() -> i64
{
    let first = { let payload = bytes_zeroed(1usize); let callback = own fn() -> i64 { inspect(payload) }; callback() };
    let second = { let callback = fn() -> i64 { 33 }; callback() };
    first + second
}
"#;

#[test]
fn owning_callback_substitution_does_not_leak_into_a_disjoint_block() {
    assert_eq!(
        run_interpreter(DISJOINT_SCOPES_SOURCE, "disjoint-scopes"),
        44
    );
    assert_shape_agrees(DISJOINT_SCOPES_SOURCE, "disjoint-scopes", 44, 2, 7201);
    // Renaming only the second callback must not change behavior.
    let renamed = DISJOINT_SCOPES_SOURCE
        .replace("let callback = fn()", "let other = fn()")
        .replace("33 }; callback()", "33 }; other()");
    assert_ne!(renamed, DISJOINT_SCOPES_SOURCE);
    assert_shape_agrees(&renamed, "disjoint-scopes-renamed", 44, 2, 7301);
}

#[test]
fn owning_callback_substitution_never_introduces_an_out_of_scope_capture() {
    assert_eq!(
        run_interpreter(DISJOINT_SCOPES_NO_SECOND_PAYLOAD_SOURCE, "disjoint-single"),
        44
    );
    assert_shape_agrees(
        DISJOINT_SCOPES_NO_SECOND_PAYLOAD_SOURCE,
        "disjoint-single",
        44,
        1,
        7401,
    );
}
