use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::hir::{DeclarationId, FunctionExecutionId, FunctionInstanceId, ResolvedType};
use semaprax::{codegen, parse, wasm};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const SOURCE: &str = r#"
module test.executable_generic_functions;

@id("test.select")
fn select<T>(left: T, right: T, flag: bool) -> T {
    if flag { left } else { right }
}

@id("test.reverse")
fn reverse<T>(left: T, right: T, flag: bool) -> T {
    if flag { right } else { left }
}

@id("test.first")
fn first<T, U>(left: T, right: U) -> T { left }

@id("test.preserve")
fn preserve<T>(marker: bool) -> bool { marker }

@id("test.invert")
fn invert<T>(marker: bool) -> bool { !marker }

@id("test.guard")
fn guard<T>(value: T, allowed: bool) -> T
    requires allowed
    ensures result == value
{
    value
}

@id("test.post")
fn post<T>(value: T) -> T ensures false { value }

@id("test.crash")
fn crash() -> i64 requires false { 1 / 0 }

@id("test.body_fail")
fn body_fail<T>(value: T) -> T {
    if crash() == 0 { value } else { value }
}

@id("test.argument_failure")
fn argument_failure() -> i64 { guard<i64>(1 / 0, false) }

@id("test.post_failure")
fn post_failure() -> i64 { post<i64>(7) }

@id("test.body_failure")
fn body_failure() -> i64 { body_fail<i64>(7) }

@id("test.branch_check")
fn branch_check() -> bool {
    let first = select<i64>(42, 90, true);
    let second = reverse<i64>(90, 42, true);
    let third = select<i64>(90, 42, false);
    let fourth = reverse<i64>(42, 90, false);
    let truth = select<bool>(true, false, true);
    let falsity = reverse<bool>(true, false, true);
    let truth_else = reverse<bool>(true, false, false);
    let falsity_else = select<bool>(true, false, false);
    truth && !falsity && truth_else && !falsity_else && first == second && second == third && third == fourth
}

@id("test.ordered_check")
fn ordered_check() -> bool {
    let ordered_number = first<i64, bool>(42, true);
    let ordered_flag = first<bool, i64>(true, 42);
    ordered_flag && ordered_number == 42
}

@id("test.identity_check")
fn identity_check() -> bool {
    let preserved_i64 = preserve<i64>(true);
    let preserved_bool = preserve<bool>(true);
    let inverted_i64 = invert<i64>(false);
    let inverted_bool = invert<bool>(false);
    preserved_i64 && preserved_bool && inverted_i64 && inverted_bool
}

@id("app.main")
fn main() -> i64 {
    if branch_check() && ordered_check() && identity_check() { 42 } else { 0 }
}
"#;

const WASM_FAILURE_SOURCE: &str = r#"
module test.executable_generic_function_failure;
@id("test.guard")
fn guard<T>(value: T, allowed: bool) -> T requires allowed { value }
@id("test.post")
fn post<T>(value: T) -> T ensures false { value }
@id("test.crash")
fn crash() -> i64 requires false { 0 }
@id("test.body_fail")
fn body_fail<T>(value: T) -> T {
    if crash() == 0 { value } else { value }
}
@id("app.main")
fn main() -> i64 { FAILURE_BODY }
"#;

fn command_available(name: &str) -> bool {
    Command::new(name).arg("--version").output().is_ok()
}

fn execution_symbol(template: &str, type_arguments: &[ResolvedType]) -> String {
    let template = DeclarationId::new(template);
    let instance = FunctionInstanceId::derive(&template, type_arguments);
    let identity = FunctionExecutionId::Generic(instance).identity_key();
    let mut symbol = String::from("spx_exec_");
    for byte in identity.bytes() {
        write!(symbol, "{byte:02x}").unwrap();
    }
    symbol
}

fn declaration_symbol(id: &str) -> String {
    let mut symbol = String::from("spx_decl_");
    for byte in id.bytes() {
        write!(symbol, "{byte:02x}").unwrap();
    }
    symbol
}

fn run_node_failure(body: &str, expected_contracts: u32, expected_divisions: u32) {
    let source = WASM_FAILURE_SOURCE.replace("FAILURE_BODY", body);
    let program = parse(&source, Path::new("generic-function-wasm-failure.spx")).unwrap();
    let bytes = wasm::emit_module(&program).unwrap();
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!(
        "semaprax-generic-function-wasm-failure-{}-{id}",
        std::process::id()
    );
    let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
    let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
    std::fs::write(&wasm_path, bytes).unwrap();
    std::fs::write(
        &script_path,
        r#"import { readFile } from "node:fs/promises";
const bytes = await readFile(process.argv[2]);
const expectedContracts = Number(process.argv[3]);
const expectedDivisions = Number(process.argv[4]);
let contracts = 0;
let divisions = 0;
const unexpected = (name) => () => { throw new Error(`unexpected host import ${name}`); };
const { instance } = await WebAssembly.instantiate(bytes, { env: {
  spx_add: unexpected("spx_add"), spx_sub: unexpected("spx_sub"), spx_mul: unexpected("spx_mul"),
  spx_div: () => { divisions += 1; throw new RangeError("expected division failure"); },
  spx_rem: unexpected("spx_rem"), spx_neg: unexpected("spx_neg"),
  spx_contract_fail: () => { contracts += 1; throw new Error("expected contract failure"); },
} });
let trapped = false;
try { instance.exports.semaprax_main(); } catch { trapped = true; }
if (!trapped) throw new Error("generic failure did not trap");
if (contracts !== expectedContracts || divisions !== expectedDivisions) {
  throw new Error(`wrong failure order contracts=${contracts} divisions=${divisions}`);
}
"#,
    )
    .unwrap();
    let output = Command::new("node")
        .arg(&script_path)
        .arg(&wasm_path)
        .arg(expected_contracts.to_string())
        .arg(expected_divisions.to_string())
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&script_path);
    let _ = std::fs::remove_file(&wasm_path);
    assert!(
        output.status.success(),
        "Node generic failure runtime failed for `{body}`: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn native_generic_functions_use_exact_instances_and_preserve_failures_at_o0_o2() {
    if !command_available("clang") {
        return;
    }
    let program = parse(SOURCE, Path::new("generic-function-native.spx")).unwrap();
    let generated = codegen::emit_c(&program).unwrap();
    assert_eq!(generated, codegen::emit_c(&program).unwrap());

    let select_i64 = execution_symbol("test.select", &[ResolvedType::I64]);
    let select_bool = execution_symbol("test.select", &[ResolvedType::Bool]);
    let reverse_i64 = execution_symbol("test.reverse", &[ResolvedType::I64]);
    let reverse_bool = execution_symbol("test.reverse", &[ResolvedType::Bool]);
    let guard_i64 = execution_symbol("test.guard", &[ResolvedType::I64]);
    let post_i64 = execution_symbol("test.post", &[ResolvedType::I64]);
    let body_fail_i64 = execution_symbol("test.body_fail", &[ResolvedType::I64]);
    let first_i64_bool = execution_symbol("test.first", &[ResolvedType::I64, ResolvedType::Bool]);
    let first_bool_i64 = execution_symbol("test.first", &[ResolvedType::Bool, ResolvedType::I64]);
    let preserve_i64 = execution_symbol("test.preserve", &[ResolvedType::I64]);
    let preserve_bool = execution_symbol("test.preserve", &[ResolvedType::Bool]);
    let invert_i64 = execution_symbol("test.invert", &[ResolvedType::I64]);
    let invert_bool = execution_symbol("test.invert", &[ResolvedType::Bool]);
    for symbol in [
        &select_i64,
        &select_bool,
        &reverse_i64,
        &reverse_bool,
        &guard_i64,
        &post_i64,
        &body_fail_i64,
        &first_i64_bool,
        &first_bool_i64,
        &preserve_i64,
        &preserve_bool,
        &invert_i64,
        &invert_bool,
    ] {
        assert!(
            generated.contains(symbol),
            "missing exact function instance {symbol}"
        );
    }
    assert_ne!(select_i64, select_bool);
    assert_ne!(select_i64, reverse_i64);
    assert_ne!(first_i64_bool, first_bool_i64);
    assert_ne!(preserve_i64, preserve_bool);
    assert_ne!(preserve_i64, invert_i64);
    assert_ne!(preserve_bool, invert_bool);

    let probe = format!(
        r#"
#include <string.h>
static int spx_test_status(const struct spx_context *context, spx_status_token token, const char *domain, uint32_t code) {{
    const struct spx_normalized_status *status = spx_status_resolve(context, token);
    return status != NULL && strcmp(status->domain_id, domain) == 0 && status->code == code;
}}
static int spx_test_contract_detail(const struct spx_context *context, spx_status_token token, const char *function, const char *expression) {{
    const struct spx_status_detail *detail = spx_status_resolve_detail(context, token);
    return detail != NULL && detail->failure_function != NULL && detail->failure_expression != NULL &&
        strcmp(detail->failure_function, function) == 0 && strcmp(detail->failure_expression, expression) == 0;
}}
int main(void) {{
    struct spx_status_entry entries[UINT32_C(32)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(1414), entries, UINT32_C(32), NULL, NULL, NULL)) return 10;

    int64_t scalar = INT64_C(-1);
    if ({select_i64}(&context, INT64_C(19), INT64_C(90), true, &scalar) != SPX_STATUS_SUCCESS || scalar != INT64_C(19)) return 11;
    if ({reverse_i64}(&context, INT64_C(90), INT64_C(23), true, &scalar) != SPX_STATUS_SUCCESS || scalar != INT64_C(23)) return 12;
    if ({select_i64}(&context, INT64_C(19), INT64_C(90), false, &scalar) != SPX_STATUS_SUCCESS || scalar != INT64_C(90)) return 22;
    if ({reverse_i64}(&context, INT64_C(90), INT64_C(23), false, &scalar) != SPX_STATUS_SUCCESS || scalar != INT64_C(90)) return 23;
    bool flag = false;
    if ({select_bool}(&context, true, false, true, &flag) != SPX_STATUS_SUCCESS || !flag) return 13;
    if ({reverse_bool}(&context, true, false, true, &flag) != SPX_STATUS_SUCCESS || flag) return 14;
    if ({select_bool}(&context, true, false, false, &flag) != SPX_STATUS_SUCCESS || flag) return 24;
    if ({reverse_bool}(&context, true, false, false, &flag) != SPX_STATUS_SUCCESS || !flag) return 25;
    if ({first_i64_bool}(&context, INT64_C(42), true, &scalar) != SPX_STATUS_SUCCESS || scalar != INT64_C(42)) return 20;
    if ({first_bool_i64}(&context, true, INT64_C(42), &flag) != SPX_STATUS_SUCCESS || !flag) return 21;
    if ({preserve_i64}(&context, true, &flag) != SPX_STATUS_SUCCESS || !flag) return 26;
    if ({preserve_bool}(&context, false, &flag) != SPX_STATUS_SUCCESS || flag) return 27;
    if ({invert_i64}(&context, false, &flag) != SPX_STATUS_SUCCESS || !flag) return 28;
    if ({invert_bool}(&context, true, &flag) != SPX_STATUS_SUCCESS || flag) return 29;
    if ({app_main}(&context, &scalar) != SPX_STATUS_SUCCESS || scalar != INT64_C(42)) return 15;

    scalar = INT64_C(0x2525252525252525);
    spx_status_token status = {guard_i64}(&context, INT64_C(7), false, &scalar);
    if (!spx_test_status(&context, status, "semaprax.contract.v1", UINT32_C(1)) ||
        !spx_test_contract_detail(&context, status, "guard", "allowed") ||
        scalar != INT64_C(0x2525252525252525)) return 16;

    scalar = INT64_C(0x2525252525252525);
    status = {post_i64}(&context, INT64_C(7), &scalar);
    if (!spx_test_status(&context, status, "semaprax.contract.v1", UINT32_C(2)) ||
        !spx_test_contract_detail(&context, status, "post", "false") ||
        scalar != INT64_C(0x2525252525252525)) return 17;

    scalar = INT64_C(0x2525252525252525);
    status = {body_fail_i64}(&context, INT64_C(7), &scalar);
    if (!spx_test_status(&context, status, "semaprax.contract.v1", UINT32_C(1)) ||
        !spx_test_contract_detail(&context, status, "crash", "false") ||
        scalar != INT64_C(0x2525252525252525)) return 18;

    uint32_t before = context.status_arena.length;
    scalar = INT64_C(0x2525252525252525);
    status = {argument_failure}(&context, &scalar);
    if (!spx_test_status(&context, status, "semaprax.arithmetic.v1", UINT32_C(4)) ||
        scalar != INT64_C(0x2525252525252525) ||
        context.status_arena.length != before + UINT32_C(1)) return 19;
    return 0;
}}
"#,
        app_main = declaration_symbol("app.main"),
        argument_failure = declaration_symbol("test.argument_failure"),
    );

    for optimization in ["-O0", "-O2"] {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!(
            "semaprax-generic-function-native-{}-{id}",
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
            "generic function C failed at {optimization}: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let executed = Command::new(&executable).output().unwrap();
        let _ = std::fs::remove_file(&source);
        let _ = std::fs::remove_file(&executable);
        assert!(
            executed.status.success(),
            "generic function executable failed at {optimization}: status={:?} stderr={}",
            executed.status.code(),
            String::from_utf8_lossy(&executed.stderr)
        );
    }
}

#[test]
fn generic_functions_are_equivalent_in_node_wasm_with_reentry() {
    if !command_available("node") {
        return;
    }
    let program = parse(SOURCE, Path::new("generic-function-wasm.spx")).unwrap();
    let bytes = wasm::emit_module(&program).unwrap();
    assert_eq!(bytes, wasm::emit_module(&program).unwrap());
    let resolved = semaprax::hir::resolve(&program).unwrap();
    let expected_functions =
        u32::try_from(resolved.functions.len() + resolved.function_instances.len()).unwrap();
    let emitted_functions = wasmparser::Parser::new(0)
        .parse_all(&bytes)
        .filter_map(|payload| match payload.unwrap() {
            wasmparser::Payload::CodeSectionStart { count, .. } => Some(count),
            _ => None,
        })
        .sum::<u32>();
    assert_eq!(emitted_functions, expected_functions);
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!("semaprax-generic-function-wasm-{}-{id}", std::process::id());
    let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
    let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
    std::fs::write(&wasm_path, bytes).unwrap();
    std::fs::write(
        &script_path,
        r#"import { readFile } from "node:fs/promises";
const fail = (name) => () => { throw new Error(`unexpected host import ${name}`); };
const bytes = await readFile(process.argv[2]);
const { instance } = await WebAssembly.instantiate(bytes, { env: {
  spx_add: fail("spx_add"), spx_sub: fail("spx_sub"), spx_mul: fail("spx_mul"),
  spx_div: fail("spx_div"), spx_rem: fail("spx_rem"), spx_neg: fail("spx_neg"),
  spx_contract_fail: fail("spx_contract_fail"),
} });
for (let index = 0; index < 4096; index += 1) {
  if (instance.exports.semaprax_main() !== 42n) throw new Error("generic function result mismatch");
}
console.log("generic-function-wasm-v1-ok");
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
        "Node generic function runtime failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "generic-function-wasm-v1-ok"
    );

    run_node_failure("guard<i64>(7, false)", 1, 0);
    run_node_failure("post<i64>(7)", 1, 0);
    run_node_failure("body_fail<i64>(7)", 1, 0);
    run_node_failure("guard<i64>(1 / 0, false)", 0, 1);
}
