//! Native O0/O2 settlement probe for the owned Bytes Box profile.
use semaprax::codegen;
use std::process::Command;

pub(super) fn run_native(
    source: &str,
    expected_status: u32,
    expected_value: i64,
    refuse_box: bool,
) {
    let parsed = semaprax::check(source, "owned-box-bytes-native.spx").unwrap();
    let generated = codegen::emit_c(&parsed).unwrap();
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
        "Box transfer must not copy Bytes"
    );
    let payload = "spx_bytes_v1 *payload = (spx_bytes_v1 *)malloc(sizeof(spx_bytes_v1));";
    assert_eq!(generated.matches(payload).count(), 1);
    let tracked = generated
        .replace(
            "uint8_t *payload = (uint8_t *)malloc(",
            "uint8_t *payload = (uint8_t *)spx_test_malloc(",
        )
        .replace(
            payload,
            if refuse_box {
                "spx_bytes_v1 *payload = NULL;"
            } else {
                "spx_bytes_v1 *payload = (spx_bytes_v1 *)spx_test_malloc(sizeof(spx_bytes_v1));"
            },
        )
        .replace("free(value->ptr);", "spx_test_free(value->ptr);")
        .replace("free(payload);", "spx_test_free(payload);");
    let domain = if refuse_box {
        "semaprax.box.v1"
    } else {
        "semaprax.contract.v1"
    };
    let probe = format!(
        r#"
int main(void) {{
 struct spx_status_entry entries[UINT32_C(32)]; struct spx_context context={{0}};
 if(!spx_context_init(&context,UINT64_C(17),entries,UINT32_C(32),NULL,NULL,NULL))return 1;
 for(uint32_t i=0;i<UINT32_C(4);++i){{
  int64_t result=INT64_C(0x2525252525252525); uint32_t before=context.status_arena.length;
  spx_status_token status=spx_decl_6170702e6d61696e(&context,&result);
  if(UINT32_C({expected_status})==SPX_STATUS_SUCCESS){{if(status!=SPX_STATUS_SUCCESS||result!=INT64_C({expected_value}))return 2;}}
  else {{if(status==SPX_STATUS_SUCCESS||result!=INT64_C(0x2525252525252525)||spx_status_resolve(&context,status)==NULL||spx_status_resolve(&context,status)->code!=UINT32_C({expected_status})||strcmp(spx_status_resolve(&context,status)->domain_id,"{domain}")!=0)return 3; if(context.status_arena.length!=before+UINT32_C(1))return 4;}}
  if(spx_test_live_allocations!=UINT64_C(0))return 5;
  for(uint32_t j=0;j<SPX_BOX_AUTHORITY_CAPACITY;++j)if(context.box_authority[j].live)return 6;
  if(status==SPX_STATUS_SUCCESS && context.status_arena.length!=before)return 7;
 }} return 0;
}}"#
    );
    let allocator = r#"#include <stdint.h>
#include <stdlib.h>
static uint64_t spx_test_live_allocations=UINT64_C(0);
static void *spx_test_malloc(size_t n){void*p=malloc(n);if(p)++spx_test_live_allocations;return p;}
static void spx_test_free(void*p){if(p){if(!spx_test_live_allocations)abort();--spx_test_live_allocations;free(p);}}"#;
    for opt in ["-O0", "-O2"] {
        let base = std::env::temp_dir().join(format!(
            "semaprax-owned-box-bytes-native-{}-{opt}",
            std::process::id()
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
            "native {opt} refusal={refuse_box} expected={expected_status}: {status}"
        );
        let _ = std::fs::remove_file(c);
        let _ = std::fs::remove_file(bin);
    }
}
