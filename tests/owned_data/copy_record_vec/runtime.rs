use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);

pub(super) fn run(name: &str, text: &str, code: u32, value: i64, refuse: bool) {
    let ast = semaprax::check(text, "copy-record-vec.spx").expect("source admission");
    let resolved = hir::resolve(&ast).expect("resolved admission");
    hir::validate(&resolved).expect("independent HIR replay");
    let root = std::env::temp_dir().join(format!(
        "copy-record-vec-{}-{}-{name}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    let source = root.join("case.spx");
    std::fs::write(&source, text).unwrap();
    if !refuse {
        for _ in 0..2 {
            let out = interpreter::interpret(
                &source,
                "app.main",
                &[],
                &interpreter::InterpreterOptions::default(),
            )
            .unwrap();
            interpreter::verify_envelope(&out.envelope).unwrap();
            let envelope: serde_json::Value = serde_json::from_str(&out.envelope).unwrap();
            if code == 0 {
                assert!(out.returned, "{}", out.envelope);
                assert_eq!(envelope["payload"]["outcome"]["value"], value.to_string());
            } else {
                assert!(!out.returned, "{}", out.envelope);
                assert_eq!(
                    envelope["payload"]["outcome"]["status"]["domain_id"],
                    "semaprax.vec.v1"
                );
                assert_eq!(envelope["payload"]["outcome"]["status"]["code"], code);
            }
        }
    }
    let generated = codegen::emit_c(&ast).unwrap();
    assert!(generated.contains("spx_copy_vec_check"));
    let tracked = generated
        .replace("calloc(", "spx_test_calloc(")
        .replace("realloc(", "spx_test_realloc(")
        .replace(
            "#define SPX_VEC_REALLOC realloc",
            "#define SPX_VEC_REALLOC spx_test_realloc",
        )
        .replace("free(", "spx_test_free(");
    let allocator = format!(
        r#"#include <stdint.h>
#include <stdlib.h>
static uint64_t allocations=0;
static __attribute__((unused)) void *spx_test_calloc(size_t n,size_t s){{if({refuse})return NULL;void*p=calloc(n,s);if(p)++allocations;return p;}}
static __attribute__((unused)) void *spx_test_realloc(void*p,size_t n){{int had=p!=NULL;void*r=realloc(p,n);if(r&&!had)++allocations;return r;}}
static __attribute__((unused)) void spx_test_free(void*p){{if(p){{if(!allocations)abort();--allocations;free(p);}}}}
"#,
        refuse = u8::from(refuse)
    );
    let probe = format!(
        r#"
int main(void) {{
 struct spx_status_entry entries[32];struct spx_context c={{0}};
 if(!spx_context_init(&c,UINT64_C(17),entries,32,NULL,NULL,NULL))return 1;
 for(unsigned i=0;i<3;++i){{
  int64_t value=INT64_C(123456789);uint32_t before=c.status_arena.length;
  spx_status_token status=spx_decl_6170702e6d61696e(&c,&value);
  if({code}==0){{if(status!=SPX_STATUS_SUCCESS||value!=INT64_C({value})||c.status_arena.length!=before)return 2;}}
  else{{const struct spx_normalized_status*e=spx_status_resolve(&c,status);if(status==SPX_STATUS_SUCCESS||value!=INT64_C(123456789)||e==NULL||e->code!={code}||strcmp(e->domain_id,"semaprax.vec.v1")||c.status_arena.length!=before+1)return 3;}}
  if(allocations)return 4;
  for(uint32_t j=0;j<SPX_VEC_AUTHORITY_CAPACITY;++j)if(c.vec_authority[j].live)return 5;
 }}return 0;
}}
"#
    );
    let c = root.join("case.c");
    std::fs::write(&c, format!("{allocator}\n{tracked}\n{probe}")).unwrap();
    for optimization in ["-O0", "-O2"] {
        let binary = root.join(format!("case{optimization}"));
        let output = Command::new("clang")
            .args([
                "-std=c11",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DSPX_NO_ENTRY_WRAPPER",
                optimization,
            ])
            .arg(&c)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output = Command::new(binary).output().unwrap();
        assert!(
            output.status.success(),
            "{name}: {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let bytes = semaprax::wasm::emit_module(&ast).unwrap();
    wasmparser::Validator::new().validate_all(&bytes).unwrap();
    let module = root.join("case.wasm");
    std::fs::write(&module, bytes).unwrap();
    let output = Command::new("node")
        .arg("-e")
        .arg(include_str!("host.js"))
        .arg(&module)
        .arg(if code == 0 { 0 } else { code + 12 }.to_string())
        .arg(value.to_string())
        .arg(u8::from(refuse).to_string())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{name}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::remove_dir_all(root).unwrap();
}
