use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::{codegen, parse, wasm};
use sha2::{Digest as _, Sha256};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const SOURCE: &str = r#"
module test.executable_try;

@id("try.source_i64")
fn source_i64(residual: bool, value: i64) -> Result<i64, bool> {
    if residual {
        Result<i64, bool>::Err { error: true }
    } else {
        Result<i64, bool>::Ok { value: value }
    }
}

@id("try.source_bool")
fn source_bool(residual: bool, value: bool) -> Result<bool, bool> {
    if residual {
        Result<bool, bool>::Err { error: true }
    } else {
        Result<bool, bool>::Ok { value: value }
    }
}

@id("try.large_to_small")
fn large_to_small(residual: bool, value: i64) -> Result<bool, bool>
    ensures match result {
        Result::Ok { value: success } => success,
        Result::Err { error: failure } => failure,
    }
{
    let number = source_i64(residual, value)?;
    Result<bool, bool>::Ok { value: number > 0 }
}

@id("try.small_to_large")
fn small_to_large(residual: bool, value: bool) -> Result<i64, bool>
    ensures match result {
        Result::Ok { value: success } => success == 0 || success == 1,
        Result::Err { error: failure } => failure,
    }
{
    let flag = source_bool(residual, value)?;
    Result<i64, bool>::Ok { value: if flag { 1 } else { 0 } }
}

@id("try.post_err")
fn post_err() -> Result<bool, bool> ensures false {
    let number = source_i64(true, 7)?;
    Result<bool, bool>::Ok { value: number > 0 }
}

@id("try.physical")
fn physical() -> Result<i64, bool> requires false {
    Result<i64, bool>::Err { error: true }
}

@id("try.physical_then_post")
fn physical_then_post() -> Result<bool, bool> ensures false {
    let number = physical()?;
    Result<bool, bool>::Ok { value: number > 0 }
}

@id("try.err_skips_later")
fn err_skips_later() -> Result<bool, bool> {
    let number = source_i64(true, 7)?;
    Result<bool, bool>::Ok { value: number + 9223372036854775807 > 0 }
}

@id("try.from_input")
fn from_input(value: Result<i64, bool>) -> Result<bool, bool> {
    let number = value?;
    Result<bool, bool>::Ok { value: number > 0 }
}

@id("app.main")
fn main() -> i64 {
    let large = large_to_small(false, 42);
    let small = small_to_large(true, true);
    let left = match large {
        Result::Ok { value: success } => if success { 40 } else { 0 },
        Result::Err { error: failure } => if failure { 1 } else { 0 },
    };
    let right = match small {
        Result::Ok { value: success } => success,
        Result::Err { error: failure } => if failure { 2 } else { 0 },
    };
    left + right
}
"#;

const OPTION_SOURCE: &str = r#"
module test.executable_option_try;

@id("option.source_i64")
fn source_i64(absent: bool, value: i64) -> Option<i64> {
    if absent {
        Option<i64>::None {}
    } else {
        Option<i64>::Some { value: value }
    }
}

@id("option.source_bool")
fn source_bool(absent: bool, value: bool) -> Option<bool> {
    if absent {
        Option<bool>::None {}
    } else {
        Option<bool>::Some { value: value }
    }
}

@id("option.large_to_small")
fn large_to_small(absent: bool, value: i64) -> Option<bool>
    ensures match result {
        Option::Some { value: success } => success,
        Option::None {} => true,
    }
{
    let number = source_i64(absent, value)?;
    Option<bool>::Some { value: number > 0 }
}

@id("option.small_to_large")
fn small_to_large(absent: bool, value: bool) -> Option<i64>
    ensures match result {
        Option::Some { value: success } => success == 0 || success == 1,
        Option::None {} => true,
    }
{
    let flag = source_bool(absent, value)?;
    Option<i64>::Some { value: if flag { 1 } else { 0 } }
}

@id("option.post_none")
fn post_none() -> Option<bool> ensures false {
    let number = source_i64(true, 7)?;
    Option<bool>::Some { value: number > 0 }
}

@id("option.physical")
fn physical() -> Option<i64> requires false {
    Option<i64>::None {}
}

@id("option.physical_then_post")
fn physical_then_post() -> Option<bool> ensures false {
    let number = physical()?;
    Option<bool>::Some { value: number > 0 }
}

@id("option.none_skips_later")
fn none_skips_later() -> Option<bool> {
    let number = source_i64(true, 7)?;
    Option<bool>::Some { value: number / 0 > 0 }
}

@id("option.from_input")
fn from_input(value: Option<i64>) -> Option<bool> {
    let number = value?;
    Option<bool>::Some { value: number > 0 }
}

@id("app.main")
fn main() -> i64 {
    let large = large_to_small(false, 42);
    let small = small_to_large(true, true);
    let left = match large {
        Option::Some { value: success } => if success { 40 } else { 0 },
        Option::None {} => 1,
    };
    let right = match small {
        Option::Some { value: success } => success,
        Option::None {} => 2,
    };
    left + right
}
"#;

fn hex_identity(value: &str) -> String {
    let mut output = String::with_capacity(value.len() * 2);
    for byte in value.bytes() {
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn generic_variant_symbol(id: &str, arguments: &[&str]) -> String {
    let mut encoded = String::new();
    for argument in arguments {
        write!(encoded, "{}:{argument}", argument.len()).expect("writing to String cannot fail");
    }
    let identity = format!("nominal:{}:{id}:{}:{encoded}", id.len(), arguments.len());
    let mut digest = Sha256::new();
    digest.update(b"semaprax.native-variant-instance.v1\0");
    digest.update(identity.as_bytes());
    let mut suffix = String::with_capacity(64);
    for byte in digest.finalize() {
        write!(suffix, "{byte:02x}").expect("writing to String cannot fail");
    }
    format!("spx_variant_{}_inst_{suffix}", hex_identity(id))
}

fn command_available(command: &str) -> bool {
    Command::new(command).arg("--version").output().is_ok()
}

#[test]
fn native_result_try_reconstructs_outer_layout_and_preserves_poison_at_o0_o2() {
    if !command_available("clang") {
        return;
    }
    let program = parse(SOURCE, Path::new("executable-try-native.spx")).unwrap();
    let generated = codegen::emit_c(&program).unwrap();
    assert_eq!(generated, codegen::emit_c(&program).unwrap());
    assert!(generated.contains("goto spx_postconditions;"));
    assert!(!generated.contains("memcpy(&spx_result"));

    let result_i64 = generic_variant_symbol("core.result", &["i64", "bool"]);
    let result_bool = generic_variant_symbol("core.result", &["bool", "bool"]);
    let err_case = format!("spx_case_{}", hex_identity("core.result.err"));
    let err_field = format!("spx_field_{}", hex_identity("core.result.err.error"));
    let ok_case = format!("spx_case_{}", hex_identity("core.result.ok"));
    let ok_field = format!("spx_field_{}", hex_identity("core.result.ok.value"));
    let symbol = |id: &str| format!("spx_decl_{}", hex_identity(id));
    let probe = format!(
        r#"
#include <string.h>
static int spx_test_poison(const unsigned char *bytes, size_t length) {{
    for (size_t index = 0; index < length; index += 1) {{
        if (bytes[index] != UINT8_C(165)) return 0;
    }}
    return 1;
}}
static int spx_test_status(const struct spx_context *context, spx_status_token token, const char *domain, uint32_t code) {{
    const struct spx_normalized_status *status = spx_status_resolve(context, token);
    return status != NULL && strcmp(status->domain_id, domain) == 0 && status->code == code;
}}
int main(int argc, char **argv) {{
    struct spx_status_entry entries[UINT32_C(32)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(101), entries, UINT32_C(32), NULL, NULL, NULL)) return 10;
    struct {result_i64} large;
    struct {result_bool} small;
    if (argc > 1) {{
        (void)argv;
        memset(&large, 0, sizeof(large));
        large.spx_tag = UINT32_MAX;
        memset(&small, 0xa5, sizeof(small));
        (void){from_input}(&context, &large, &small);
        return 90;
    }}

    memset(&small, 0xa5, sizeof(small));
    if ({large_to_small}(&context, false, INT64_C(42), &small) != SPX_STATUS_SUCCESS) return 11;
    struct {result_bool} expected_small = {{0}};
    expected_small.spx_tag = UINT32_C(0);
    expected_small.spx_payload.{ok_case}.{ok_field} = true;
    if (memcmp(&small, &expected_small, sizeof(small)) != 0) return 12;

    memset(&small, 0xa5, sizeof(small));
    if ({large_to_small}(&context, true, INT64_C(42), &small) != SPX_STATUS_SUCCESS) return 13;
    memset(&expected_small, 0, sizeof(expected_small));
    expected_small.spx_tag = UINT32_C(1);
    expected_small.spx_payload.{err_case}.{err_field} = true;
    if (memcmp(&small, &expected_small, sizeof(small)) != 0) return 14;

    memset(&large, 0xa5, sizeof(large));
    if ({small_to_large}(&context, false, true, &large) != SPX_STATUS_SUCCESS) return 15;
    struct {result_i64} expected_large = {{0}};
    expected_large.spx_tag = UINT32_C(0);
    expected_large.spx_payload.{ok_case}.{ok_field} = INT64_C(1);
    if (memcmp(&large, &expected_large, sizeof(large)) != 0) return 16;

    memset(&large, 0xa5, sizeof(large));
    if ({small_to_large}(&context, true, true, &large) != SPX_STATUS_SUCCESS) return 17;
    memset(&expected_large, 0, sizeof(expected_large));
    expected_large.spx_tag = UINT32_C(1);
    expected_large.spx_payload.{err_case}.{err_field} = true;
    if (memcmp(&large, &expected_large, sizeof(large)) != 0) return 18;

    memset(&small, 0xa5, sizeof(small));
    spx_status_token status = {post_err}(&context, &small);
    if (!spx_test_status(&context, status, "semaprax.contract.v1", UINT32_C(2)) ||
        !spx_test_poison((const unsigned char *)&small, sizeof(small))) return 19;

    memset(&small, 0xa5, sizeof(small));
    status = {physical_then_post}(&context, &small);
    if (!spx_test_status(&context, status, "semaprax.contract.v1", UINT32_C(1)) ||
        !spx_test_poison((const unsigned char *)&small, sizeof(small))) return 20;

    memset(&small, 0xa5, sizeof(small));
    if ({err_skips_later}(&context, &small) != SPX_STATUS_SUCCESS) return 21;
    if (memcmp(&small, &expected_small, sizeof(small)) != 0) return 22;

    int64_t public_result = INT64_C(0);
    if ({main_fn}(&context, &public_result) != SPX_STATUS_SUCCESS || public_result != INT64_C(42)) return 23;
    return 0;
}}
"#,
        large_to_small = symbol("try.large_to_small"),
        small_to_large = symbol("try.small_to_large"),
        post_err = symbol("try.post_err"),
        physical_then_post = symbol("try.physical_then_post"),
        err_skips_later = symbol("try.err_skips_later"),
        from_input = symbol("try.from_input"),
        main_fn = symbol("app.main"),
    );

    for optimization in ["-O0", "-O2"] {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!("semaprax-try-native-{}-{id}", std::process::id());
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
            "typed ? C failed at {optimization}: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let executed = Command::new(&executable).output().unwrap();
        let invalid = Command::new(&executable)
            .arg("invalid-tag")
            .output()
            .unwrap();
        let _ = std::fs::remove_file(&source);
        let _ = std::fs::remove_file(&executable);
        assert!(
            executed.status.success(),
            "typed ? C failed at {optimization}: status={:?} stderr={}",
            executed.status.code(),
            String::from_utf8_lossy(&executed.stderr)
        );
        assert!(
            !invalid.status.success(),
            "invalid Result tag did not fail closed at {optimization}"
        );
        assert!(String::from_utf8_lossy(&invalid.stderr).contains("invalid variant tag"));
    }
}

#[test]
fn public_result_try_is_equivalent_in_node_wasm_with_reentry() {
    if !command_available("node") {
        return;
    }
    let program = parse(SOURCE, Path::new("executable-try-wasm.spx")).unwrap();
    let bytes = wasm::emit_module(&program).unwrap();
    assert_eq!(bytes, wasm::emit_module(&program).unwrap());
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!("semaprax-try-wasm-{}-{id}", std::process::id());
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
  if (instance.exports.semaprax_main() !== 42n) throw new Error("typed ? backend result mismatch");
}
console.log("result-try-wasm-v1-ok");
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
        "Node typed ? runtime failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "result-try-wasm-v1-ok"
    );
}

#[test]
fn native_option_try_reconstructs_none_and_preserves_status_poison_at_o0_o2() {
    if !command_available("clang") {
        return;
    }
    let program = parse(OPTION_SOURCE, Path::new("executable-option-try-native.spx")).unwrap();
    let generated = codegen::emit_c(&program).unwrap();
    assert_eq!(generated, codegen::emit_c(&program).unwrap());
    assert!(generated.contains("goto spx_postconditions;"));
    assert!(!generated.contains("memcpy(&spx_result"));

    let option_i64 = generic_variant_symbol("core.option", &["i64"]);
    let option_bool = generic_variant_symbol("core.option", &["bool"]);
    let some_case = format!("spx_case_{}", hex_identity("core.option.some"));
    let some_field = format!("spx_field_{}", hex_identity("core.option.some.value"));
    let symbol = |id: &str| format!("spx_decl_{}", hex_identity(id));
    let probe = format!(
        r#"
#include <string.h>
static int spx_test_poison(const unsigned char *bytes, size_t length) {{
    for (size_t index = 0; index < length; index += 1) {{
        if (bytes[index] != UINT8_C(165)) return 0;
    }}
    return 1;
}}
static int spx_test_status(const struct spx_context *context, spx_status_token token, const char *domain, uint32_t code) {{
    const struct spx_normalized_status *status = spx_status_resolve(context, token);
    return status != NULL && strcmp(status->domain_id, domain) == 0 && status->code == code;
}}
int main(int argc, char **argv) {{
    struct spx_status_entry entries[UINT32_C(32)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(102), entries, UINT32_C(32), NULL, NULL, NULL)) return 10;
    struct {option_i64} large;
    struct {option_bool} small;
    if (argc > 1) {{
        (void)argv;
        memset(&large, 0, sizeof(large));
        large.spx_tag = UINT32_MAX;
        memset(&small, 0xa5, sizeof(small));
        (void){from_input}(&context, &large, &small);
        return 90;
    }}

    memset(&small, 0xa5, sizeof(small));
    if ({large_to_small}(&context, false, INT64_C(42), &small) != SPX_STATUS_SUCCESS) return 11;
    struct {option_bool} expected_small = {{0}};
    expected_small.spx_tag = UINT32_C(1);
    expected_small.spx_payload.{some_case}.{some_field} = true;
    if (memcmp(&small, &expected_small, sizeof(small)) != 0) return 12;

    memset(&small, 0xa5, sizeof(small));
    if ({large_to_small}(&context, true, INT64_C(42), &small) != SPX_STATUS_SUCCESS) return 13;
    memset(&expected_small, 0, sizeof(expected_small));
    expected_small.spx_tag = UINT32_C(0);
    if (memcmp(&small, &expected_small, sizeof(small)) != 0) return 14;

    memset(&large, 0xa5, sizeof(large));
    if ({small_to_large}(&context, false, true, &large) != SPX_STATUS_SUCCESS) return 15;
    struct {option_i64} expected_large = {{0}};
    expected_large.spx_tag = UINT32_C(1);
    expected_large.spx_payload.{some_case}.{some_field} = INT64_C(1);
    if (memcmp(&large, &expected_large, sizeof(large)) != 0) return 16;

    memset(&large, 0xa5, sizeof(large));
    if ({small_to_large}(&context, true, true, &large) != SPX_STATUS_SUCCESS) return 17;
    memset(&expected_large, 0, sizeof(expected_large));
    expected_large.spx_tag = UINT32_C(0);
    if (memcmp(&large, &expected_large, sizeof(large)) != 0) return 18;

    memset(&small, 0xa5, sizeof(small));
    spx_status_token status = {post_none}(&context, &small);
    if (!spx_test_status(&context, status, "semaprax.contract.v1", UINT32_C(2)) ||
        !spx_test_poison((const unsigned char *)&small, sizeof(small))) return 19;

    memset(&small, 0xa5, sizeof(small));
    status = {physical_then_post}(&context, &small);
    if (!spx_test_status(&context, status, "semaprax.contract.v1", UINT32_C(1)) ||
        !spx_test_poison((const unsigned char *)&small, sizeof(small))) return 20;

    memset(&small, 0xa5, sizeof(small));
    if ({none_skips_later}(&context, &small) != SPX_STATUS_SUCCESS) return 21;
    if (memcmp(&small, &expected_small, sizeof(small)) != 0) return 22;

    int64_t public_result = INT64_C(0);
    if ({main_fn}(&context, &public_result) != SPX_STATUS_SUCCESS || public_result != INT64_C(42)) return 23;
    return 0;
}}
"#,
        large_to_small = symbol("option.large_to_small"),
        small_to_large = symbol("option.small_to_large"),
        post_none = symbol("option.post_none"),
        physical_then_post = symbol("option.physical_then_post"),
        none_skips_later = symbol("option.none_skips_later"),
        from_input = symbol("option.from_input"),
        main_fn = symbol("app.main"),
    );

    for optimization in ["-O0", "-O2"] {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!("semaprax-option-try-native-{}-{id}", std::process::id());
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
            "Option ? C failed at {optimization}: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let executed = Command::new(&executable).output().unwrap();
        let invalid = Command::new(&executable)
            .arg("invalid-tag")
            .output()
            .unwrap();
        let _ = std::fs::remove_file(&source);
        let _ = std::fs::remove_file(&executable);
        assert!(
            executed.status.success(),
            "Option ? C failed at {optimization}: status={:?} stderr={}",
            executed.status.code(),
            String::from_utf8_lossy(&executed.stderr)
        );
        assert!(
            !invalid.status.success(),
            "invalid Option tag did not fail closed at {optimization}"
        );
        assert!(String::from_utf8_lossy(&invalid.stderr).contains("invalid variant tag"));
    }
}

#[test]
fn public_option_try_is_equivalent_in_node_wasm_with_reentry() {
    if !command_available("node") {
        return;
    }
    let program = parse(OPTION_SOURCE, Path::new("executable-option-try-wasm.spx")).unwrap();
    let bytes = wasm::emit_module(&program).unwrap();
    assert_eq!(bytes, wasm::emit_module(&program).unwrap());
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!("semaprax-option-try-wasm-{}-{id}", std::process::id());
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
  if (instance.exports.semaprax_main() !== 42n) throw new Error("Option ? backend result mismatch");
}
console.log("option-try-wasm-v1-ok");
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
        "Node Option ? runtime failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "option-try-wasm-v1-ok"
    );
}
