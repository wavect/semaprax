use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::{codegen, parse, wasm};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const SOURCE: &str = r#"
module test.aggregate_backends;
@id("inner.type")
record Inner {
    @id("inner.value") value: i64,
    @id("inner.flag") flag: bool,
}
@id("outer.type")
record Outer {
    @id("outer.inner") inner: Inner,
    @id("outer.other") other: i64,
}
@id("case.ok")
fn ok(base: Outer) -> Outer {
    base with { inner: base.inner with { value: base.inner.value + 2 } }
}
@id("case.fail.base")
fn fail_base() -> Outer requires false {
    Outer { inner: Inner { value: 1, flag: true }, other: 2 }
}
@id("case.base.first")
fn base_first() -> Outer {
    fail_base() with { other: 9223372036854775807 + 1 }
}
@id("case.replacements")
fn replacements(base: Outer) -> Outer {
    base with {
        inner: base.inner with { value: 9223372036854775807 + 1 },
        other: 1 / 0,
    }
}
@id("case.post")
fn post(base: Outer) -> Outer ensures false { ok(base) }
@id("app.main")
fn main() -> i64 {
    let value = Outer {
        inner: Inner { value: 18, flag: true },
        other: 22,
    };
    let changed = ok(value);
    if changed.inner.flag { changed.inner.value + changed.other } else { 0 }
}
"#;

fn hex_identity(value: &str) -> String {
    let mut output = String::with_capacity(value.len() * 2);
    for byte in value.bytes() {
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn compiler_available() -> bool {
    Command::new("clang").arg("--version").output().is_ok()
}

#[test]
fn native_aggregate_layout_status_out_and_source_order_execute_at_o0_o2() {
    if !compiler_available() {
        return;
    }
    let program = parse(SOURCE, Path::new("aggregate-native.spx")).unwrap();
    let generated = codegen::emit_c(&program).unwrap();
    assert_eq!(generated, codegen::emit_c(&program).unwrap());

    let outer = format!("spx_record_{}", hex_identity("outer.type"));
    let inner_field = format!("spx_field_{}", hex_identity("outer.inner"));
    let other_field = format!("spx_field_{}", hex_identity("outer.other"));
    let value_field = format!("spx_field_{}", hex_identity("inner.value"));
    let flag_field = format!("spx_field_{}", hex_identity("inner.flag"));
    assert!(generated.contains(&format!(
        "_Static_assert(sizeof(struct {outer}) == UINT32_C(24)"
    )));
    assert!(generated.contains(&format!(
        "_Static_assert(offsetof(struct {outer}, {other_field}) == UINT32_C(16)"
    )));
    assert!(generated.contains(&format!("const struct {outer} *spx_param_0")));

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
    struct {outer} input = {{0}};
    input.{inner_field}.{value_field} = INT64_C(18);
    input.{inner_field}.{flag_field} = true;
    input.{other_field} = INT64_C(22);
    struct {outer} output;
    memset(&output, 0xa5, sizeof(output));
    if ({ok}(&context, &input, &output) != SPX_STATUS_SUCCESS) return 11;
    if (output.{inner_field}.{value_field} != INT64_C(20) ||
        !output.{inner_field}.{flag_field} || output.{other_field} != INT64_C(22)) return 12;

    memset(&output, 0xa5, sizeof(output));
    spx_status_token status = {base_first}(&context, &output);
    if (!spx_test_status(&context, status, "semaprax.contract.v1", UINT32_C(1))) return 13;
    if (!spx_test_poison((const unsigned char *)&output, sizeof(output))) return 14;

    memset(&output, 0xa5, sizeof(output));
    status = {replacements}(&context, &input, &output);
    if (!spx_test_status(&context, status, "semaprax.arithmetic.v1", UINT32_C(1))) return 15;
    if (!spx_test_poison((const unsigned char *)&output, sizeof(output))) return 16;

    memset(&output, 0xa5, sizeof(output));
    status = {post}(&context, &input, &output);
    if (!spx_test_status(&context, status, "semaprax.contract.v1", UINT32_C(2))) return 17;
    if (!spx_test_poison((const unsigned char *)&output, sizeof(output))) return 18;
    return 0;
}}
"#,
        ok = symbol("case.ok"),
        base_first = symbol("case.base.first"),
        replacements = symbol("case.replacements"),
        post = symbol("case.post"),
    );

    for optimization in ["-O0", "-O2"] {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!("semaprax-aggregate-native-{}-{id}", std::process::id());
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
            "aggregate C failed at {optimization}: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let executed = Command::new(&executable).output().unwrap();
        let _ = std::fs::remove_file(&source);
        let _ = std::fs::remove_file(&executable);
        assert!(
            executed.status.success(),
            "aggregate executable failed at {optimization}: status={:?} stderr={}",
            executed.status.code(),
            String::from_utf8_lossy(&executed.stderr)
        );
    }
}

#[test]
fn public_native_and_node_wasm_match_the_executable_nested_record_example() {
    let source = std::fs::read_to_string("examples/records.spx").unwrap();
    let program = parse(&source, Path::new("examples/records.spx")).unwrap();
    let first = wasm::emit_module(&program).unwrap();
    assert_eq!(first, wasm::emit_module(&program).unwrap());
    if !compiler_available() || Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "semaprax-aggregate-equivalence-{}-{id}",
        std::process::id()
    ));
    let native = std::env::temp_dir().join(format!(
        "semaprax-aggregate-equivalence-{}-{id}.native{}",
        std::process::id(),
        std::env::consts::EXE_SUFFIX
    ));
    codegen::build(&program, &native).unwrap();
    let native_output = Command::new(&native).output().unwrap();
    assert!(native_output.status.success());
    assert_eq!(String::from_utf8_lossy(&native_output.stdout).trim(), "42");

    wasm::build_web(&program, &root).unwrap();
    let node = Command::new("node")
        .arg("scripts/verify-web.mjs")
        .arg(&root)
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&native);
    let _ = std::fs::remove_dir_all(&root);
    assert!(
        node.status.success(),
        "Node aggregate example failed: {}",
        String::from_utf8_lossy(&node.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&node.stdout).trim(), "42");
}
