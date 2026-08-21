use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::{codegen, parse, wasm};
use sha2::{Digest as _, Sha256};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const SOURCE: &str = r#"
module test.executable_generic_records;

@id("test.box")
record Box<T> { @id("test.box.value") value: T, }

@id("test.pair")
record Pair<T> {
    @id("test.pair.left") left: T,
    @id("test.pair.right") right: T,
}

@id("test.duo")
record Duo<T, U> {
    @id("test.duo.left") left: T,
    @id("test.duo.right") right: U,
}

@id("test.box_i64")
fn box_i64(value: i64) -> Box<i64> { Box<i64> { value: value } }

@id("test.bump")
fn bump(boxed: Box<i64>) -> Box<i64> {
    boxed with { value: boxed.value + 1 }
}

@id("test.read_bool")
fn read_bool(boxed: Box<bool>) -> i64 {
    if boxed.value { 1 } else { 0 }
}

@id("test.sum_pair")
fn sum_pair(pair: Pair<i64>) -> i64 { pair.left + pair.right }

@id("test.duo_value")
fn duo_value(value: i64, flag: bool) -> Duo<i64, bool> {
    Duo<i64, bool> { left: value, right: flag }
}

@id("test.read_duo")
fn read_duo(value: Duo<i64, bool>) -> i64 {
    if value.right { value.left } else { 0 }
}

@id("test.fail_base")
fn fail_base() -> Box<i64> requires false { Box<i64> { value: 1 } }

@id("test.base_first")
fn base_first() -> Box<i64> {
    fail_base() with { value: 9223372036854775807 + 1 }
}

@id("test.replacements")
fn replacements(pair: Pair<i64>) -> Pair<i64> {
    pair with {
        left: 9223372036854775807 + 1,
        right: 1 / 0,
    }
}

@id("test.post")
fn post(boxed: Box<i64>) -> Box<i64> ensures false { bump(boxed) }

@id("app.main")
fn main() -> i64 {
    let first = bump(box_i64(19));
    let truth = Box<bool> { value: true };
    let falsity = Box<bool> { value: false };
    let pair = Pair<i64> { left: first.value, right: 21 };
    let duo = duo_value(1, true);
    sum_pair(pair) + read_bool(truth) + read_bool(falsity) + read_duo(duo) - 1
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

fn record_symbol(declaration: &str, argument: &str) -> String {
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
fn native_generic_records_preserve_instance_layout_order_and_poison_at_o0_o2() {
    if !command_available("clang") {
        return;
    }
    let program = parse(SOURCE, Path::new("generic-record-native.spx")).unwrap();
    let generated = codegen::emit_c(&program).unwrap();
    assert_eq!(generated, codegen::emit_c(&program).unwrap());

    let box_i64 = record_symbol("test.box", "i64");
    let box_bool = record_symbol("test.box", "bool");
    let pair_i64 = record_symbol("test.pair", "i64");
    assert_ne!(box_i64, box_bool);
    assert!(generated.contains(&format!("struct {box_i64}")));
    assert!(generated.contains(&format!("struct {box_bool}")));
    assert!(generated.contains(&format!("struct {pair_i64}")));

    let function = |id: &str| format!("spx_decl_{}", hex_identity(id));
    let pair_left = format!("spx_field_{}", hex_identity("test.pair.left"));
    let pair_right = format!("spx_field_{}", hex_identity("test.pair.right"));
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
    struct spx_status_entry entries[UINT32_C(16)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(77), entries, UINT32_C(16), NULL, NULL, NULL)) return 10;

    struct {box_i64} input = {{0}};
    input.{box_value} = INT64_C(41);
    struct {box_i64} box_output;
    memset(&box_output, 0xa5, sizeof(box_output));
    if ({bump}(&context, &input, &box_output) != SPX_STATUS_SUCCESS) return 11;
    if (box_output.{box_value} != INT64_C(42)) return 12;

    struct {box_bool} flag = {{0}};
    int64_t scalar_output = INT64_C(-1);
    flag.{box_value} = false;
    if ({read_bool}(&context, &flag, &scalar_output) != SPX_STATUS_SUCCESS || scalar_output != INT64_C(0)) return 19;
    flag.{box_value} = true;
    if ({read_bool}(&context, &flag, &scalar_output) != SPX_STATUS_SUCCESS || scalar_output != INT64_C(1)) return 20;
    if ({app_main}(&context, &scalar_output) != SPX_STATUS_SUCCESS || scalar_output != INT64_C(42)) return 21;

    memset(&box_output, 0xa5, sizeof(box_output));
    spx_status_token status = {base_first}(&context, &box_output);
    if (!spx_test_status(&context, status, "semaprax.contract.v1", UINT32_C(1))) return 13;
    if (!spx_test_poison((const unsigned char *)&box_output, sizeof(box_output))) return 14;

    struct {pair_i64} pair = {{0}};
    pair.{pair_left} = INT64_C(20);
    pair.{pair_right} = INT64_C(22);
    struct {pair_i64} pair_output;
    memset(&pair_output, 0xa5, sizeof(pair_output));
    status = {replacements}(&context, &pair, &pair_output);
    if (!spx_test_status(&context, status, "semaprax.arithmetic.v1", UINT32_C(1))) return 15;
    if (!spx_test_poison((const unsigned char *)&pair_output, sizeof(pair_output))) return 16;

    memset(&box_output, 0xa5, sizeof(box_output));
    status = {post}(&context, &input, &box_output);
    if (!spx_test_status(&context, status, "semaprax.contract.v1", UINT32_C(2))) return 17;
    if (!spx_test_poison((const unsigned char *)&box_output, sizeof(box_output))) return 18;
    return 0;
}}
"#,
        bump = function("test.bump"),
        read_bool = function("test.read_bool"),
        app_main = function("app.main"),
        base_first = function("test.base_first"),
        replacements = function("test.replacements"),
        post = function("test.post"),
    );

    for optimization in ["-O0", "-O2"] {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!("semaprax-generic-record-native-{}-{id}", std::process::id());
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
            "generic record C failed at {optimization}: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let executed = Command::new(&executable).output().unwrap();
        let _ = std::fs::remove_file(&source);
        let _ = std::fs::remove_file(&executable);
        assert!(
            executed.status.success(),
            "generic record executable failed at {optimization}: status={:?} stderr={}",
            executed.status.code(),
            String::from_utf8_lossy(&executed.stderr)
        );
    }
}

#[test]
fn public_generic_records_are_equivalent_in_node_wasm_with_reentry() {
    if !command_available("node") {
        return;
    }
    let program = parse(SOURCE, Path::new("generic-record-wasm.spx")).unwrap();
    let bytes = wasm::emit_module(&program).unwrap();
    assert_eq!(bytes, wasm::emit_module(&program).unwrap());
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!("semaprax-generic-record-wasm-{}-{id}", std::process::id());
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
  if (instance.exports.semaprax_main() !== 42n) throw new Error("generic record result mismatch");
}
console.log("generic-record-wasm-v1-ok");
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
        "Node generic record runtime failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "generic-record-wasm-v1-ok"
    );
}
