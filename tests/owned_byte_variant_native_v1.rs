use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::{codegen, parse};

static SERIAL: AtomicU64 = AtomicU64::new(0);

const SOURCE: &str = include_str!("owned_byte_variant_v1_fixture.spx");

fn symbol(id: &str) -> String {
    use std::fmt::Write as _;
    let mut hex = String::with_capacity(id.len() * 2);
    for byte in id.bytes() {
        write!(hex, "{byte:02x}").unwrap();
    }
    format!("spx_decl_{hex}")
}

fn variant_symbol(id: &str) -> String {
    symbol(id).replacen("spx_decl_", "spx_variant_", 1)
}

fn compile_and_run(source: &str, probe: &str, optimization: &str) {
    let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
    let stem = format!(
        "semaprax-owned-variant-native-{}-{serial}",
        std::process::id()
    );
    let c_path = std::env::temp_dir().join(format!("{stem}.c"));
    let executable = std::env::temp_dir().join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
    std::fs::write(&c_path, format!("{source}\n{probe}")).unwrap();
    let compiled = Command::new("clang")
        .args([
            "-std=c11",
            optimization,
            "-Wall",
            "-Wextra",
            "-Werror",
            "-DSPX_NO_ENTRY_WRAPPER",
        ])
        .arg(&c_path)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let executed = Command::new(&executable).output().unwrap();
    let _ = std::fs::remove_file(c_path);
    let _ = std::fs::remove_file(executable);
    assert!(executed.status.success(), "probe failed: {executed:?}");
}

fn compile_and_expect_failure(source: &str, probe: &str, optimization: &str) {
    let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
    let stem = format!(
        "semaprax-owned-variant-native-invalid-{}-{serial}",
        std::process::id()
    );
    let c_path = std::env::temp_dir().join(format!("{stem}.c"));
    let executable = std::env::temp_dir().join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
    std::fs::write(&c_path, format!("{source}\n{probe}")).unwrap();
    let compiled = Command::new("clang")
        .args([
            "-std=c11",
            optimization,
            "-Wall",
            "-Wextra",
            "-Werror",
            "-DSPX_NO_ENTRY_WRAPPER",
        ])
        .arg(&c_path)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let executed = Command::new(&executable).output().unwrap();
    let _ = std::fs::remove_file(c_path);
    let _ = std::fs::remove_file(executable);
    assert!(
        !executed.status.success(),
        "invalid owned carrier returned instead of terminating"
    );
}

#[test]
fn owned_byte_variants_move_and_settle_exactly_at_o0_and_o2() {
    assert_native_variant_corpus(SOURCE);
}

#[test]
fn variant_field_places_branches_and_blocks_follow_exact_cleanup_transfers() {
    let original = "Choice::Data { payload: bytes_copy(data), marker: 20 }";
    assert_eq!(SOURCE.matches(original).count(), 1);
    for replacement in [
        "let staged = bytes_copy(data); Choice::Data { payload: staged, marker: 20 }",
        "Choice::Data { payload: if true { bytes_copy(data) } else { bytes_copy(data) }, marker: 20 }",
        "Choice::Data { payload: { let staged = bytes_copy(data); staged }, marker: 20 }",
    ] {
        assert_native_variant_corpus(&SOURCE.replace(original, replacement));
    }
}

fn assert_native_variant_corpus(source: &str) {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let program = parse(source, Path::new("owned-byte-variant-native-v1.spx")).unwrap();
    let generated = codegen::emit_c(&program).unwrap();
    assert_eq!(generated, codegen::emit_c(&program).unwrap());
    assert!(generated.contains("spx_bytes_move"));
    assert!(generated.contains("invalid variant tag"));

    let tracked = format!(
        "#include <stddef.h>\nstatic void *spx_test_malloc(size_t);\nstatic void spx_test_free(void *);\n{}",
        generated
            .replace(
                "malloc((size_t)value.len)",
                "spx_test_malloc((size_t)value.len)",
            )
            .replace("free(value->ptr)", "spx_test_free(value->ptr)")
    );
    let probe = format!(
        r#"
static void *spx_allocations[UINT32_C(64)];
static uint32_t spx_allocation_count = UINT32_C(0);
static uint32_t spx_free_count = UINT32_C(0);
static void *spx_test_malloc(size_t size) {{
    void *value = malloc(size);
    if (value == NULL || spx_allocation_count >= UINT32_C(64)) abort();
    spx_allocations[spx_allocation_count++] = value;
    return value;
}}
static void spx_test_free(void *value) {{
    bool found = false;
    for (uint32_t index = UINT32_C(0); index < spx_allocation_count; ++index) {{
        if (spx_allocations[index] == value) {{
            if (found) abort();
            found = true;
            spx_allocations[index] = NULL;
        }}
    }}
    if (!found) abort();
    ++spx_free_count;
    free(value);
}}
int main(void) {{
    struct spx_status_entry entries[UINT32_C(32)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(170), entries, UINT32_C(32), NULL, NULL, NULL)) return 10;
    uint8_t failure_bytes[UINT32_C(2)] = {{UINT8_C(7), UINT8_C(9)}};
    spx_slice_u8_v1 failure_input = {{ .ptr = failure_bytes, .len = UINT64_C(2) }};
    for (uint32_t index = UINT32_C(0); index < UINT32_C(4); ++index) {{
        int64_t result = INT64_C(0);
        if ({main}(&context, &result) != SPX_STATUS_SUCCESS || result != INT64_C(132)) return 11;
        if ({failure}(&context, failure_input, &result) == SPX_STATUS_SUCCESS) return 13;
        if ({post_commit_failure}(&context, failure_input, &result) == SPX_STATUS_SUCCESS) return 14;
        if ({match_failure}(&context, failure_input, &result) == SPX_STATUS_SUCCESS) return 15;
    }}
    return spx_allocation_count == UINT32_C(48) && spx_free_count == UINT32_C(48) ? 0 : 12;
}}
"#,
        main = symbol("app.main"),
        failure = symbol("sum.fail-after-owned"),
        post_commit_failure = symbol("sum.fail-after-call-commit"),
        match_failure = symbol("sum.fail-in-owned-match"),
    );
    for optimization in ["-O0", "-O2"] {
        compile_and_run(&tracked, &probe, optimization);
        let invalid_probe = format!(
            r#"
int main(void) {{
    struct spx_status_entry entries[UINT32_C(8)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(19), entries, UINT32_C(8), NULL, NULL, NULL)) return 10;
    struct {choice} invalid = {{0}};
    invalid.spx_tag = UINT32_MAX;
    int64_t result = INT64_C(0x5a5a5a5a);
    (void){consume}(&context, &invalid, &result);
    return 0;
}}
"#,
            choice = variant_symbol("sum.choice"),
            consume = symbol("sum.consume"),
        );
        compile_and_expect_failure(&generated, &invalid_probe, optimization);
    }
}
