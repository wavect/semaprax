use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::{codegen, parse, wasm};
use sha2::{Digest as _, Sha256};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const SOURCE: &str = r#"
module test.executable_record_patterns;

@id("test.inner")
record Inner {
    @id("test.inner.value") value: i64,
    @id("test.inner.flag") flag: bool,
}

@id("test.outer")
record Outer {
    @id("test.outer.inner") inner: Inner,
    @id("test.outer.other") other: i64,
}

@id("test.box")
record Box<T> { @id("test.box.value") value: T, }

@id("test.make")
fn make(value: i64, flag: bool, other_value: i64) -> Outer {
    Outer { inner: Inner { value: value, flag: flag }, other: other_value }
}

@id("test.unpack")
fn unpack(value: i64, flag: bool, other_value: i64) -> i64 {
    match make(value, flag, other_value) {
        Outer { inner: Inner { value: nested, flag: truth }, other } =>
            if truth { nested + other } else { other - nested },
    }
}

@id("test.ignore")
fn ignore(input: Outer) -> i64 {
    match input { Outer { inner: _, other } => other, }
}

@id("test.whole_inner")
fn whole_inner(input: Outer) -> i64 {
    match input {
        Outer { inner, other: _ } => if inner.flag { inner.value } else { 0 },
    }
}

@id("test.read_i64")
fn read_i64(input: Box<i64>) -> i64 { match input { Box { value } => value, } }

@id("test.read_bool")
fn read_bool(input: Box<bool>) -> i64 {
    match input { Box { value: truth } => if truth { 1 } else { 0 }, }
}

@id("test.fail_make")
fn fail_make() -> Outer requires false {
    Outer { inner: Inner { value: 1, flag: true }, other: 2 }
}

@id("test.scrutinee_failure")
fn scrutinee_failure() -> i64 {
    match fail_make() {
        Outer { inner: Inner { value, flag: _ }, other: _ } => value / 0,
    }
}

@id("test.arm_failure")
fn arm_failure(input: Outer) -> i64 {
    match input {
        Outer { inner: Inner { value, flag: _ }, other: _ } => value / 0,
    }
}

@id("test.post_failure")
fn post_failure(input: Outer) -> i64 ensures false {
    match input {
        Outer { inner: Inner { value, flag: _ }, other: _ } => value,
    }
}

@id("app.main")
fn main() -> i64 {
    let truth = unpack(20, true, 22);
    let falsity = unpack(20, false, 22);
    let scalar = read_i64(Box<i64> { value: 0 });
    let yes = read_bool(Box<bool> { value: true });
    let no = read_bool(Box<bool> { value: false });
    let ignored = ignore(make(0, false, 7));
    let whole_yes = whole_inner(make(5, true, 0));
    let whole_no = whole_inner(make(5, false, 0));
    truth + falsity - 2 + scalar + yes + no - 1 + ignored - 7 + whole_yes + whole_no - 5
}
"#;

fn command_available(name: &str) -> bool {
    Command::new(name).arg("--version").output().is_ok()
}

fn hex_identity(value: &str) -> String {
    let mut output = String::with_capacity(value.len() * 2);
    for byte in value.bytes() {
        write!(output, "{byte:02x}").unwrap();
    }
    output
}

fn generic_record_symbol(declaration: &str, argument: &str) -> String {
    let identity = format!(
        "nominal:{}:{declaration}:1:{}:{argument}",
        declaration.len(),
        argument.len()
    );
    let mut digest = Sha256::new();
    digest.update(b"semaprax.native-record-instance.v1\0");
    digest.update(identity.as_bytes());
    format!(
        "spx_record_{}_inst_{:x}",
        hex_identity(declaration),
        semaprax::digest_hex::LowerHex(digest.finalize())
    )
}

#[test]
fn native_record_patterns_execute_nested_generic_failure_and_poison_at_o0_o2() {
    if !command_available("clang") {
        return;
    }
    let program = parse(SOURCE, Path::new("record-pattern-native.spx")).unwrap();
    let generated = codegen::emit_c(&program).unwrap();
    assert_eq!(generated, codegen::emit_c(&program).unwrap());

    let outer = format!("spx_record_{}", hex_identity("test.outer"));
    let box_i64 = generic_record_symbol("test.box", "i64");
    let box_bool = generic_record_symbol("test.box", "bool");
    assert_ne!(box_i64, box_bool);
    assert!(generated.contains(&format!("struct {outer}")));
    assert!(generated.contains(&format!("struct {box_i64}")));
    assert!(generated.contains(&format!("struct {box_bool}")));

    let function = |id: &str| format!("spx_decl_{}", hex_identity(id));
    let make = function("test.make");
    let fail_make = function("test.fail_make");
    let function_definition = |symbol: &str| {
        let marker = format!("static __attribute__((unused)) spx_status_token {symbol}(");
        let start = generated.rfind(&marker).unwrap();
        let tail = &generated[start..];
        let end = tail[marker.len()..]
            .find("\nstatic __attribute__((unused)) spx_status_token ")
            .map_or(tail.len(), |offset| marker.len() + offset);
        &tail[..end]
    };
    assert_eq!(
        function_definition(&function("test.unpack"))
            .matches(&format!("{make}("))
            .count(),
        1
    );
    assert_eq!(
        function_definition(&function("test.scrutinee_failure"))
            .matches(&format!("{fail_make}("))
            .count(),
        1
    );

    let outer_inner = format!("spx_field_{}", hex_identity("test.outer.inner"));
    let outer_other = format!("spx_field_{}", hex_identity("test.outer.other"));
    let inner_value = format!("spx_field_{}", hex_identity("test.inner.value"));
    let inner_flag = format!("spx_field_{}", hex_identity("test.inner.flag"));
    let box_value = format!("spx_field_{}", hex_identity("test.box.value"));
    let probe = format!(
        r#"
#include <string.h>
static int spx_test_poison(const unsigned char *bytes, size_t length) {{
    for (size_t index = 0; index < length; index += 1) {{
        if (bytes[index] != UINT8_C(165)) return 0;
    }}
    return 1;
}}
static int spx_test_status(
    const struct spx_context *context,
    spx_status_token token,
    const char *domain,
    uint32_t code
) {{
    const struct spx_normalized_status *status = spx_status_resolve(context, token);
    return status != NULL && strcmp(status->domain_id, domain) == 0 && status->code == code;
}}
int main(void) {{
    struct spx_status_entry entries[UINT32_C(32)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(91), entries, UINT32_C(32), NULL, NULL, NULL)) return 10;

    int64_t scalar_output = INT64_C(-1);
    if ({unpack}(&context, INT64_C(20), true, INT64_C(22), &scalar_output) != SPX_STATUS_SUCCESS || scalar_output != INT64_C(42)) return 11;
    if ({unpack}(&context, INT64_C(20), false, INT64_C(22), &scalar_output) != SPX_STATUS_SUCCESS || scalar_output != INT64_C(2)) return 12;

    struct {box_i64} integer = {{0}};
    integer.{box_value} = INT64_C(42);
    if ({read_i64}(&context, &integer, &scalar_output) != SPX_STATUS_SUCCESS || scalar_output != INT64_C(42)) return 13;

    struct {box_bool} flag = {{0}};
    flag.{box_value} = false;
    if ({read_bool}(&context, &flag, &scalar_output) != SPX_STATUS_SUCCESS || scalar_output != INT64_C(0)) return 14;
    flag.{box_value} = true;
    if ({read_bool}(&context, &flag, &scalar_output) != SPX_STATUS_SUCCESS || scalar_output != INT64_C(1)) return 15;

    struct {outer} input = {{0}};
    input.{outer_inner}.{inner_value} = INT64_C(35);
    input.{outer_inner}.{inner_flag} = false;
    input.{outer_other} = INT64_C(7);
    if ({ignore}(&context, &input, &scalar_output) != SPX_STATUS_SUCCESS || scalar_output != INT64_C(7)) return 16;
    input.{outer_inner}.{inner_value} = INT64_C(5);
    input.{outer_inner}.{inner_flag} = true;
    if ({whole_inner}(&context, &input, &scalar_output) != SPX_STATUS_SUCCESS || scalar_output != INT64_C(5)) return 24;
    input.{outer_inner}.{inner_flag} = false;
    if ({whole_inner}(&context, &input, &scalar_output) != SPX_STATUS_SUCCESS || scalar_output != INT64_C(0)) return 25;

    memset(&scalar_output, 0xa5, sizeof(scalar_output));
    spx_status_token status = {scrutinee_failure}(&context, &scalar_output);
    if (!spx_test_status(&context, status, "semaprax.contract.v1", UINT32_C(1))) return 17;
    if (!spx_test_poison((const unsigned char *)&scalar_output, sizeof(scalar_output))) return 18;

    memset(&scalar_output, 0xa5, sizeof(scalar_output));
    status = {arm_failure}(&context, &input, &scalar_output);
    if (!spx_test_status(&context, status, "semaprax.arithmetic.v1", UINT32_C(4))) return 19;
    if (!spx_test_poison((const unsigned char *)&scalar_output, sizeof(scalar_output))) return 20;

    memset(&scalar_output, 0xa5, sizeof(scalar_output));
    status = {post_failure}(&context, &input, &scalar_output);
    if (!spx_test_status(&context, status, "semaprax.contract.v1", UINT32_C(2))) return 21;
    if (!spx_test_poison((const unsigned char *)&scalar_output, sizeof(scalar_output))) return 22;

    if ({app_main}(&context, &scalar_output) != SPX_STATUS_SUCCESS || scalar_output != INT64_C(42)) return 23;
    return 0;
}}
"#,
        unpack = function("test.unpack"),
        read_i64 = function("test.read_i64"),
        read_bool = function("test.read_bool"),
        ignore = function("test.ignore"),
        whole_inner = function("test.whole_inner"),
        scrutinee_failure = function("test.scrutinee_failure"),
        arm_failure = function("test.arm_failure"),
        post_failure = function("test.post_failure"),
        app_main = function("app.main"),
    );

    for optimization in ["-O0", "-O2"] {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!("semaprax-record-pattern-native-{}-{id}", std::process::id());
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
            "record pattern C failed at {optimization}: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let executed = Command::new(&executable).output().unwrap();
        let _ = std::fs::remove_file(&source);
        let _ = std::fs::remove_file(&executable);
        assert!(
            executed.status.success(),
            "record pattern executable failed at {optimization}: status={:?} stderr={}",
            executed.status.code(),
            String::from_utf8_lossy(&executed.stderr)
        );
    }
}

#[test]
fn node_wasm_record_patterns_match_native_success_paths_for_4096_reentries() {
    if !command_available("node") {
        return;
    }
    let program = parse(SOURCE, Path::new("record-pattern-wasm.spx")).unwrap();
    let bytes = wasm::emit_module(&program).unwrap();
    assert_eq!(bytes, wasm::emit_module(&program).unwrap());
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!("semaprax-record-pattern-wasm-{}-{id}", std::process::id());
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
  if (instance.exports.semaprax_main() !== 42n) throw new Error("record pattern result mismatch");
}
console.log("record-pattern-wasm-v1-ok");
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
        "Node record pattern runtime failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "record-pattern-wasm-v1-ok"
    );
}
