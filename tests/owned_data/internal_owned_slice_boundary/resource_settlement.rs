//! Native allocation accounting for an internally borrowed view of owned Bytes.
use semaprax::{codegen, hir, verify};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn symbol(id: &str) -> String {
    let mut result = String::from("spx_decl_");
    for byte in id.bytes() {
        use std::fmt::Write as _;
        write!(result, "{byte:02x}").unwrap();
    }
    result
}

#[test]
fn internal_owned_slice_calls_settle_physical_allocations_at_o0_and_o2() {
    assert!(Command::new("clang").arg("--version").output().is_ok());
    let source = format!(
        "{}\n{}",
        super::SOURCE,
        r#"@id("probe.failure") fn failure() -> i64 ensures false {
    let buffer = bytes_zeroed(131072usize);
    inspect(bytes_as_slice(buffer))
}"#
    );
    let ast = semaprax::check(&source, "internal-owned-slice-resource-settlement.spx").unwrap();
    let diagnostics = verify::verify(&ast);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.severity.is_error()),
        "{diagnostics:?}"
    );
    let resolved = hir::resolve(&ast).unwrap();
    hir::validate(&resolved).unwrap();
    let generated = codegen::emit_hir_c(&resolved).unwrap();
    let tracked = generated
        .replace("malloc(", "spx_test_malloc(")
        .replace("calloc(", "spx_test_calloc(")
        .replace("realloc(", "spx_test_realloc(")
        .replace(
            "#define SPX_VEC_REALLOC realloc",
            "#define SPX_VEC_REALLOC spx_test_realloc",
        )
        .replace("free(", "spx_test_free(");
    assert!(generated.contains("calloc("));
    assert!(generated.contains("free("));
    assert!(tracked.contains("spx_test_calloc("));
    assert!(tracked.contains("spx_test_free("));
    let declarations = r#"
#include <stdint.h>
#include <stdlib.h>
static __attribute__((unused)) void *spx_test_malloc(size_t);
static __attribute__((unused)) void *spx_test_calloc(size_t, size_t);
static __attribute__((unused)) void *spx_test_realloc(void *, size_t);
static __attribute__((unused)) void spx_test_free(void *);
"#;
    let allocator = r#"
static uint64_t spx_test_allocations = UINT64_C(0);
static uint64_t spx_test_live_allocations = UINT64_C(0);
static uint64_t spx_test_frees = UINT64_C(0);
static __attribute__((unused)) void *spx_test_malloc(size_t size) {
    void *value = malloc(size);
    if (value != NULL) { ++spx_test_allocations; ++spx_test_live_allocations; }
    return value;
}
static __attribute__((unused)) void *spx_test_calloc(size_t count, size_t size) {
    void *value = calloc(count, size);
    if (value != NULL) { ++spx_test_allocations; ++spx_test_live_allocations; }
    return value;
}
static __attribute__((unused)) void *spx_test_realloc(void *old, size_t size) {
    int was_null = old == NULL;
    void *value = realloc(old, size);
    if (value != NULL && was_null) {
        ++spx_test_allocations;
        ++spx_test_live_allocations;
    }
    return value;
}
static __attribute__((unused)) void spx_test_free(void *value) {
    if (value != NULL) {
        if (spx_test_live_allocations == UINT64_C(0)) abort();
        ++spx_test_frees;
        --spx_test_live_allocations;
        free(value);
    }
}
"#;
    let probe = format!(
        r#"
int main(void) {{
    struct spx_status_entry entries[UINT32_C(32)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(401), entries, UINT32_C(32), NULL, NULL, NULL)) return 10;
    for (uint32_t iteration = UINT32_C(0); iteration < UINT32_C(3); ++iteration) {{
        uint64_t before = spx_test_allocations;
        uint64_t frees_before = spx_test_frees;
        int64_t result = INT64_C(0);
        if ({main}(&context, &result) != SPX_STATUS_SUCCESS || result != INT64_C(9)) return 11;
        if (spx_test_allocations <= before || spx_test_frees <= frees_before
            || spx_test_live_allocations != UINT64_C(0)) return 12;
        if (context.call_depth != UINT32_C(0) || context.borrowed_str_depth != UINT32_C(0)) return 13;

        before = spx_test_allocations;
        frees_before = spx_test_frees;
        result = INT64_C(0x2525252525252525);
        spx_status_token status = {failure}(&context, &result);
        if (status == SPX_STATUS_SUCCESS) return 14;
        const struct spx_status_entry *selected = spx_status_resolve(&context, status);
        if (selected == NULL || strcmp(selected->domain_id, "semaprax.contract.v1") != 0
            || selected->code != UINT32_C(2)) return 15;
        if (spx_test_allocations <= before || spx_test_frees <= frees_before
            || spx_test_live_allocations != UINT64_C(0)) return 16;
        if (context.call_depth != UINT32_C(0) || context.borrowed_str_depth != UINT32_C(0)) return 17;
    }}
    return spx_test_allocations == spx_test_frees && spx_test_live_allocations == UINT64_C(0) ? 0 : 18;
}}
"#,
        main = symbol("app.main"),
        failure = symbol("probe.failure"),
    );
    let root = std::env::temp_dir().join(format!(
        "internal-owned-slice-settlement-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
    ));
    std::fs::create_dir(&root).unwrap();
    let c_source = root.join("resource-settlement.c");
    std::fs::write(&c_source, format!("{declarations}\n{tracked}\n{allocator}\n{probe}")).unwrap();
    for optimization in ["-O0", "-O2"] {
        let binary = root.join(format!("resource-settlement-{optimization}"));
        let compiled = Command::new("clang")
            .args([
                "-std=c11",
                optimization,
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DSPX_NO_ENTRY_WRAPPER",
            ])
            .arg(&c_source)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{optimization}: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let executed = Command::new(&binary).output().unwrap();
        assert!(
            executed.status.success(),
            "{optimization}: exit={:?} stderr={}",
            executed.status.code(),
            String::from_utf8_lossy(&executed.stderr)
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}
