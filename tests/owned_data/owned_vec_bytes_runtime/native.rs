//! Native O0/O2 settlement probe for the owned Bytes Vec profile.
use semaprax::codegen;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT_NATIVE_CASE: AtomicU64 = AtomicU64::new(0);

pub(super) fn run_native(
    source: &str,
    expected_domain: &str,
    expected_code: u32,
    expected_value: i64,
    refusal_mode: &str,
) {
    let parsed = semaprax::check(source, "owned-vec-bytes-native.spx").unwrap();
    let generated = codegen::emit_c(&parsed).unwrap();
    assert!(generated.contains("spx_vec_bytes_with_capacity"));
    // Mixed programs may read scalar elements; only the owned tag must stay closed.
    assert!(!generated
        .lines()
        .any(|line| line.contains("spx_vec_get(spx_ctx, &") && line.contains("UINT32_C(9)")));
    let mut surface = generated.clone();
    for admitted in [
        "memcpy(payload, value.ptr, (size_t)value.len);",
        "memcpy(entry->domain_storage, status.domain_id, domain_size);",
    ] {
        assert_eq!(surface.matches(admitted).count(), 1);
        surface = surface.replacen(admitted, "", 1);
    }
    assert!(
        !surface.contains("memcpy("),
        "Vec transfer must not copy Bytes"
    );
    let tracked = generated
        .replace(
            "uint8_t *payload = (uint8_t *)malloc(",
            "uint8_t *payload = (uint8_t *)spx_test_malloc(",
        )
        .replace(
            "(spx_bytes_v1 *)calloc((size_t)capacity, sizeof(spx_bytes_v1))",
            if refusal_mode == "allocation" {
                "(spx_bytes_v1 *)spx_test_refused_calloc((size_t)capacity, sizeof(spx_bytes_v1))"
            } else {
                "(spx_bytes_v1 *)spx_test_calloc((size_t)capacity, sizeof(spx_bytes_v1))"
            },
        )
        .replace(
            "(spx_bytes_v1 *)calloc((size_t)target, sizeof(spx_bytes_v1))",
            if refusal_mode == "reserve" {
                "(spx_bytes_v1 *)spx_test_refused_calloc((size_t)target, sizeof(spx_bytes_v1))"
            } else {
                "(spx_bytes_v1 *)spx_test_calloc((size_t)target, sizeof(spx_bytes_v1))"
            },
        )
        .replace(
            "(uint8_t *)calloc((size_t)count, sizeof(uint8_t))",
            "(uint8_t *)spx_test_calloc((size_t)count, sizeof(uint8_t))",
        )
        .replace("(spx_bytes_v1*)calloc(", "(spx_bytes_v1*)spx_test_calloc(")
        .replace(
            "#define SPX_VEC_REALLOC realloc",
            "#define SPX_VEC_REALLOC spx_test_realloc",
        )
        .replace("free(", "spx_test_free(");
    let probe = format!(
        r#"
int main(void) {{
 struct spx_status_entry entries[UINT32_C(32)]; struct spx_context context={{0}};
 if(!spx_context_init(&context,UINT64_C(17),entries,UINT32_C(32),NULL,NULL,NULL))return 1;
 for(uint32_t i=0;i<UINT32_C(4);++i){{
  int64_t result=INT64_C(0x2525252525252525); uint32_t before=context.status_arena.length;
  spx_status_token status=spx_decl_6170702e6d61696e(&context,&result);
  if(UINT32_C({expected_code})==SPX_STATUS_SUCCESS){{
   if(status!=SPX_STATUS_SUCCESS||result!=INT64_C({expected_value})||context.status_arena.length!=before)return 2;
  }} else {{
   const struct spx_normalized_status *entry=spx_status_resolve(&context,status);
   if(status==SPX_STATUS_SUCCESS||result!=INT64_C(0x2525252525252525)||entry==NULL||entry->code!=UINT32_C({expected_code})||strcmp(entry->domain_id,"{expected_domain}")!=0||context.status_arena.length!=before+UINT32_C(1))return 3;
  }}
  if(spx_test_live_allocations!=UINT64_C(0))return 4;
  for(uint32_t j=0;j<SPX_VEC_AUTHORITY_CAPACITY;++j)if(context.vec_authority[j].live)return 5;
 }} return 0;
}}"#
    );
    let allocator = r#"#include <stdint.h>
#include <stdlib.h>
static uint64_t spx_test_live_allocations=UINT64_C(0);
static __attribute__((unused)) void *spx_test_malloc(size_t n){void*p=malloc(n);if(p)++spx_test_live_allocations;return p;}
static __attribute__((unused)) void *spx_test_calloc(size_t n,size_t s){void*p=calloc(n,s);if(p)++spx_test_live_allocations;return p;}
static __attribute__((unused)) void *spx_test_realloc(void*p,size_t n){void*r=realloc(p,n);if(r && !p)++spx_test_live_allocations;return r;}
static __attribute__((unused)) void *spx_test_refused_calloc(size_t n,size_t s){(void)n;(void)s;return NULL;}
static __attribute__((unused)) void spx_test_free(void*p){if(p){if(!spx_test_live_allocations)abort();--spx_test_live_allocations;free(p);}}"#;
    for opt in ["-O0", "-O2"] {
        let base = std::env::temp_dir().join(format!(
            "semaprax-owned-vec-bytes-native-{}-{}-{opt}",
            std::process::id(),
            NEXT_NATIVE_CASE.fetch_add(1, Ordering::Relaxed)
        ));
        let c = base.with_extension("c");
        let bin = base.with_extension(std::env::consts::EXE_EXTENSION);
        std::fs::write(&c, format!("{allocator}\n{tracked}\n{probe}")).unwrap();
        let out = Command::new("clang")
            .args([
                "-std=c11",
                opt,
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DSPX_NO_ENTRY_WRAPPER",
            ])
            .arg(&c)
            .arg("-o")
            .arg(&bin)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{opt}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let status = Command::new(&bin).status().unwrap();
        assert!(
            status.success(),
            "native {opt} refusal={refusal_mode}: {status}"
        );
        let _ = std::fs::remove_file(c);
        let _ = std::fs::remove_file(bin);
    }
}
