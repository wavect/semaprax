use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::{codegen, parse, verify, wasm};

const REQUIRE_ENV: &str = "SEMAPRAX_REQUIRE_SCALAR_BACKEND_EQUIVALENCE";

const STATUS_FIXTURE: &str = r#"
module test.scalar_status_backend_equivalence;

@id("case.success.i64")
fn success_i64() -> i64 { 42 }

@id("case.success.bool")
fn success_bool() -> bool { true }

@id("case.requires")
fn requires_false() -> i64 requires false { 11 }

@id("case.ensures")
fn ensures_false() -> i64 ensures false { 12 }

@id("case.add")
fn add_overflow() -> i64 { 9223372036854775807 + 1 }

@id("case.sub")
fn sub_overflow() -> i64 { -9223372036854775807 - 2 }

@id("case.mul")
fn mul_overflow() -> i64 { 9223372036854775807 * 2 }

@id("case.div.zero")
fn division_by_zero() -> i64 { 7 / 0 }

@id("case.div.overflow")
fn division_overflow() -> i64 { (-9223372036854775807 - 1) / -1 }

@id("case.rem.zero")
fn remainder_by_zero() -> i64 { 7 % 0 }

@id("case.rem.overflow")
fn remainder_overflow() -> i64 { (-9223372036854775807 - 1) % -1 }

@id("case.neg")
fn negation_overflow() -> i64 { -(-9223372036854775807 - 1) }

@id("case.i32")
fn i32_overflow() -> i64 {
    let maximum = 2147483647i32;
    let ignored = maximum + 1i32;
    0
}

@id("case.u8")
fn u8_overflow() -> i64 {
    let maximum = 255u8;
    let ignored = maximum + 1u8;
    0
}

@id("case.first")
fn first() -> i64 requires false { 1 }

@id("case.second")
fn second() -> i64 ensures false { 2 }

@id("case.sum")
fn sum(left: i64, right: i64) -> i64 { left + right }

@id("case.nested")
fn nested() -> i64 { sum(first(), second()) }

@id("app.main")
fn main() -> i64 { success_i64() }
"#;

const EXPORT_IDS: &[&str] = &[
    "case.add",
    "case.div.overflow",
    "case.div.zero",
    "case.ensures",
    "case.mul",
    "case.neg",
    "case.i32",
    "case.u8",
    "case.nested",
    "case.rem.overflow",
    "case.rem.zero",
    "case.requires",
    "case.sub",
    "case.success.bool",
    "case.success.i64",
];

const EXPECTED_TRANSCRIPT: &str = concat!(
    "{\"id\":\"case.success.i64\",\"ok\":true,\"value\":\"42\"}\n",
    "{\"id\":\"case.success.bool\",\"ok\":true,\"value\":true}\n",
    "{\"id\":\"case.requires\",\"ok\":false,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.contract.v1\",\"code\":1}}\n",
    "{\"id\":\"case.ensures\",\"ok\":false,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.contract.v1\",\"code\":2}}\n",
    "{\"id\":\"case.add\",\"ok\":false,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.arithmetic.v1\",\"code\":1}}\n",
    "{\"id\":\"case.sub\",\"ok\":false,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.arithmetic.v1\",\"code\":2}}\n",
    "{\"id\":\"case.mul\",\"ok\":false,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.arithmetic.v1\",\"code\":3}}\n",
    "{\"id\":\"case.div.zero\",\"ok\":false,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.arithmetic.v1\",\"code\":4}}\n",
    "{\"id\":\"case.div.overflow\",\"ok\":false,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.arithmetic.v1\",\"code\":5}}\n",
    "{\"id\":\"case.rem.zero\",\"ok\":false,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.arithmetic.v1\",\"code\":6}}\n",
    "{\"id\":\"case.rem.overflow\",\"ok\":false,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.arithmetic.v1\",\"code\":7}}\n",
    "{\"id\":\"case.neg\",\"ok\":false,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.arithmetic.v1\",\"code\":8}}\n",
    "{\"id\":\"case.i32\",\"ok\":false,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.arithmetic.v1\",\"code\":1}}\n",
    "{\"id\":\"case.u8\",\"ok\":false,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.arithmetic.v1\",\"code\":1}}\n",
    "{\"id\":\"case.nested\",\"ok\":false,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.contract.v1\",\"code\":1}}\n",
);

static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(0);

fn tool_available(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn required() -> bool {
    std::env::var_os(REQUIRE_ENV).is_some()
}

fn require_tools_or_skip() -> bool {
    let missing = ["clang", "node"]
        .into_iter()
        .filter(|tool| !tool_available(tool))
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return true;
    }
    assert!(
        !required(),
        "{REQUIRE_ENV} requires clang and Node; missing {}",
        missing.join(", ")
    );
    false
}

fn temporary_root() -> PathBuf {
    let ordinal = NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "semaprax-scalar-status-equivalence-{}-{ordinal}",
        std::process::id()
    ))
}

fn c_symbol(declaration_id: &str) -> String {
    let mut symbol = String::from("spx_decl_");
    for byte in declaration_id.bytes() {
        symbol.push_str(&format!("{byte:02x}"));
    }
    symbol
}

fn native_probe() -> String {
    let mut source = r#"
typedef spx_status_token (*spx_i64_case)(struct spx_context *, int64_t *);
typedef spx_status_token (*spx_bool_case)(struct spx_context *, bool *);

static int spx_emit_i64_success(const char *id, spx_i64_case test_case) {
    struct spx_status_entry records[UINT32_C(2)];
    struct spx_context context = {0};
    if (!spx_context_init(&context, UINT64_C(401), records, UINT32_C(2), NULL, NULL, NULL)) return 10;
    int64_t value = -INT64_C(777777777777777777);
    spx_status_token token = test_case(&context, &value);
    if (token != SPX_STATUS_SUCCESS || context.status_arena.length != UINT32_C(0)) return 11;
    printf("{\"id\":\"%s\",\"ok\":true,\"value\":\"%lld\"}\n", id, (long long)value);
    return 0;
}

static int spx_emit_bool_success(const char *id, spx_bool_case test_case) {
    struct spx_status_entry records[UINT32_C(2)];
    struct spx_context context = {0};
    if (!spx_context_init(&context, UINT64_C(402), records, UINT32_C(2), NULL, NULL, NULL)) return 20;
    bool value = false;
    spx_status_token token = test_case(&context, &value);
    if (token != SPX_STATUS_SUCCESS || context.status_arena.length != UINT32_C(0)) return 21;
    printf("{\"id\":\"%s\",\"ok\":true,\"value\":%s}\n", id, value ? "true" : "false");
    return 0;
}

static int spx_emit_failure(const char *id, spx_i64_case test_case) {
    struct spx_status_entry records[UINT32_C(2)];
    struct spx_context context = {0};
    if (!spx_context_init(&context, UINT64_C(403), records, UINT32_C(2), NULL, NULL, NULL)) return 30;
    const int64_t poison = -INT64_C(777777777777777777);
    int64_t value = poison;
    spx_status_token token = test_case(&context, &value);
    if (token != UINT32_C(1) || value != poison || context.status_arena.length != UINT32_C(1)) return 31;
    const struct spx_normalized_status *status = spx_status_resolve(&context, token);
    if (status == NULL) return 32;
    printf(
        "{\"id\":\"%s\",\"ok\":false,\"status\":{\"schema\":\"%s\",\"domain_id\":\"%s\",\"code\":%u}}\n",
        id, status->schema, status->domain_id, (unsigned int)status->code
    );
    return 0;
}

int main(void) {
    int result = spx_emit_i64_success("case.success.i64", __SUCCESS_I64__);
    if (result != 0) return result;
    result = spx_emit_bool_success("case.success.bool", __SUCCESS_BOOL__);
    if (result != 0) return result;
    result = spx_emit_failure("case.requires", __REQUIRES__);
    if (result != 0) return result;
    result = spx_emit_failure("case.ensures", __ENSURES__);
    if (result != 0) return result;
    result = spx_emit_failure("case.add", __ADD__);
    if (result != 0) return result;
    result = spx_emit_failure("case.sub", __SUB__);
    if (result != 0) return result;
    result = spx_emit_failure("case.mul", __MUL__);
    if (result != 0) return result;
    result = spx_emit_failure("case.div.zero", __DIV_ZERO__);
    if (result != 0) return result;
    result = spx_emit_failure("case.div.overflow", __DIV_OVERFLOW__);
    if (result != 0) return result;
    result = spx_emit_failure("case.rem.zero", __REM_ZERO__);
    if (result != 0) return result;
    result = spx_emit_failure("case.rem.overflow", __REM_OVERFLOW__);
    if (result != 0) return result;
    result = spx_emit_failure("case.neg", __NEG__);
    if (result != 0) return result;
    result = spx_emit_failure("case.i32", __I32__);
    if (result != 0) return result;
    result = spx_emit_failure("case.u8", __U8__);
    if (result != 0) return result;
    result = spx_emit_failure("case.nested", __NESTED__);
    if (result != 0) return result;
    return 0;
}
"#
    .to_owned();
    for (placeholder, declaration_id) in [
        ("__SUCCESS_I64__", "case.success.i64"),
        ("__SUCCESS_BOOL__", "case.success.bool"),
        ("__REQUIRES__", "case.requires"),
        ("__ENSURES__", "case.ensures"),
        ("__ADD__", "case.add"),
        ("__SUB__", "case.sub"),
        ("__MUL__", "case.mul"),
        ("__DIV_ZERO__", "case.div.zero"),
        ("__DIV_OVERFLOW__", "case.div.overflow"),
        ("__REM_ZERO__", "case.rem.zero"),
        ("__REM_OVERFLOW__", "case.rem.overflow"),
        ("__NEG__", "case.neg"),
        ("__I32__", "case.i32"),
        ("__U8__", "case.u8"),
        ("__NESTED__", "case.nested"),
    ] {
        source = source.replace(placeholder, &c_symbol(declaration_id));
    }
    source
}

fn normalized_stdout(output: Output, label: &str) -> Vec<u8> {
    assert!(
        output.status.success(),
        "{label} failed with {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .replace("\r\n", "\n")
        .into_bytes()
}

fn run_native(generated: &str, root: &Path, optimization: &str) -> Vec<u8> {
    let source = root.join(format!("native-{optimization}.c"));
    let executable = root.join(format!(
        "native-{optimization}{}",
        std::env::consts::EXE_SUFFIX
    ));
    std::fs::write(&source, format!("{generated}\n{}", native_probe())).unwrap();
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
        "native {optimization} compilation failed: {}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    normalized_stdout(
        Command::new(&executable).output().unwrap(),
        &format!("native {optimization}"),
    )
}

fn run_core_wasm(program: &semaprax::ast::Program, root: &Path) -> Vec<u8> {
    let package = root.join("web");
    let selected = EXPORT_IDS
        .iter()
        .map(|stable_id| (*stable_id).to_owned())
        .collect::<Vec<_>>();
    wasm::build_web_with_scalar_exports(program, &package, &selected).unwrap();
    let script = root.join("observe-core-wasm.mjs");
    std::fs::write(
        &script,
        r#"import { readFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";
import { resolve } from "node:path";

const packageDirectory = resolve(process.argv[2]);
const bindings = await import(pathToFileURL(resolve(packageDirectory, "semaprax.bindings.js")));
const runtime = await bindings.instantiateBytes(await readFile(resolve(packageDirectory, "app.wasm")));
const cases = [
  "case.success.i64",
  "case.success.bool",
  "case.requires",
  "case.ensures",
  "case.add",
  "case.sub",
  "case.mul",
  "case.div.zero",
  "case.div.overflow",
  "case.rem.zero",
  "case.rem.overflow",
  "case.neg",
  "case.i32",
  "case.u8",
  "case.nested",
];
for (const id of cases) {
  const outcome = runtime.call(id);
  const observation = outcome.ok
    ? { id, ok: true, value: typeof outcome.value === "bigint" ? outcome.value.toString() : outcome.value }
    : { id, ok: false, status: {
        schema: outcome.status.schema,
        domain_id: outcome.status.domain_id,
        code: outcome.status.code,
      } };
  process.stdout.write(`${JSON.stringify(observation)}\n`);
}
"#,
    )
    .unwrap();
    normalized_stdout(
        Command::new("node")
            .arg(&script)
            .arg(&package)
            .output()
            .unwrap(),
        "Core-Wasm Node observer",
    )
}

#[test]
fn native_o0_o2_and_core_wasm_share_exact_scalar_status_results() {
    if !require_tools_or_skip() {
        return;
    }

    let program = parse(
        STATUS_FIXTURE,
        Path::new("scalar-status-backend-equivalence.spx"),
    )
    .unwrap();
    let diagnostics = verify::verify(&program);
    assert!(
        diagnostics.is_empty(),
        "fixture verification failed: {diagnostics:?}"
    );
    let generated = codegen::emit_c(&program).unwrap();
    let root = temporary_root();
    std::fs::create_dir(&root).unwrap();

    let native_o0 = run_native(&generated, &root, "-O0");
    let native_o2 = run_native(&generated, &root, "-O2");
    let core_wasm = run_core_wasm(&program, &root);
    assert_eq!(native_o0, native_o2, "native optimization changed results");
    assert_eq!(native_o0, core_wasm, "native and Core-Wasm results differ");
    assert_eq!(native_o0, EXPECTED_TRANSCRIPT.as_bytes());

    let _ = std::fs::remove_dir_all(root);
}
